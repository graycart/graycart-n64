//! Edge-walker rasterizer: triangles, fill rectangles, texture rectangles.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   (triangles `0x08..=0x0F`, texture rectangles `0x24`/`0x25`, fill rectangle
//!   `0x36`, scissor `0x2D`, primitive depth `0x2E`, other modes, fill/color/depth image)
//! Cited: N64brew — Reality Display Processor/Pipeline
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Pipeline
//!   docs/04-rdp.md
//!
//! `regs::submit_commands` still calls [`unimplemented_primitive`] with only the
//! opcode word (and the regs tests require that path to trap). Draw through
//! [`draw_primitive`], which consumes the full command.
//!
//! ## Texture, combiner, blender
//!
//! Textured fragments call [`crate::rdp::tmem::sample_texel`] (point sample).
//! FILL mode does not: a texture rectangle in FILL is a fill rectangle, and the
//! fill color is stored verbatim. COPY mode still traps — that pipeline writes
//! 64-bit texel groups, which the point sampler does not describe.
//!
//! A non-zero [`CpuRdpLle::combine_mode`] is evaluated with
//! [`crate::rdp::combine_blend::eval_combine`] and, when the blender accepts the
//! mode, [`crate::rdp::combine_blend::eval_blend`]. Mode 0 is the unprogrammed
//! combiner: shaded fragments write the interpolated shade, unshaded fragments
//! write `prim_color`, and a textured fragment writes the point texel. The
//! blender traps on anti-alias, `color_on_cvg`, and coverage-as-alpha; those
//! bits stay in this module's coverage path and the combiner color is stored
//! directly. `alpha_compare_en` and RGB-dither noise still trap.
//!
//! ## Coverage and depth packing
//!
//! Pipeline coverage is 3 bits, 7 = full (n64brew cvg_dest / "memory coverage
//! full (7)").
//!
//! * **16-bit color:** coverage bit 2 is the RGBA5551 LSB, bits 1:0 are the
//!   hidden bits of the high byte then the low byte.
//! * **32-bit color:** coverage is alpha\[7:5\]. The same three bits are mirrored
//!   into the hidden bits of the pixel's first three bytes.
//! * **Depth:** the 18-bit value's top 16 bits sit in the halfword; bits 1:0 sit
//!   in the two hidden bits (high byte, then low byte). Deltaz compression is
//!   not applied; the buffer holds the raw 18-bit Z.

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;

use crate::bus::Rdram;
use crate::rdp::combine_blend::{self, BlendInputs, BlendOutput, CombineInputs};
use crate::rdp::tmem::{self, TexelRgba};
use crate::rdp::{CpuRdpLle, RdpSpanTemps};
use crate::trap::Trap;

const CYCLE_COPY: u8 = 2;
const CYCLE_FILL: u8 = 3;

const CMD_TEX_RECT: u8 = 0x24;
const CMD_TEX_RECT_FLIP: u8 = 0x25;
const CMD_PRIM_DEPTH: u8 = 0x2E;
const CMD_FILL_RECT: u8 = 0x36;

/// One-word primitive entry used by the command walker.
///
/// Always a trap: the walker does not pass coefficient words or RDRAM. Use
/// [`draw_primitive`] for the real rasterizer.
pub fn unimplemented_primitive(cmd_id: u8, word: u64) -> Result<(), Trap> {
    Err(Trap::UnimplementedRdpCommand { cmd_id, word })
}

/// Words occupied by the primitive that starts at `word`, including the opcode.
#[must_use]
pub fn primitive_word_count(word: u64) -> usize {
    let cmd = command_id(word);
    match cmd {
        0x08..=0x0F => {
            let mut n = 4usize;
            if cmd & 0b100 != 0 {
                n += 8;
            }
            if cmd & 0b010 != 0 {
                n += 8;
            }
            if cmd & 0b001 != 0 {
                n += 2;
            }
            n
        }
        CMD_TEX_RECT | CMD_TEX_RECT_FLIP => 2,
        _ => 1,
    }
}

/// Latch `Set Primitive Depth` (`0x2E`).
///
/// The command walker does not route this opcode yet; callers (and tests)
/// invoke this directly. Any other opcode traps.
pub fn set_primitive_depth(rdp: &mut CpuRdpLle, word: u64) -> Result<(), Trap> {
    let cmd = command_id(word);
    if cmd != CMD_PRIM_DEPTH {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    rdp.prim_z = ((word >> 16) & 0xFFFF) as u16;
    rdp.prim_dz = (word & 0xFFFF) as u16;
    Ok(())
}

/// Draw the primitive at the front of `words`.
///
/// Returns how many 64-bit words the command consumed.
pub fn draw_primitive(
    rdp: &mut CpuRdpLle,
    words: &[u64],
    rdram: &mut Rdram,
) -> Result<usize, Trap> {
    let Some(&word) = words.first() else {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: 0, word: 0 });
    };
    let cmd = command_id(word);
    let len = primitive_word_count(word);
    if words.len() < len {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    match cmd {
        0x08..=0x0F => draw_triangle(rdp, &words[..len], rdram),
        CMD_TEX_RECT | CMD_TEX_RECT_FLIP => {
            draw_texture_rectangle(rdp, &words[..len], rdram)?;
            Ok(len)
        }
        CMD_FILL_RECT => {
            draw_fill_rectangle(rdp, word, rdram)?;
            Ok(1)
        }
        _ => Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word }),
    }
}

/// Rasterize one triangle command (`0x08..=0x0F`), including shade / texture / Z words.
pub fn draw_triangle(rdp: &mut CpuRdpLle, words: &[u64], rdram: &mut Rdram) -> Result<usize, Trap> {
    let Some(&word) = words.first() else {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: 0, word: 0 });
    };
    let cmd = command_id(word);
    if !(0x08..=0x0F).contains(&cmd) {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    let len = primitive_word_count(word);
    if words.len() < len {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    let cycle = cycle_type(rdp);
    if cycle == CYCLE_COPY {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    let shaded = cmd & 0b100 != 0;
    let has_z = cmd & 0b001 != 0;
    reject_unimplemented_modes(rdp, cmd, word, cycle)?;

    let tri = parse_triangle(&words[..len])?;
    if cycle == CYCLE_FILL && !tri.lmajor {
        // n64brew: FILL + right-major triangles are liable to deadlock the pipe.
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }

    let fill_like = cycle == CYCLE_FILL;
    let aa = antialias(rdp) && !fill_like;
    let y0 = tri.yh >> 2;
    let y1 = div_ceil_4(tri.yl);
    let y_mid = tri.ym >> 2;

    let mut x_major = tri.xh;
    let mut x_minor = tri.xm;
    let mut on_lower = false;
    let mut edge_rgba = tri.shade;
    let mut edge_z = tri.z;
    let mut edge_tex = tri.tex;

    rdp.span = RdpSpanTemps::default();
    let mut y = y0;
    while y < y1 {
        if !on_lower && y >= y_mid {
            x_minor = tri.xl;
            on_lower = true;
        }
        rdp.span = RdpSpanTemps {
            y,
            x_major,
            x_minor,
            valid: true,
        };
        let (left, right) = if tri.lmajor {
            (x_major, x_minor)
        } else {
            (x_minor, x_major)
        };
        if left < right {
            let (x_start, x_end) = if aa {
                (
                    first_overlap_x(left).clamp(0, 4096),
                    last_overlap_end(right).clamp(0, 4096),
                )
            } else {
                (
                    first_covered_x(left).clamp(0, 4096),
                    first_rejected_x(right).clamp(0, 4096),
                )
            };
            for x in x_start..x_end {
                if !allow_line(rdp, y) {
                    continue;
                }
                if aa {
                    if !scissor_overlaps_pixel(rdp, x, y) {
                        continue;
                    }
                } else if !pixel_in_scissor(rdp, x, y, fill_like) {
                    continue;
                }
                let cvg = if fill_like {
                    7
                } else if aa {
                    let raw = coverage_raw(left, right, tri.yh, tri.yl, x, y);
                    if raw == 0 {
                        continue;
                    }
                    raw - 1
                } else if center_inside(left, right, tri.yh, tri.yl, x, y) {
                    7
                } else {
                    continue;
                };
                let rgba = if fill_like {
                    None
                } else {
                    let shade = if shaded {
                        sample_shade(edge_rgba, tri.drdx, x, x_major)
                    } else {
                        rgba_from_u32(rdp.prim_color)
                    };
                    let texel = match edge_tex {
                        Some(tex) => Some(sample_tri_texel(rdp, tex, x, x_major)?),
                        None => None,
                    };
                    Some(shade_texel_color(rdp, shade, texel)?)
                };
                commit_pixel(
                    rdp,
                    rdram,
                    Frag {
                        x,
                        y,
                        rgba,
                        cvg,
                        fill: fill_like,
                        edge_z,
                        dzdx: tri.dzdx,
                        x_major,
                    },
                )?;
            }
        }
        let minor_step = if on_lower { tri.dxldy } else { tri.dxmdy };
        x_major += tri.dxhdy;
        x_minor += minor_step;
        if shaded {
            edge_rgba.r += tri.drde.r;
            edge_rgba.g += tri.drde.g;
            edge_rgba.b += tri.drde.b;
            edge_rgba.a += tri.drde.a;
        }
        if let Some(tex) = edge_tex.as_mut() {
            tex.s = tex.s.wrapping_add(tex.dsde);
            tex.t = tex.t.wrapping_add(tex.dtde);
        }
        if has_z {
            edge_z += tri.dzde;
        }
        y += 1;
    }
    Ok(len)
}

/// `Fill Rectangle` (`0x36`).
pub fn draw_fill_rectangle(rdp: &mut CpuRdpLle, word: u64, rdram: &mut Rdram) -> Result<(), Trap> {
    let cmd = command_id(word);
    if cmd != CMD_FILL_RECT {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    paint_rect(rdp, word, None, rdram)
}

/// `Texture Rectangle` (`0x24`) or `Texture Rectangle Flip` (`0x25`).
///
/// Two words. FILL mode ignores the derivative word and fills. Other cycles
/// point-sample [`tmem::sample_texel`]. Flip applies `dsdx` along Y and `dtdy`
/// along X. COPY mode traps.
pub fn draw_texture_rectangle(
    rdp: &mut CpuRdpLle,
    words: &[u64],
    rdram: &mut Rdram,
) -> Result<(), Trap> {
    let Some(&word) = words.first() else {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: 0, word: 0 });
    };
    let cmd = command_id(word);
    if cmd != CMD_TEX_RECT && cmd != CMD_TEX_RECT_FLIP {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    if words.len() < 2 {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    if cycle_type(rdp) == CYCLE_FILL {
        return paint_rect(rdp, word, None, rdram);
    }
    let tex = TexRect {
        flip: cmd == CMD_TEX_RECT_FLIP,
        tile: ((word >> 24) & 7) as u8,
        s: i16_lane(words[1], 48),
        t: i16_lane(words[1], 32),
        dsdx: i16_lane(words[1], 16),
        dtdy: i16_lane(words[1], 0),
    };
    paint_rect(rdp, word, Some(tex), rdram)
}

/// Read the 3-bit coverage stored for a color pixel.
///
/// `bytes` is 2 (RGBA16) or 4 (RGBA32). Other sizes have no coverage record
/// and return 0.
#[must_use]
pub fn read_coverage(rdram: &Rdram, addr: u32, bytes: u32) -> u8 {
    let addr = addr as usize;
    if bytes == 4 {
        let a = rdram.read_u8(addr as u32 + 3);
        return (a >> 5) & 7;
    }
    if bytes == 2 {
        let px = rdram.read_u16(addr as u32).unwrap_or(0);
        let h1 = u8::from(rdram.read_hidden_bit(addr));
        let h0 = u8::from(rdram.read_hidden_bit(addr + 1));
        return ((px & 1) as u8) << 2 | (h1 << 1) | h0;
    }
    0
}

/// Read an 18-bit depth value (raw, not deltaz-compressed).
pub fn read_depth(rdram: &Rdram, addr: u32) -> Result<u32, Trap> {
    let half = u32::from(rdram.read_u16(addr)?);
    let h1 = u32::from(rdram.read_hidden_bit(addr as usize));
    let h0 = u32::from(rdram.read_hidden_bit(addr as usize + 1));
    Ok((half << 2) | (h1 << 1) | h0)
}

/// Write an 18-bit depth value into the halfword plus two hidden bits.
pub fn write_depth(rdram: &mut Rdram, addr: u32, z18: u32) -> Result<(), Trap> {
    let z18 = z18 & 0x3_FFFF;
    rdram.write_u16(addr, (z18 >> 2) as u16)?;
    rdram.write_hidden_bit(addr as usize, (z18 & 2) != 0);
    rdram.write_hidden_bit(addr as usize + 1, (z18 & 1) != 0);
    Ok(())
}

fn paint_rect(
    rdp: &mut CpuRdpLle,
    word: u64,
    tex: Option<TexRect>,
    rdram: &mut Rdram,
) -> Result<(), Trap> {
    let cmd = command_id(word);
    let cycle = cycle_type(rdp);
    if cycle == CYCLE_COPY {
        // COPY fill/tex rectangles sample TMEM. No sampler yet.
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    reject_unimplemented_modes(rdp, cmd, word, cycle)?;
    let ulx = ((word >> 12) & 0xFFF) as i32;
    let uly = (word & 0xFFF) as i32;
    let lrx = ((word >> 44) & 0xFFF) as i32;
    let lry = ((word >> 32) & 0xFFF) as i32;
    let fill_like = cycle == CYCLE_FILL;
    let aa = antialias(rdp) && !fill_like;

    let (x0, x1, y0, y1) = if fill_like {
        let x0 = ulx >> 2;
        let y0 = uly >> 2;
        let x1 = ceil_10_2(lrx);
        let y1 = ceil_10_2(lry);
        if x0 > x1 || y0 > y1 {
            rdp.span = RdpSpanTemps::default();
            return Ok(());
        }
        (x0, x1 + 1, y0, y1 + 1) // inclusive ends, half-open loop
    } else {
        // Exclusive lower-right. Visit every pixel the box might touch;
        // the center / coverage test drops the rest.
        let x0 = ulx >> 2;
        let y0 = uly >> 2;
        let x1 = (lrx >> 2) + 1;
        let y1 = (lry >> 2) + 1;
        (x0, x1, y0, y1)
    };

    rdp.span = RdpSpanTemps::default();
    let left = (ulx as i64) << 14;
    let right = (lrx as i64) << 14;
    for y in y0..y1 {
        if !allow_line(rdp, y) {
            continue;
        }
        rdp.span = RdpSpanTemps {
            y,
            x_major: left,
            x_minor: right,
            valid: true,
        };
        for x in x0..x1 {
            if x < 0 || y < 0 || !pixel_in_scissor(rdp, x, y, fill_like) {
                continue;
            }
            let cvg = if fill_like {
                7
            } else if aa {
                let raw = rect_coverage(ulx, uly, lrx, lry, rdp, x, y);
                if raw == 0 {
                    continue;
                }
                raw - 1
            } else if rect_center_inside(ulx, uly, lrx, lry, x, y)
                && pixel_in_scissor(rdp, x, y, false)
            {
                7
            } else {
                continue;
            };
            let rgba = if fill_like {
                None
            } else {
                let shade = rgba_from_u32(rdp.prim_color);
                let texel = match tex {
                    Some(src) => Some(sample_rect_texel(rdp, src, x, y, ulx, uly)?),
                    None => None,
                };
                Some(shade_texel_color(rdp, shade, texel)?)
            };
            let z_anchor = ((rdp.prim_z as i64) & 0x7FFF) << 16;
            commit_pixel(
                rdp,
                rdram,
                Frag {
                    x,
                    y,
                    rgba,
                    cvg,
                    fill: fill_like,
                    edge_z: z_anchor,
                    dzdx: 0,
                    x_major: left,
                },
            )?;
        }
    }
    Ok(())
}

fn commit_pixel(rdp: &mut CpuRdpLle, rdram: &mut Rdram, frag: Frag) -> Result<(), Trap> {
    let Frag {
        x,
        y,
        rgba,
        cvg: new_cvg,
        fill: fill_like,
        edge_z,
        dzdx,
        x_major,
    } = frag;
    let img = rdp.color_image;
    if img.size == 0 && fill_like {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: CMD_FILL_RECT,
            word: u64::from(CMD_FILL_RECT) << 56,
        });
    }
    if matches!(img.size, 2 | 3) && img.format != 0 && !fill_like {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: command_id_from_image(),
            word: 0,
        });
    }

    if !fill_like && (z_compare(rdp) || z_update(rdp)) {
        let z18 = pixel_z18(rdp, edge_z, dzdx, x, x_major);
        let addr = depth_addr(rdp, x, y);
        if z_compare(rdp) {
            let old = read_depth(rdram, addr)?;
            let dz = pixel_dz(rdp, dzdx);
            if !z_passes(z_mode(rdp), z18, old, dz) {
                return Ok(());
            }
        }
        if z_update(rdp) {
            write_depth(rdram, addr, z18)?;
        }
    }

    write_color(
        rdp,
        rdram,
        &Frag {
            x,
            y,
            rgba,
            cvg: new_cvg,
            fill: fill_like,
            edge_z,
            dzdx,
            x_major,
        },
    )
}

fn write_color(rdp: &CpuRdpLle, rdram: &mut Rdram, frag: &Frag) -> Result<(), Trap> {
    let Frag {
        x,
        y,
        rgba,
        cvg: new_cvg,
        fill: fill_like,
        ..
    } = *frag;
    let img = rdp.color_image;
    let width = u32::from(img.width) + 1;
    let pix = (y as u32) * width + (x as u32);
    match img.size {
        0 => {
            let addr = img.addr.wrapping_add(pix / 2);
            rdram.write_u8(addr, 0);
            Ok(())
        }
        1 => {
            let addr = img.addr.wrapping_add(pix);
            let byte = if fill_like {
                let shift = (3 - (x & 3)) * 8;
                ((rdp.fill_color >> shift) & 0xFF) as u8
            } else {
                rgba.unwrap_or_default().r
            };
            rdram.write_u8(addr, byte);
            Ok(())
        }
        2 => {
            let addr = img.addr.wrapping_add(pix * 2);
            if fill_like {
                let half = if x & 1 == 0 {
                    (rdp.fill_color >> 16) as u16
                } else {
                    rdp.fill_color as u16
                };
                return rdram.write_u16(addr, half);
            }
            let rgba = rgba.unwrap_or_default();
            let out_cvg = combine_cvg(rdp, rdram, addr, 2, new_cvg);
            if keep_memory_color(rdp, rdram, addr, 2, new_cvg) {
                let mut px = rdram.read_u16(addr)?;
                px = (px & !1) | u16::from(out_cvg >> 2);
                rdram.write_u16(addr, px)?;
            } else {
                let d = dither_of(rdp, x, y);
                let r5 = quantize5(rgba.r, d);
                let g5 = quantize5(rgba.g, d);
                let b5 = quantize5(rgba.b, d);
                let px = (r5 << 11) | (g5 << 6) | (b5 << 1) | u16::from(out_cvg >> 2);
                rdram.write_u16(addr, px)?;
            }
            rdram.write_hidden_bit(addr as usize, (out_cvg & 2) != 0);
            rdram.write_hidden_bit(addr as usize + 1, (out_cvg & 1) != 0);
            Ok(())
        }
        3 => {
            let addr = img.addr.wrapping_add(pix * 4);
            if fill_like {
                return rdram.write_u32(addr, rdp.fill_color);
            }
            let rgba = rgba.unwrap_or_default();
            let out_cvg = combine_cvg(rdp, rdram, addr, 4, new_cvg);
            let a_lo = u32::from(rgba.a >> 3);
            let a = (u32::from(out_cvg) << 5) | a_lo;
            if keep_memory_color(rdp, rdram, addr, 4, new_cvg) {
                let old = rdram.read_u32(addr)?;
                let px = (old & 0xFFFF_FF1F) | (u32::from(out_cvg) << 5);
                rdram.write_u32(addr, px)?;
            } else {
                let px = (u32::from(rgba.r) << 24)
                    | (u32::from(rgba.g) << 16)
                    | (u32::from(rgba.b) << 8)
                    | a;
                rdram.write_u32(addr, px)?;
            }
            for i in 0..3 {
                rdram.write_hidden_bit(addr as usize + i, ((out_cvg >> i) & 1) != 0);
            }
            rdram.write_hidden_bit(addr as usize + 3, false);
            Ok(())
        }
        _ => Err(Trap::UnimplementedRdpCommand {
            cmd_id: CMD_FILL_RECT,
            word: u64::from(CMD_FILL_RECT) << 56,
        }),
    }
}

fn combine_cvg(rdp: &CpuRdpLle, rdram: &Rdram, addr: u32, bytes: u32, new_cvg: u8) -> u8 {
    let read = image_read(rdp);
    let old = if read {
        read_coverage(rdram, addr, bytes)
    } else {
        0
    };
    let sum = u16::from(old) + u16::from(new_cvg);
    match cvg_dest(rdp) {
        0 => sum.min(7) as u8,
        1 => (sum & 7) as u8,
        2 => 7,
        _ => {
            if read {
                old
            } else {
                7
            }
        }
    }
}

fn keep_memory_color(rdp: &CpuRdpLle, rdram: &Rdram, addr: u32, bytes: u32, new_cvg: u8) -> bool {
    if !color_on_cvg(rdp) || !image_read(rdp) {
        return false;
    }
    let old = u16::from(read_coverage(rdram, addr, bytes));
    old + u16::from(new_cvg) < 8
}

fn pixel_z18(rdp: &CpuRdpLle, edge_z: i64, dzdx: i64, x: i32, x_major: i64) -> u32 {
    if z_source_prim(rdp) {
        return (u32::from(rdp.prim_z) << 3) & 0x3_FFFF;
    }
    let dx = ((x as i64) << 16) - x_major;
    let z = edge_z + ((dzdx * dx) >> 16);
    z18_from_fixed(z)
}

fn pixel_dz(rdp: &CpuRdpLle, dzdx: i64) -> u32 {
    if z_source_prim(rdp) {
        let dz = u32::from(rdp.prim_dz);
        return if dz == 0 { 1 } else { dz };
    }
    let mag = (dzdx.abs() >> 13) as u32;
    if mag == 0 {
        1
    } else {
        mag
    }
}

fn z18_from_fixed(z: i64) -> u32 {
    if z <= 0 {
        0
    } else {
        ((z >> 13) as u32) & 0x3_FFFF
    }
}

fn z_passes(mode: u8, new_z: u32, old_z: u32, dz: u32) -> bool {
    match mode {
        // Opaque and transparent: nearer (smaller) wins. Z update is separate.
        0 | 2 => new_z < old_z,
        // Interpenetrating: a pixel still in front of old+dz passes.
        1 => new_z < old_z.saturating_add(dz),
        // Decal: within ±dz of the surface already in the buffer.
        _ => {
            let lo = old_z.saturating_sub(dz);
            let hi = old_z.saturating_add(dz);
            (lo..=hi).contains(&new_z)
        }
    }
}

fn depth_addr(rdp: &CpuRdpLle, x: i32, y: i32) -> u32 {
    let width = u32::from(rdp.color_image.width) + 1;
    rdp.depth_image_addr
        .wrapping_add((y as u32 * width + x as u32) * 2)
}

fn reject_unimplemented_modes(rdp: &CpuRdpLle, cmd: u8, word: u64, cycle: u8) -> Result<(), Trap> {
    if cycle == CYCLE_FILL || cycle == CYCLE_COPY {
        return Ok(());
    }
    if alpha_compare(rdp) {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    if rdp.color_image.size == 2 && rgb_dither(rdp) == 2 {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    let want_z = z_compare(rdp) || z_update(rdp);
    let pixel_z = (0x08..=0x0F).contains(&cmd) && (cmd & 0b001) != 0;
    if want_z && !z_source_prim(rdp) && !pixel_z {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: cmd, word });
    }
    Ok(())
}

fn command_id_from_image() -> u8 {
    0x3F
}

#[derive(Clone, Copy)]
struct Frag {
    x: i32,
    y: i32,
    rgba: Option<Rgba8>,
    cvg: u8,
    fill: bool,
    edge_z: i64,
    dzdx: i64,
    x_major: i64,
}

#[derive(Clone, Copy, Default)]
struct Rgba8 {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Clone, Copy, Default)]
struct Shade {
    r: i64,
    g: i64,
    b: i64,
    a: i64,
}

struct Tri {
    lmajor: bool,
    yh: i32,
    ym: i32,
    yl: i32,
    xh: i64,
    xm: i64,
    xl: i64,
    dxhdy: i64,
    dxmdy: i64,
    dxldy: i64,
    shade: Shade,
    drdx: Shade,
    drde: Shade,
    z: i64,
    dzdx: i64,
    dzde: i64,
    tex: Option<TexAttr>,
}

#[derive(Clone, Copy)]
struct TexRect {
    flip: bool,
    tile: u8,
    s: i32,
    t: i32,
    /// s5.10.
    dsdx: i32,
    /// s5.10.
    dtdy: i32,
}

#[derive(Clone, Copy)]
struct TexAttr {
    tile: u8,
    /// s10.5 coordinate at the major edge.
    s: i32,
    t: i32,
    /// Added to `s` per pixel of +X (s10.5). Integer lane of `dsdx`.
    dsdx: i32,
    dtdx: i32,
    dsde: i32,
    dtde: i32,
}

fn parse_triangle(words: &[u64]) -> Result<Tri, Trap> {
    let w0 = words[0];
    let cmd = command_id(w0);
    let (xh, dxhdy) = parse_edge(words[2]);
    let (xm, dxmdy) = parse_edge(words[3]);
    let (xl, dxldy) = parse_edge(words[1]);
    let mut idx = 4usize;
    let mut shade = Shade::default();
    let mut drdx = Shade::default();
    let mut drde = Shade::default();
    if cmd & 0b100 != 0 {
        if words.len() < idx + 8 {
            return Err(Trap::UnimplementedRdpCommand {
                cmd_id: cmd,
                word: w0,
            });
        }
        shade = parse_color(words[idx], words[idx + 2]);
        drdx = parse_shade_level(words[idx + 1], words[idx + 3]);
        drde = parse_shade_level(words[idx + 4], words[idx + 6]);
        // drdy (words idx+5 / idx+7) is the pure vertical partial. The walker
        // steps the major-edge attribute with drde, which already includes it.
        let _drdy = parse_shade_level(words[idx + 5], words[idx + 7]);
        idx += 8;
    }
    let tex = if cmd & 0b010 != 0 {
        if words.len() < idx + 8 {
            return Err(Trap::UnimplementedRdpCommand {
                cmd_id: cmd,
                word: w0,
            });
        }
        let attr = Some(parse_tex_attr(
            words[idx],
            words[idx + 1],
            words[idx + 4],
            (w0 >> 48) as u8,
        ));
        idx += 8;
        attr
    } else {
        None
    };
    let mut z = 0i64;
    let mut dzdx = 0i64;
    let mut dzde = 0i64;
    if cmd & 0b001 != 0 {
        if words.len() < idx + 2 {
            return Err(Trap::UnimplementedRdpCommand {
                cmd_id: cmd,
                word: w0,
            });
        }
        let zw = words[idx];
        let dw = words[idx + 1];
        z = s16_16(lane16(zw, 48), lane16(zw, 32));
        dzdx = s16_16(lane16(zw, 16), lane16(zw, 0));
        dzde = s16_16(lane16(dw, 48), lane16(dw, 32));
        let _dzdy = s16_16(lane16(dw, 16), lane16(dw, 0));
    }
    Ok(Tri {
        lmajor: (w0 & (1 << 55)) != 0,
        yl: sext14((w0 >> 32) as u32),
        ym: sext14((w0 >> 16) as u32),
        yh: sext14(w0 as u32),
        xh,
        xm,
        xl,
        dxhdy,
        dxmdy,
        dxldy,
        shade,
        drdx,
        drde,
        z,
        dzdx,
        dzde,
        tex,
    })
}

fn parse_tex_attr(anchor: u64, dx: u64, de: u64, tile: u8) -> TexAttr {
    // s[31:16] / t[31:16] are the s10.5 fields the point sampler consumes.
    // The integer lane of each derivative is one s10.5 step per pixel or scanline.
    TexAttr {
        tile: tile & 7,
        s: i16_lane(anchor, 48),
        t: i16_lane(anchor, 32),
        dsdx: i16_lane(dx, 48),
        dtdx: i16_lane(dx, 32),
        dsde: i16_lane(de, 48),
        dtde: i16_lane(de, 32),
    }
}

fn parse_edge(word: u64) -> (i64, i64) {
    let x = sext_fixed(((word >> 48) & 0xFFF) as u32, 12, lane16(word, 32));
    let dx = sext_fixed(((word >> 16) & 0x3FFF) as u32, 14, lane16(word, 0));
    (x, dx)
}

fn parse_color(int_word: u64, frac_word: u64) -> Shade {
    // Anchor color is s8.16: 9-bit integer field in bits [8:0] of each lane.
    Shade {
        r: sext_fixed(((int_word >> 48) & 0x1FF) as u32, 9, lane16(frac_word, 48)),
        g: sext_fixed(((int_word >> 32) & 0x1FF) as u32, 9, lane16(frac_word, 32)),
        b: sext_fixed(((int_word >> 16) & 0x1FF) as u32, 9, lane16(frac_word, 16)),
        a: sext_fixed((int_word & 0x1FF) as u32, 9, lane16(frac_word, 0)),
    }
}

fn parse_shade_level(int_word: u64, frac_word: u64) -> Shade {
    Shade {
        r: s16_16(lane16(int_word, 48), lane16(frac_word, 48)),
        g: s16_16(lane16(int_word, 32), lane16(frac_word, 32)),
        b: s16_16(lane16(int_word, 16), lane16(frac_word, 16)),
        a: s16_16(lane16(int_word, 0), lane16(frac_word, 0)),
    }
}

fn sample_shade(edge: Shade, drdx: Shade, x: i32, x_major: i64) -> Rgba8 {
    let dx = ((x as i64) << 16) - x_major;
    Rgba8 {
        r: channel8(edge.r + ((drdx.r * dx) >> 16)),
        g: channel8(edge.g + ((drdx.g * dx) >> 16)),
        b: channel8(edge.b + ((drdx.b * dx) >> 16)),
        a: channel8(edge.a + ((drdx.a * dx) >> 16)),
    }
}

fn channel8(v: i64) -> u8 {
    let i = v >> 16;
    if i <= 0 {
        0
    } else if i >= 255 {
        255
    } else {
        i as u8
    }
}

fn rgba_from_u32(c: u32) -> Rgba8 {
    Rgba8 {
        r: (c >> 24) as u8,
        g: (c >> 16) as u8,
        b: (c >> 8) as u8,
        a: c as u8,
    }
}

fn i16_lane(word: u64, shift: u32) -> i32 {
    ((word >> shift) as u16 as i16) as i32
}

fn sample_tri_texel(
    rdp: &CpuRdpLle,
    tex: TexAttr,
    x: i32,
    x_major: i64,
) -> Result<TexelRgba, Trap> {
    let dx = x.wrapping_sub((x_major >> 16) as i32);
    let s = tex.s.wrapping_add(tex.dsdx.wrapping_mul(dx));
    let t = tex.t.wrapping_add(tex.dtdx.wrapping_mul(dx));
    tmem::sample_texel(rdp, tex.tile, s, t)
}

fn sample_rect_texel(
    rdp: &CpuRdpLle,
    src: TexRect,
    x: i32,
    y: i32,
    ulx: i32,
    uly: i32,
) -> Result<TexelRgba, Trap> {
    let dx = x - (ulx >> 2);
    let dy = y - (uly >> 2);
    // dsdx / dtdy are s5.10. >> 5 converts one pixel of derivative into s10.5.
    let (s_step, t_step) = if src.flip {
        (
            i64::from(src.dsdx) * i64::from(dy),
            i64::from(src.dtdy) * i64::from(dx),
        )
    } else {
        (
            i64::from(src.dsdx) * i64::from(dx),
            i64::from(src.dtdy) * i64::from(dy),
        )
    };
    let s = src.s.wrapping_add((s_step >> 5) as i32);
    let t = src.t.wrapping_add((t_step >> 5) as i32);
    tmem::sample_texel(rdp, src.tile, s, t)
}

fn shade_texel_color(
    rdp: &CpuRdpLle,
    shade: Rgba8,
    texel: Option<TexelRgba>,
) -> Result<Rgba8, Trap> {
    let texel_rgba = texel.map(texel_to_rgba).unwrap_or_default();
    if rdp.combine_mode == 0 {
        return Ok(if texel.is_some() { texel_rgba } else { shade });
    }
    let combined = combine_blend::eval_combine(
        rdp,
        &CombineInputs {
            texel0: rgba_to_combine(texel_rgba),
            texel1: rgba_to_combine(texel_rgba),
            shade: rgba_to_combine(shade),
            ..CombineInputs::default()
        },
    )?;
    if blend_blocked(rdp) {
        return Ok(combine_to_rgba(combined));
    }
    match combine_blend::eval_blend(
        rdp,
        &BlendInputs {
            combined,
            shade_alpha: shade.a,
            shade_alpha_next: Some(shade.a),
            memory: combine_blend::Rgba8::default(),
            memory_prev: Some(combine_blend::Rgba8::default()),
        },
    )? {
        BlendOutput::Color(c) => Ok(combine_to_rgba(c)),
        BlendOutput::Fill(_) | BlendOutput::Copy => Ok(shade),
    }
}

fn blend_blocked(rdp: &CpuRdpLle) -> bool {
    // Same bits `eval_blend` rejects: AA, color_on_cvg, cvg*alpha, key.
    let m = rdp.other_modes;
    (m & ((1 << 3) | (1 << 7) | (1 << 12) | (1 << 13) | (1 << 40))) != 0
}

fn texel_to_rgba(t: TexelRgba) -> Rgba8 {
    Rgba8 {
        r: t.r,
        g: t.g,
        b: t.b,
        a: t.a,
    }
}

fn rgba_to_combine(c: Rgba8) -> combine_blend::Rgba8 {
    combine_blend::Rgba8::new(c.r, c.g, c.b, c.a)
}

fn combine_to_rgba(c: combine_blend::Rgba8) -> Rgba8 {
    Rgba8 {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn coverage_raw(left: i64, right: i64, yh: i32, yl: i32, x: i32, y: i32) -> u8 {
    let x0 = (x as i64) << 16;
    let x_ov = overlap(x0, x0 + 0x1_0000, left, right);
    let top = y * 4;
    let y_ov = overlap(
        i64::from(top),
        i64::from(top + 4),
        i64::from(yh),
        i64::from(yl),
    );
    raw_from_area(x_ov, y_ov)
}

fn rect_coverage(ulx: i32, uly: i32, lrx: i32, lry: i32, rdp: &CpuRdpLle, x: i32, y: i32) -> u8 {
    let (x0, y0, x1, y1) = clip_box_to_scissor(rdp, ulx, uly, lrx, lry);
    if x0 >= x1 || y0 >= y1 {
        return 0;
    }
    let px0 = (x as i64) << 16;
    let x_ov = overlap(px0, px0 + 0x1_0000, (x0 as i64) << 14, (x1 as i64) << 14);
    let top = i64::from(y * 4);
    let y_ov = overlap(top, top + 4, i64::from(y0), i64::from(y1));
    // y box is in 10.2 quarters; overlap() returns quarters. x_ov is 16.16.
    // Convert y overlap (0..=4) into the same raw scale as `coverage_raw`.
    raw_from_area(x_ov, y_ov)
}

fn raw_from_area(x_ov: i64, y_quarters: i64) -> u8 {
    if x_ov <= 0 || y_quarters <= 0 {
        return 0;
    }
    let raw = (x_ov * y_quarters) / 32_768;
    if raw <= 0 {
        1
    } else if raw >= 8 {
        8
    } else {
        raw as u8
    }
}

fn overlap(a0: i64, a1: i64, b0: i64, b1: i64) -> i64 {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    (hi - lo).max(0)
}

fn clip_box_to_scissor(
    rdp: &CpuRdpLle,
    ulx: i32,
    uly: i32,
    lrx: i32,
    lry: i32,
) -> (i32, i32, i32, i32) {
    let s = &rdp.scissor;
    (
        ulx.max(i32::from(s.ulx)),
        uly.max(i32::from(s.uly)),
        lrx.min(i32::from(s.lrx)),
        lry.min(i32::from(s.lry)),
    )
}

fn center_inside(left: i64, right: i64, yh: i32, yl: i32, x: i32, y: i32) -> bool {
    let cx = ((x as i64) << 16) + 0x8000;
    let cy = y * 4 + 2;
    cx >= left && cx < right && cy >= yh && cy < yl
}

fn rect_center_inside(ulx: i32, uly: i32, lrx: i32, lry: i32, x: i32, y: i32) -> bool {
    let cx = x * 4 + 2;
    let cy = y * 4 + 2;
    cx >= ulx && cx < lrx && cy >= uly && cy < lry
}

fn first_overlap_x(left: i64) -> i32 {
    (left.saturating_sub(1) >> 16) as i32
}

fn last_overlap_end(right: i64) -> i32 {
    div_ceil_i64(right, 0x1_0000) as i32
}

fn scissor_overlaps_pixel(rdp: &CpuRdpLle, x: i32, y: i32) -> bool {
    let px0 = x * 4;
    let py0 = y * 4;
    px0 < i32::from(rdp.scissor.lrx)
        && px0 + 4 > i32::from(rdp.scissor.ulx)
        && py0 < i32::from(rdp.scissor.lry)
        && py0 + 4 > i32::from(rdp.scissor.uly)
}

fn first_covered_x(left: i64) -> i32 {
    div_ceil_i64(left - 0x8000, 0x1_0000) as i32
}

fn first_rejected_x(right: i64) -> i32 {
    div_ceil_i64(right - 0x8000, 0x1_0000) as i32
}

fn pixel_in_scissor(rdp: &CpuRdpLle, x: i32, y: i32, fill_like: bool) -> bool {
    if !allow_line(rdp, y) {
        return false;
    }
    let ulx = i32::from(rdp.scissor.ulx);
    let uly = i32::from(rdp.scissor.uly);
    let lrx = i32::from(rdp.scissor.lrx);
    let lry = i32::from(rdp.scissor.lry);
    if fill_like {
        // Inclusive right edge, exclusive lower edge, subpixels ignored.
        x >= (ulx >> 2) && x <= (lrx >> 2) && y >= (uly >> 2) && y < (lry >> 2)
    } else {
        let sx = x * 4 + 2;
        let sy = y * 4 + 2;
        sx >= ulx && sx < lrx && sy >= uly && sy < lry
    }
}

fn allow_line(rdp: &CpuRdpLle, y: i32) -> bool {
    if !rdp.scissor.field {
        return true;
    }
    let odd_line = (y & 1) == 1;
    odd_line == rdp.scissor.odd
}

fn dither_of(rdp: &CpuRdpLle, x: i32, y: i32) -> i32 {
    let mode = rgb_dither(rdp);
    if mode == 3 {
        return 0;
    }
    let yi = if rdp.scissor.uly != 0 {
        (y >> 1) & 3
    } else {
        y & 3
    };
    let xi = (x & 3) as usize;
    let row = yi as usize;
    // n64brew RGB dither matrices, indexed by screen x[1:0] and y[1:0]
    // (y[2:1] when the scissor origin is not on a pixel boundary at y = 0).
    const MAGIC: [[i32; 4]; 4] = [[0, 6, 1, 7], [4, 2, 5, 3], [3, 5, 2, 4], [7, 1, 6, 0]];
    const BAYER: [[i32; 4]; 4] = [[0, 4, 1, 5], [4, 0, 5, 1], [3, 7, 2, 6], [7, 3, 6, 2]];
    if mode == 1 {
        BAYER[row][xi]
    } else {
        MAGIC[row][xi]
    }
}

fn quantize5(channel: u8, dither: i32) -> u16 {
    let v = (i32::from(channel) + dither).clamp(0, 255);
    (v >> 3) as u16
}

fn cycle_type(rdp: &CpuRdpLle) -> u8 {
    ((rdp.other_modes >> 52) & 3) as u8
}

fn rgb_dither(rdp: &CpuRdpLle) -> u8 {
    ((rdp.other_modes >> 38) & 3) as u8
}

fn cvg_dest(rdp: &CpuRdpLle) -> u8 {
    ((rdp.other_modes >> 8) & 3) as u8
}

fn z_mode(rdp: &CpuRdpLle) -> u8 {
    ((rdp.other_modes >> 10) & 3) as u8
}

fn flag(rdp: &CpuRdpLle, bit: u32) -> bool {
    (rdp.other_modes & (1u64 << bit)) != 0
}

fn color_on_cvg(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 7)
}

fn image_read(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 6)
}

fn z_update(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 5)
}

fn z_compare(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 4)
}

fn antialias(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 3)
}

fn z_source_prim(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 2)
}

fn alpha_compare(rdp: &CpuRdpLle) -> bool {
    flag(rdp, 0)
}

fn command_id(word: u64) -> u8 {
    ((word >> 56) & 0x3F) as u8
}

fn lane16(word: u64, shift: u32) -> u32 {
    ((word >> shift) & 0xFFFF) as u32
}

fn sext14(v: u32) -> i32 {
    let v = v & 0x3FFF;
    ((v << 18) as i32) >> 18
}

fn sext_fixed(int_field: u32, int_bits: u32, frac: u32) -> i64 {
    let total = int_bits + 16;
    let raw = ((u64::from(int_field) & ((1u64 << int_bits) - 1)) << 16) | u64::from(frac & 0xFFFF);
    let sign = 1u64 << (total - 1);
    if raw & sign != 0 {
        raw as i64 - (1i64 << total)
    } else {
        raw as i64
    }
}

fn s16_16(int_field: u32, frac: u32) -> i64 {
    sext_fixed(int_field, 16, frac)
}

fn div_ceil_4(v: i32) -> i32 {
    if v >= 0 {
        (v + 3) >> 2
    } else {
        -((-v) >> 2)
    }
}

fn div_ceil_i64(n: i64, d: i64) -> i64 {
    if n >= 0 {
        (n + d - 1) / d
    } else {
        -((-n) / d)
    }
}

fn ceil_10_2(v: i32) -> i32 {
    let i = v >> 2;
    if v & 3 == 0 {
        i
    } else {
        i + 1
    }
}
