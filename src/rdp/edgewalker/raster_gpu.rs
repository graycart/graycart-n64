// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! EdgeWalker host primitive decoder, bounding-box planner, and trap validators.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use crate::rdp::tmem::TileDesc;
use crate::rdp::{RdpImageState, RdpScissor};
use crate::trap::Trap;

use super::combine_gpu::validate_combine_and_blend;
use super::gpu_types::{
    RasterParamsPack, PRIM_FLAG_HAS_Z, PRIM_FLAG_LMAJOR, PRIM_FLAG_SHADED, PRIM_FLAG_TEXTURED,
    PRIM_FLAG_TILE_SHIFT, PRIM_KIND_FILL_RECT, PRIM_KIND_TEX_RECT, PRIM_KIND_TEX_RECT_FLIP,
    PRIM_KIND_TRIANGLE,
};
use super::tmem_gpu::validate_tile_sample;

pub const RASTER_CORE_WGSL: &str = include_str!("shaders/raster_core.wgsl");

/// Concatenate texel sampler, combiner/blender, and raster core WGSL into one compute shader.
#[must_use]
pub fn assemble_raster_wgsl(tmem_sample_wgsl: &str, combine_blend_wgsl: &str) -> String {
    format!("{tmem_sample_wgsl}\n{combine_blend_wgsl}\n{RASTER_CORE_WGSL}")
}

/// Latched host RDP state needed to validate and dispatch one primitive.
#[derive(Debug, Clone, Copy)]
pub struct HostRasterState<'a> {
    pub color_image: RdpImageState,
    pub depth_image_addr: u32,
    pub scissor: RdpScissor,
    pub other_modes: u64,
    pub combine_mode: u64,
    pub fill_color: u32,
    pub prim_color: u32,
    pub env_color: u32,
    pub fog_color: u32,
    pub blend_color: u32,
    pub prim_lod_frac: u8,
    pub prim_z: u16,
    pub prim_dz: u16,
    pub convert_k4: u16,
    pub convert_k5: u16,
    pub key_center: [u8; 3],
    pub key_scale: [u8; 3],
    pub tiles: &'a [TileDesc; 8],
}

/// Number of 64-bit words occupied by the command starting at `word`.
#[must_use]
pub fn primitive_word_count(word: u64) -> usize {
    let cmd = ((word >> 56) & 0x3F) as u8;
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
        0x24 | 0x25 => 2,
        _ => 1,
    }
}

/// Validate command words and hardware state, then pack [`RasterParamsPack`] and work thread count.
pub fn prepare_primitive(
    state: &HostRasterState<'_>,
    words: &[u64],
) -> Result<Option<(RasterParamsPack, u32)>, Trap> {
    let Some(&word) = words.first() else {
        return Err(Trap::UnimplementedRdpCommand { cmd_id: 0, word: 0 });
    };
    let cmd_id = ((word >> 56) & 0x3F) as u8;
    let len = primitive_word_count(word);
    if words.len() < len {
        return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
    }

    let cycle = ((state.other_modes >> 52) & 3) as u8;
    if cycle == 2 {
        return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
    }
    let fill_like = cycle == 3;
    let img = state.color_image;

    if !fill_like {
        if (state.other_modes & 1) != 0 {
            return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
        }
        let rgb_dither = ((state.other_modes >> 38) & 3) as u8;
        if img.size == 2 && rgb_dither == 2 {
            return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
        }
        let want_z = (state.other_modes & ((1 << 4) | (1 << 5))) != 0;
        let z_source_prim = (state.other_modes & (1 << 2)) != 0;
        let pixel_z = (0x08..=0x0F).contains(&cmd_id) && (cmd_id & 0b001) != 0;
        if want_z && !z_source_prim && !pixel_z {
            return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
        }
        validate_combine_and_blend(
            state.combine_mode,
            state.other_modes,
            state.prim_color,
            state.fog_color,
        )?;
    }

    let key_center_rgb = (u32::from(state.key_center[0]) << 16)
        | (u32::from(state.key_center[1]) << 8)
        | u32::from(state.key_center[2]);
    let key_scale_rgb = (u32::from(state.key_scale[0]) << 16)
        | (u32::from(state.key_scale[1]) << 8)
        | u32::from(state.key_scale[2]);

    let mut pack = RasterParamsPack {
        other_modes_lo: state.other_modes as u32,
        other_modes_hi: (state.other_modes >> 32) as u32,
        combine_lo: state.combine_mode as u32,
        combine_hi: (state.combine_mode >> 32) as u32,
        fill_color: state.fill_color,
        prim_color: state.prim_color,
        env_color: state.env_color,
        fog_color: state.fog_color,
        blend_color: state.blend_color,
        prim_lod_frac: u32::from(state.prim_lod_frac),
        prim_z: u32::from(state.prim_z),
        prim_dz: u32::from(state.prim_dz),
        color_addr: img.addr,
        color_width_px: u32::from(img.width) + 1,
        color_size: u32::from(img.size),
        color_format: u32::from(img.format),
        depth_addr: state.depth_image_addr,
        scissor_ulx: u32::from(state.scissor.ulx),
        scissor_uly: u32::from(state.scissor.uly),
        scissor_lrx: u32::from(state.scissor.lrx),
        scissor_lry: u32::from(state.scissor.lry),
        scissor_field: u32::from(state.scissor.field),
        scissor_odd: u32::from(state.scissor.odd),
        convert_k4: u32::from(state.convert_k4),
        convert_k5: u32::from(state.convert_k5),
        key_center_rgb,
        key_scale_rgb,
        ..Default::default()
    };

    let (bbox_x0, bbox_y0, bbox_x1, bbox_y1) = match cmd_id {
        0x36 => {
            pack.prim_kind = PRIM_KIND_FILL_RECT;
            let (ulx, uly, lrx, lry) = rect_coords(word);
            pack.rect_ulx = ulx;
            pack.rect_uly = uly;
            pack.rect_lrx = lrx;
            pack.rect_lry = lry;
            if fill_like {
                let x0 = ulx >> 2;
                let y0 = uly >> 2;
                let x1 = ceil_10_2(lrx);
                let y1 = ceil_10_2(lry);
                if x0 > x1 || y0 > y1 {
                    return Ok(None);
                }
                (x0, y0, x1 + 1, y1 + 1)
            } else {
                (ulx >> 2, uly >> 2, (lrx >> 2) + 1, (lry >> 2) + 1)
            }
        }
        0x24 | 0x25 => {
            pack.prim_kind = if cmd_id == 0x25 {
                PRIM_KIND_TEX_RECT_FLIP
            } else {
                PRIM_KIND_TEX_RECT
            };
            let (ulx, uly, lrx, lry) = rect_coords(word);
            let tile = ((word >> 24) & 7) as u8;
            let w1 = words[1];
            let s = i16_lane(w1, 48);
            let t = i16_lane(w1, 32);
            let dsdx = i16_lane(w1, 16);
            let dtdy = i16_lane(w1, 0);
            pack.rect_ulx = ulx;
            pack.rect_uly = uly;
            pack.rect_lrx = lrx;
            pack.rect_lry = lry;
            pack.rect_s = s;
            pack.rect_t = t;
            pack.rect_dsdx = dsdx;
            pack.rect_dtdy = dtdy;
            pack.prim_flags |= PRIM_FLAG_TEXTURED | (u32::from(tile) << PRIM_FLAG_TILE_SHIFT);

            if fill_like {
                let x0 = ulx >> 2;
                let y0 = uly >> 2;
                let x1 = ceil_10_2(lrx);
                let y1 = ceil_10_2(lry);
                if x0 > x1 || y0 > y1 {
                    return Ok(None);
                }
                (x0, y0, x1 + 1, y1 + 1)
            } else {
                let bbox = (ulx >> 2, uly >> 2, (lrx >> 2) + 1, (lry >> 2) + 1);
                validate_rect_texture_samples(state, &pack, tile, cmd_id == 0x25, fill_like)?;
                bbox
            }
        }
        0x08..=0x0F => prepare_triangle(state, words, fill_like, &mut pack)?,
        _ => return Err(Trap::UnimplementedRdpCommand { cmd_id, word }),
    };

    // Clip dispatch box to screen-positive region overlapping the scissor bounds.
    let aa = (state.other_modes & 8) != 0 && !fill_like;
    let (sc_x0, sc_y0, sc_x1, sc_y1) = if aa {
        (
            (i32::from(state.scissor.ulx) >> 2).max(0),
            (i32::from(state.scissor.uly) >> 2).max(0),
            ceil_10_2(i32::from(state.scissor.lrx)).max(0) + 1,
            ceil_10_2(i32::from(state.scissor.lry)).max(0) + 1,
        )
    } else if fill_like {
        (
            (i32::from(state.scissor.ulx) >> 2).max(0),
            (i32::from(state.scissor.uly) >> 2).max(0),
            (i32::from(state.scissor.lrx) >> 2).max(0) + 1,
            (i32::from(state.scissor.lry) >> 2).max(0),
        )
    } else {
        (
            (i32::from(state.scissor.ulx) >> 2).max(0),
            (i32::from(state.scissor.uly) >> 2).max(0),
            (i32::from(state.scissor.lrx) >> 2).max(0) + 1,
            (i32::from(state.scissor.lry) >> 2).max(0) + 1,
        )
    };

    let x0 = bbox_x0.max(sc_x0).max(0);
    let y0 = bbox_y0.max(sc_y0).max(0);
    let x1 = bbox_x1.min(sc_x1).clamp(0, 4096);
    let y1 = bbox_y1.min(sc_y1).clamp(0, 4096);

    if x0 >= x1 || y0 >= y1 {
        return Ok(None);
    }

    // Only trap on invalid color/depth targets if pixels may actually be visited inside scissor.
    validate_target_alignment(
        img,
        state.depth_image_addr,
        state.other_modes,
        fill_like,
        cmd_id,
        word,
    )?;

    pack.bbox_x0 = x0;
    pack.bbox_y0 = y0;
    pack.bbox_w = (x1 - x0) as u32;
    pack.bbox_h = (y1 - y0) as u32;

    let threads = pack
        .bbox_w
        .checked_mul(pack.bbox_h)
        .ok_or(Trap::UnimplementedRdpCommand { cmd_id, word })?;
    Ok(Some((pack, threads)))
}

fn validate_target_alignment(
    img: RdpImageState,
    depth_addr: u32,
    other_modes: u64,
    fill_like: bool,
    cmd_id: u8,
    word: u64,
) -> Result<(), Trap> {
    if img.size == 0 && fill_like {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: 0x36,
            word: u64::from(0x36u8) << 56,
        });
    }
    if matches!(img.size, 2 | 3) && img.format != 0 && !fill_like {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: 0x3F,
            word: 0,
        });
    }
    if img.size > 3 {
        return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
    }
    if img.size == 2 && (img.addr & 1) != 0 {
        return Err(Trap::MisalignedAccess {
            pa: img.addr,
            size: 2,
        });
    }
    if img.size == 3 && (img.addr & 3) != 0 {
        return Err(Trap::MisalignedAccess {
            pa: img.addr,
            size: 4,
        });
    }
    if !fill_like && (other_modes & ((1 << 4) | (1 << 5))) != 0 && (depth_addr & 1) != 0 {
        return Err(Trap::MisalignedAccess {
            pa: depth_addr,
            size: 2,
        });
    }
    Ok(())
}

fn prepare_triangle(
    state: &HostRasterState<'_>,
    words: &[u64],
    fill_like: bool,
    pack: &mut RasterParamsPack,
) -> Result<(i32, i32, i32, i32), Trap> {
    let w0 = words[0];
    let cmd = ((w0 >> 56) & 0x3F) as u8;
    let lmajor = (w0 & (1 << 55)) != 0;
    if fill_like && !lmajor {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: cmd,
            word: w0,
        });
    }
    pack.prim_kind = PRIM_KIND_TRIANGLE;
    if lmajor {
        pack.prim_flags |= PRIM_FLAG_LMAJOR;
    }
    let tile = ((w0 >> 48) & 7) as u8;
    pack.prim_flags |= u32::from(tile) << PRIM_FLAG_TILE_SHIFT;

    let yl = sext14((w0 >> 32) as u32);
    let ym = sext14((w0 >> 16) as u32);
    let yh = sext14(w0 as u32);
    let (xl, dxldy) = parse_edge(words[1]);
    let (xh, dxhdy) = parse_edge(words[2]);
    let (xm, dxmdy) = parse_edge(words[3]);

    pack.yh = yh;
    pack.ym = ym;
    pack.yl = yl;
    pack.xh = xh as i32;
    pack.xm = xm as i32;
    pack.xl = xl as i32;
    pack.dxhdy = dxhdy as i32;
    pack.dxmdy = dxmdy as i32;
    pack.dxldy = dxldy as i32;

    let mut idx = 4usize;
    if (cmd & 0b100) != 0 {
        pack.prim_flags |= PRIM_FLAG_SHADED;
        pack.shade_r = sext_fixed(
            ((words[idx] >> 48) & 0x1FF) as u32,
            9,
            lane16(words[idx + 2], 48),
        ) as i32;
        pack.shade_g = sext_fixed(
            ((words[idx] >> 32) & 0x1FF) as u32,
            9,
            lane16(words[idx + 2], 32),
        ) as i32;
        pack.shade_b = sext_fixed(
            ((words[idx] >> 16) & 0x1FF) as u32,
            9,
            lane16(words[idx + 2], 16),
        ) as i32;
        pack.shade_a = sext_fixed((words[idx] & 0x1FF) as u32, 9, lane16(words[idx + 2], 0)) as i32;
        pack.drdx_r = s16_16(lane16(words[idx + 1], 48), lane16(words[idx + 3], 48)) as i32;
        pack.drdx_g = s16_16(lane16(words[idx + 1], 32), lane16(words[idx + 3], 32)) as i32;
        pack.drdx_b = s16_16(lane16(words[idx + 1], 16), lane16(words[idx + 3], 16)) as i32;
        pack.drdx_a = s16_16(lane16(words[idx + 1], 0), lane16(words[idx + 3], 0)) as i32;
        pack.drde_r = s16_16(lane16(words[idx + 4], 48), lane16(words[idx + 6], 48)) as i32;
        pack.drde_g = s16_16(lane16(words[idx + 4], 32), lane16(words[idx + 6], 32)) as i32;
        pack.drde_b = s16_16(lane16(words[idx + 4], 16), lane16(words[idx + 6], 16)) as i32;
        pack.drde_a = s16_16(lane16(words[idx + 4], 0), lane16(words[idx + 6], 0)) as i32;
        idx += 8;
    }

    if (cmd & 0b010) != 0 {
        pack.prim_flags |= PRIM_FLAG_TEXTURED;
        let anchor = words[idx];
        let dx = words[idx + 1];
        let de = words[idx + 4];
        pack.tex_s = i16_lane(anchor, 48);
        pack.tex_t = i16_lane(anchor, 32);
        pack.tex_dsdx = i16_lane(dx, 48);
        pack.tex_dtdx = i16_lane(dx, 32);
        pack.tex_dsde = i16_lane(de, 48);
        pack.tex_dtde = i16_lane(de, 32);
        idx += 8;
    }

    if (cmd & 0b001) != 0 {
        pack.prim_flags |= PRIM_FLAG_HAS_Z;
        let zw = words[idx];
        let dw = words[idx + 1];
        pack.z_anchor = s16_16(lane16(zw, 48), lane16(zw, 32)) as i32;
        pack.dzdx = s16_16(lane16(zw, 16), lane16(zw, 0)) as i32;
        pack.dzde = s16_16(lane16(dw, 48), lane16(dw, 32)) as i32;
    }

    let y0 = yh >> 2;
    let y1 = div_ceil_4(yl);
    let y_mid = ym >> 2;
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut x_major = xh;
    let mut x_minor = xm;
    let mut on_lower = false;

    let aa = (state.other_modes & 8) != 0 && !fill_like;
    let textured = (cmd & 0b010) != 0 && !fill_like;
    let mut tex_s = pack.tex_s;
    let mut tex_t = pack.tex_t;
    let tile_desc = &state.tiles[usize::from(tile)];

    for y in y0..y1 {
        if !on_lower && y >= y_mid {
            x_minor = xl;
            on_lower = true;
        }
        let (left, right) = if lmajor {
            (x_major, x_minor)
        } else {
            (x_minor, x_major)
        };
        if left < right {
            let (xs, xe) = if aa {
                (
                    ((left.saturating_sub(1) >> 16) as i32).clamp(0, 4096),
                    (div_ceil_i64(right, 0x1_0000) as i32).clamp(0, 4096),
                )
            } else {
                (
                    (div_ceil_i64(left - 0x8000, 0x1_0000) as i32).clamp(0, 4096),
                    (div_ceil_i64(right - 0x8000, 0x1_0000) as i32).clamp(0, 4096),
                )
            };
            if xs < xe {
                min_x = min_x.min(xs);
                max_x = max_x.max(xe);
                if textured && y >= 0 {
                    validate_tri_scanline_texture(
                        state,
                        tile_desc,
                        tile,
                        xs,
                        xe,
                        y,
                        x_major,
                        tex_s,
                        tex_t,
                        pack.tex_dsdx,
                        pack.tex_dtdx,
                        aa,
                    )?;
                }
            }
        }
        let minor_step = if on_lower { dxldy } else { dxmdy };
        x_major += dxhdy;
        x_minor += minor_step;
        tex_s = tex_s.wrapping_add(pack.tex_dsde);
        tex_t = tex_t.wrapping_add(pack.tex_dtde);
    }

    if min_x >= max_x || y0 >= y1 {
        return Ok((0, 0, 0, 0));
    }
    Ok((min_x, y0, max_x, y1))
}

#[allow(clippy::too_many_arguments)]
fn validate_tri_scanline_texture(
    state: &HostRasterState<'_>,
    tile_desc: &TileDesc,
    tile: u8,
    xs: i32,
    xe: i32,
    y: i32,
    x_major: i64,
    tex_s: i32,
    tex_t: i32,
    dsdx: i32,
    dtdx: i32,
    aa: bool,
) -> Result<(), Trap> {
    let field_ok = !state.scissor.field || ((y & 1) == 1) == state.scissor.odd;
    if !field_ok {
        return Ok(());
    }
    let xm_px = (x_major >> 16) as i32;
    let sc_ulx = i32::from(state.scissor.ulx);
    let sc_uly = i32::from(state.scissor.uly);
    let sc_lrx = i32::from(state.scissor.lrx);
    let sc_lry = i32::from(state.scissor.lry);
    for x in xs..xe {
        let in_sc = if aa {
            x * 4 < sc_lrx && x * 4 + 4 > sc_ulx && y * 4 < sc_lry && y * 4 + 4 > sc_uly
        } else {
            let cx = x * 4 + 2;
            let cy = y * 4 + 2;
            cx >= sc_ulx && cx < sc_lrx && cy >= sc_uly && cy < sc_lry
        };
        if !in_sc {
            continue;
        }
        let dx = x.wrapping_sub(xm_px);
        let s = tex_s.wrapping_add(dsdx.wrapping_mul(dx));
        let t = tex_t.wrapping_add(dtdx.wrapping_mul(dx));
        validate_tile_sample(tile_desc, tile, s, t)?;
    }
    Ok(())
}

fn validate_rect_texture_samples(
    state: &HostRasterState<'_>,
    pack: &RasterParamsPack,
    tile: u8,
    flip: bool,
    fill_like: bool,
) -> Result<(), Trap> {
    if fill_like {
        return Ok(());
    }
    let tile_desc = &state.tiles[usize::from(tile)];
    let aa = (state.other_modes & 8) != 0;
    let ulx = pack.rect_ulx;
    let uly = pack.rect_uly;
    let lrx = pack.rect_lrx;
    let lry = pack.rect_lry;
    let x0 = (ulx >> 2).max(i32::from(state.scissor.ulx) >> 2).max(0);
    let y0 = (uly >> 2).max(i32::from(state.scissor.uly) >> 2).max(0);
    let x1 = ((lrx >> 2) + 1).min((i32::from(state.scissor.lrx) >> 2) + 1);
    let y1 = ((lry >> 2) + 1).min((i32::from(state.scissor.lry) >> 2) + 1);

    for y in y0..y1 {
        if state.scissor.field && ((y & 1) == 1) != state.scissor.odd {
            continue;
        }
        for x in x0..x1 {
            let cx = x * 4 + 2;
            let cy = y * 4 + 2;
            if !aa
                && (cx < ulx
                    || cx >= lrx
                    || cy < uly
                    || cy >= lry
                    || cx < i32::from(state.scissor.ulx)
                    || cx >= i32::from(state.scissor.lrx)
                    || cy < i32::from(state.scissor.uly)
                    || cy >= i32::from(state.scissor.lry))
            {
                continue;
            }
            let dx = x - (ulx >> 2);
            let dy = y - (uly >> 2);
            let (s_step, t_step) = if flip {
                (
                    i64::from(pack.rect_dsdx) * i64::from(dy),
                    i64::from(pack.rect_dtdy) * i64::from(dx),
                )
            } else {
                (
                    i64::from(pack.rect_dsdx) * i64::from(dx),
                    i64::from(pack.rect_dtdy) * i64::from(dy),
                )
            };
            let s = pack.rect_s.wrapping_add((s_step >> 5) as i32);
            let t = pack.rect_t.wrapping_add((t_step >> 5) as i32);
            validate_tile_sample(tile_desc, tile, s, t)?;
        }
    }
    Ok(())
}

fn rect_coords(word: u64) -> (i32, i32, i32, i32) {
    let ulx = ((word >> 12) & 0xFFF) as i32;
    let uly = (word & 0xFFF) as i32;
    let lrx = ((word >> 44) & 0xFFF) as i32;
    let lry = ((word >> 32) & 0xFFF) as i32;
    (ulx, uly, lrx, lry)
}

fn parse_edge(word: u64) -> (i64, i64) {
    let x = sext_fixed(((word >> 48) & 0xFFF) as u32, 12, lane16(word, 32));
    let dx = sext_fixed(((word >> 16) & 0x3FFF) as u32, 14, lane16(word, 0));
    (x, dx)
}

fn i16_lane(word: u64, shift: u32) -> i32 {
    i32::from(((word >> shift) as u16) as i16)
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

fn ceil_10_2(v: i32) -> i32 {
    let i = v >> 2;
    if v & 3 == 0 {
        i
    } else {
        i + 1
    }
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

#[cfg(test)]
#[path = "raster_gpu_tests.rs"]
mod tests;
