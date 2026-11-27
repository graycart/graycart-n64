//! Edge-walker tests. Command words follow n64brew RDP command layouts.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use super::{
    draw_fill_rectangle, draw_primitive, draw_texture_rectangle, draw_triangle,
    primitive_word_count, read_coverage, read_depth, set_primitive_depth, unimplemented_primitive,
    write_depth,
};
use crate::bus::Rdram;
use crate::rdp::tmem::TileDesc;
use crate::rdp::{CpuRdpLle, RdpImageState, RdpScissor};
use crate::trap::Trap;

const W: u32 = 16;

fn rdram() -> Rdram {
    Rdram::new(false)
}

fn image(size: u8, addr: u32) -> RdpImageState {
    RdpImageState {
        format: 0,
        size,
        width: (W - 1) as u16,
        addr,
    }
}

/// 1-cycle / 2-cycle scissor: exclusive right and bottom, origin at 0.
fn scissor_exclusive(pixels: u32) -> RdpScissor {
    let edge = (pixels << 2) as u16;
    RdpScissor {
        ulx: 0,
        uly: 0,
        lrx: edge,
        lry: edge,
        field: false,
        odd: false,
    }
}

/// FILL/COPY scissor: inclusive right, exclusive bottom.
fn scissor_fill(pixels: u32) -> RdpScissor {
    RdpScissor {
        ulx: 0,
        uly: 0,
        lrx: ((pixels - 1) << 2) as u16,
        lry: (pixels << 2) as u16,
        field: false,
        odd: false,
    }
}

fn modes(cycle: u8) -> u64 {
    // Dither disabled (selector 3) so 8→5 quantization is a plain shift.
    (u64::from(cycle) << 52) | (3 << 38)
}

fn rect_word(cmd: u8, lrx: u32, lry: u32, ulx: u32, uly: u32) -> u64 {
    (u64::from(cmd) << 56)
        | (u64::from(lrx & 0xFFF) << 44)
        | (u64::from(lry & 0xFFF) << 32)
        | (u64::from(ulx & 0xFFF) << 12)
        | u64::from(uly & 0xFFF)
}

fn tri_header(cmd: u8, lmajor: bool, yl: u32, ym: u32, yh: u32) -> u64 {
    let mut w = u64::from(cmd) << 56;
    if lmajor {
        w |= 1 << 55;
    }
    w |= u64::from(yl & 0x3FFF) << 32;
    w |= u64::from(ym & 0x3FFF) << 16;
    w |= u64::from(yh & 0x3FFF);
    w
}

/// Edge coefficient. `x_px` / `dx_px` are integer pixels (fraction 0).
fn edge(x_px: i32, dx_px: i32) -> u64 {
    let xi = (x_px as u32) & 0xFFF;
    let di = (dx_px as u32) & 0x3FFF;
    (u64::from(xi) << 48) | (u64::from(di) << 16)
}

fn shade_words(r: u32, drdx: u32) -> [u64; 8] {
    let color = u64::from(r & 0x1FF) << 48;
    let slope = u64::from(drdx) << 48;
    [color, slope, 0, 0, 0, 0, 0, 0]
}

fn z_words(z: u32) -> [u64; 2] {
    let zi = u64::from(z >> 16);
    let zf = u64::from(z & 0xFFFF);
    [(zi << 48) | (zf << 32), 0]
}

fn addr16(x: u32, y: u32) -> u32 {
    (y * W + x) * 2
}

fn addr32(x: u32, y: u32) -> u32 {
    (y * W + x) * 4
}

fn px16(ram: &Rdram, x: u32, y: u32) -> u16 {
    ram.read_u16(addr16(x, y)).unwrap()
}

fn px32(ram: &Rdram, x: u32, y: u32) -> u32 {
    ram.read_u32(addr32(x, y)).unwrap()
}

#[test]
fn primitive_shell_traps() {
    let err = unimplemented_primitive(0x36, 0x3600_0000_0000_0000).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. }
    ));
}

#[test]
fn word_counts_follow_opcode_flags() {
    assert_eq!(primitive_word_count(0x08 << 56), 4);
    assert_eq!(primitive_word_count(0x09 << 56), 6);
    assert_eq!(primitive_word_count(0x0A << 56), 12);
    assert_eq!(primitive_word_count(0x0C << 56), 12);
    assert_eq!(primitive_word_count(0x0F << 56), 22);
    assert_eq!(primitive_word_count(0x24 << 56), 2);
    assert_eq!(primitive_word_count(0x36 << 56), 1);
}

#[test]
fn fill_rect_rgba16_even_odd_and_scissor() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(2, 0);
    rdp.scissor = scissor_fill(W);
    rdp.scissor.lrx = 3 << 2;
    rdp.other_modes = modes(3);
    rdp.fill_color = 0x1111_2222;
    let mut ram = rdram();
    // Inclusive FILL bounds: last pixel is x = 10, but scissor stops at x = 3.
    let word = rect_word(0x36, 10 << 2, 0, 0, 0);
    draw_fill_rectangle(&mut rdp, word, &mut ram).unwrap();
    assert_eq!(px16(&ram, 0, 0), 0x1111);
    assert_eq!(px16(&ram, 1, 0), 0x2222);
    assert_eq!(px16(&ram, 2, 0), 0x1111);
    assert_eq!(px16(&ram, 3, 0), 0x2222);
    assert_eq!(px16(&ram, 4, 0), 0);
    assert!(!ram.read_hidden_bit(0));
    assert!(rdp.span.valid);
}

#[test]
fn fill_rect_1cycle_is_exclusive_and_uses_prim_color() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    rdp.prim_color = 0x1122_3344;
    let mut ram = rdram();
    // 4.0 x 2.0 exclusive → pixels x = 0..3, y = 0..1.
    let word = rect_word(0x36, 4 << 2, 2 << 2, 0, 0);
    assert_eq!(draw_primitive(&mut rdp, &[word], &mut ram).unwrap(), 1);
    let px = px32(&ram, 3, 1);
    assert_eq!(px >> 24, 0x11);
    assert_eq!((px >> 16) & 0xFF, 0x22);
    assert_eq!((px >> 8) & 0xFF, 0x33);
    // Coverage 7 lives in alpha[7:5]; prim alpha 0x44 keeps its top 5 bits.
    assert_eq!(px & 0xFF, (7 << 5) | (0x44 >> 3));
    assert_eq!(px32(&ram, 4, 0), 0);
    assert_eq!(px32(&ram, 0, 2), 0);
    assert!(ram.read_hidden_bit(addr32(3, 1) as usize));
    assert!(ram.read_hidden_bit(addr32(3, 1) as usize + 1));
    assert!(ram.read_hidden_bit(addr32(3, 1) as usize + 2));
    assert!(!ram.read_hidden_bit(addr32(3, 1) as usize + 3));
}

#[test]
fn fill_rect_rgba8_repeats_four_bytes() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(1, 0);
    rdp.scissor = scissor_fill(W);
    rdp.other_modes = modes(3);
    rdp.fill_color = 0xA1B2_C3D4;
    let mut ram = rdram();
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 4 << 2, 0, 0, 0), &mut ram).unwrap();
    assert_eq!(ram.read_u8(0), 0xA1);
    assert_eq!(ram.read_u8(1), 0xB2);
    assert_eq!(ram.read_u8(2), 0xC3);
    assert_eq!(ram.read_u8(3), 0xD4);
    assert_eq!(ram.read_u8(4), 0xA1);
}

#[test]
fn fill_rect_4bit_and_copy_and_alpha_compare_trap() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(0, 0);
    rdp.scissor = scissor_fill(W);
    rdp.other_modes = modes(3);
    let mut ram = rdram();
    let word = rect_word(0x36, 4, 0, 0, 0);
    let err = draw_fill_rectangle(&mut rdp, word, &mut ram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. }
    ));

    rdp.color_image = image(2, 0);
    rdp.other_modes = modes(2);
    let err = draw_fill_rectangle(&mut rdp, word, &mut ram).unwrap_err();
    assert!(matches!(err, Trap::UnimplementedRdpCommand { .. }));

    rdp.other_modes = modes(0) | 1;
    let err = draw_fill_rectangle(&mut rdp, word, &mut ram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. }
    ));
}

#[test]
fn interlace_scissor_keeps_only_odd_lines() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(2, 0);
    rdp.scissor = scissor_fill(W);
    rdp.scissor.field = true;
    rdp.scissor.odd = true;
    rdp.other_modes = modes(3);
    rdp.fill_color = 0x7C1F_7C1F;
    let mut ram = rdram();
    // y = 0 and y = 1 inclusive.
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 0, 1 << 2, 0, 0), &mut ram).unwrap();
    assert_eq!(px16(&ram, 0, 0), 0);
    assert_eq!(px16(&ram, 0, 1), 0x7C1F);
}

#[test]
fn triangle_quad_matches_1cycle_rect() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    rdp.prim_color = 0x00FF_0000;
    let mut ram = rdram();
    let rect = rect_word(0x36, 4 << 2, 2 << 2, 0, 0);
    draw_fill_rectangle(&mut rdp, rect, &mut ram).unwrap();

    let mut rdp_t = CpuRdpLle::new();
    rdp_t.color_image = image(3, 0x100);
    rdp_t.scissor = scissor_exclusive(W);
    rdp_t.other_modes = modes(0);
    rdp_t.prim_color = 0x00FF_0000;
    // Left-major, zero slopes, exclusive box [0, 4) x [0, 2).
    let words = [
        tri_header(0x08, true, 2 << 2, 2 << 2, 0),
        edge(4, 0),
        edge(0, 0),
        edge(4, 0),
    ];
    assert_eq!(draw_triangle(&mut rdp_t, &words, &mut ram).unwrap(), 4);
    for y in 0..2 {
        for x in 0..4 {
            let from_rect = px32(&ram, x, y);
            let from_tri = ram.read_u32(0x100 + addr32(x, y)).unwrap();
            assert_eq!(from_tri, from_rect, "pixel {x},{y}");
        }
    }
    assert_eq!(ram.read_u32(0x100 + addr32(4, 0)).unwrap(), 0);
    assert!(rdp_t.span.valid);
    assert_eq!(rdp_t.span.y, 1);
}

#[test]
fn shaded_triangle_ramps_red_along_x() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    let mut words = [0u64; 12];
    words[0] = tri_header(0x0C, true, 1 << 2, 1 << 2, 0);
    words[1] = edge(3, 0);
    words[2] = edge(0, 0);
    words[3] = edge(3, 0);
    words[4..12].copy_from_slice(&shade_words(0, 32));
    let mut ram = rdram();
    draw_primitive(&mut rdp, &words, &mut ram).unwrap();
    assert_eq!(px32(&ram, 0, 0) >> 24, 0);
    assert_eq!(px32(&ram, 1, 0) >> 24, 32);
    assert_eq!(px32(&ram, 2, 0) >> 24, 64);
    assert_eq!(px32(&ram, 3, 0), 0);
}

#[test]
fn major_edge_steps_and_right_major_covers_the_same_span() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    rdp.prim_color = 0x0100_0000;
    let mut ram = rdram();
    // Major X starts at 0 and moves +1 pixel per scanline. Minor stays at x = 8.
    let slant = [
        tri_header(0x08, true, 4 << 2, 4 << 2, 0),
        edge(8, 0),
        edge(0, 1),
        edge(8, 0),
    ];
    draw_triangle(&mut rdp, &slant, &mut ram).unwrap();
    assert_eq!(rdp.span.y, 3);
    assert_eq!(rdp.span.x_major, 3 << 16);
    assert_ne!(px32(&ram, 0, 0), 0);
    assert_eq!(px32(&ram, 0, 3), 0);
    assert_ne!(px32(&ram, 3, 3), 0);

    // Negative slope: 14-bit −1 encoded in the major-edge derivative.
    let neg = [
        tri_header(0x08, true, 2 << 2, 2 << 2, 0),
        edge(0, 0),
        edge(4, -1),
        edge(0, 0),
    ];
    draw_triangle(&mut rdp, &neg, &mut ram).unwrap();
    assert_eq!(rdp.span.y, 1);
    assert_eq!(rdp.span.x_major, 3 << 16);

    let right = [
        tri_header(0x08, false, 1 << 2, 1 << 2, 0),
        edge(0, 0),
        edge(4, 0),
        edge(0, 0),
    ];
    let mut ram_right = rdram();
    draw_triangle(&mut rdp, &right, &mut ram_right).unwrap();
    assert_ne!(px32(&ram_right, 0, 0), 0);
    assert_ne!(px32(&ram_right, 3, 0), 0);
    assert_eq!(px32(&ram_right, 4, 0), 0);
}

#[test]
fn z_opaque_keeps_nearer_pixel_and_hidden_low_bits() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.depth_image_addr = 0x2000;
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0) | (1 << 5) | (1 << 4);
    let mut ram = rdram();
    for x in 0..4 {
        write_depth(&mut ram, 0x2000 + x * 2, 0x3_FFFF).unwrap();
    }
    let mut near = [0u64; 14];
    near[0] = tri_header(0x0D, true, 1 << 2, 1 << 2, 0);
    near[1] = edge(2, 0);
    near[2] = edge(0, 0);
    near[3] = edge(2, 0);
    near[4..12].copy_from_slice(&shade_words(0x10, 0));
    // 0x6000 >> 13 = 3, so the 18-bit value's low bits land in the hidden pair.
    near[12..14].copy_from_slice(&z_words(0x6000));
    draw_triangle(&mut rdp, &near, &mut ram).unwrap();

    let mut far = near;
    far[4..12].copy_from_slice(&shade_words(0x20, 0));
    far[12..14].copy_from_slice(&z_words(0x18_0000));
    draw_triangle(&mut rdp, &far, &mut ram).unwrap();

    assert_eq!(px32(&ram, 0, 0) >> 24, 0x10);
    assert_eq!(read_depth(&ram, 0x2000).unwrap(), 3);
    assert_eq!(ram.read_u16(0x2000).unwrap(), 0);
    assert!(ram.read_hidden_bit(0x2000));
    assert!(ram.read_hidden_bit(0x2001));
}

#[test]
fn primitive_z_decal_and_missing_per_pixel_z_traps() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.depth_image_addr = 0x2000;
    rdp.scissor = scissor_exclusive(W);
    // Decal, primitive Z, compare on, update off.
    rdp.other_modes = modes(0) | (3 << 10) | (1 << 4) | (1 << 2);
    set_primitive_depth(&mut rdp, (0x2Eu64 << 56) | (16 << 16) | 8).unwrap();
    assert_eq!(rdp.prim_z, 16);
    assert_eq!(rdp.prim_dz, 8);
    let bad = set_primitive_depth(&mut rdp, 0x36 << 56).unwrap_err();
    assert!(matches!(
        bad,
        Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. }
    ));

    let mut ram = rdram();
    write_depth(&mut ram, 0x2000, 128).unwrap();
    rdp.prim_color = 0x0000_FF00;
    let word = rect_word(0x36, 1 << 2, 1 << 2, 0, 0);
    draw_fill_rectangle(&mut rdp, word, &mut ram).unwrap();
    assert_ne!(px32(&ram, 0, 0), 0);
    assert_eq!(read_depth(&ram, 0x2000).unwrap(), 128);

    let mut ram_far = rdram();
    write_depth(&mut ram_far, 0x2000, 0).unwrap();
    draw_fill_rectangle(&mut rdp, word, &mut ram_far).unwrap();
    assert_eq!(px32(&ram_far, 0, 0), 0);

    rdp.other_modes = modes(0) | (1 << 4);
    let err = draw_fill_rectangle(&mut rdp, word, &mut ram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. }
    ));
}

#[test]
fn aa_coverage_partial_full_clamp_save_wrap_and_color_on_cvg() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(2, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0) | (1 << 3);
    rdp.prim_color = 0;
    let mut ram = rdram();
    // 1.5 x 1.0 exclusive: pixel 0 is full, pixel 1 is half in X.
    let partial = rect_word(0x36, 6, 4, 0, 0);
    draw_fill_rectangle(&mut rdp, partial, &mut ram).unwrap();
    assert_eq!(px16(&ram, 0, 0), 0x0001);
    assert_eq!(read_coverage(&ram, 0, 2), 7);
    assert!(ram.read_hidden_bit(0) && ram.read_hidden_bit(1));
    assert_eq!(px16(&ram, 1, 0), 0);
    assert_eq!(read_coverage(&ram, addr16(1, 0), 2), 3);
    assert!(ram.read_hidden_bit(addr16(1, 0) as usize));
    assert!(ram.read_hidden_bit(addr16(1, 0) as usize + 1));

    // cvg_dest = full forces 7 onto the partial pixel.
    ram = rdram();
    rdp.other_modes = modes(0) | (1 << 3) | (2 << 8);
    draw_fill_rectangle(&mut rdp, partial, &mut ram).unwrap();
    assert_eq!(read_coverage(&ram, addr16(1, 0), 2), 7);

    // Clamp: old 5 + full 7 → 7.
    ram = rdram();
    rdp.other_modes = modes(0) | (1 << 6);
    write_cvg16(&mut ram, 0, 5);
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 4, 4, 0, 0), &mut ram).unwrap();
    assert_eq!(read_coverage(&ram, 0, 2), 7);

    // Wrap: old 6 + full 7 → 5.
    ram = rdram();
    rdp.other_modes = modes(0) | (1 << 6) | (1 << 8);
    write_cvg16(&mut ram, 0, 6);
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 4, 4, 0, 0), &mut ram).unwrap();
    assert_eq!(read_coverage(&ram, 0, 2), 5);

    // Save keeps the preset coverage while the new color is stored.
    ram = rdram();
    rdp.other_modes = modes(0) | (1 << 6) | (3 << 8);
    rdp.prim_color = 0xFF00_0000;
    write_cvg16(&mut ram, 0, 3);
    ram.write_u16(0, 0x1110).unwrap();
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 4, 4, 0, 0), &mut ram).unwrap();
    assert_eq!(read_coverage(&ram, 0, 2), 3);
    assert_eq!(px16(&ram, 0, 0) & 0xF800, 0xF800);

    // color_on_cvg with no overflow leaves the previous color and writes coverage.
    ram = rdram();
    rdp.other_modes = modes(0) | (1 << 6) | (1 << 7);
    rdp.prim_color = 0xFF00_0000;
    ram.write_u16(0, 0x2220).unwrap();
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 4, 4, 0, 0), &mut ram).unwrap();
    assert_eq!(px16(&ram, 0, 0), 0x2221);
    assert_eq!(read_coverage(&ram, 0, 2), 7);
}

#[test]
fn texture_rect_samples_tmem_and_flip_swaps_the_derivative_axis() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    rdp.tiles[0] = TileDesc {
        format: 0,
        size: 2,
        line: 1,
        clamp_s: true,
        clamp_t: true,
        sh: 1 << 2,
        ..TileDesc::default()
    };
    // Texel 0 red, texel 1 green, both RGBA16.
    rdp.tmem[0] = 0xF8;
    rdp.tmem[1] = 0x00;
    rdp.tmem[2] = 0x07;
    rdp.tmem[3] = 0xC0;
    let mut ram = rdram();
    // dsdx = 1.0 in s5.10. Non-flip steps S along X, so pixel 1 is green.
    let deriv = 1024u64 << 16;
    let rect = rect_word(0x24, 2 << 2, 1 << 2, 0, 0);
    draw_texture_rectangle(&mut rdp, &[rect, deriv], &mut ram).unwrap();
    assert_eq!(px32(&ram, 0, 0) >> 24, 255);
    assert_eq!((px32(&ram, 1, 0) >> 16) & 0xFF, 255);
    assert_eq!(px32(&ram, 1, 0) >> 24, 0);

    // Flip applies dsdx along Y. One row stays on texel 0.
    let mut ram_flip = rdram();
    let flip = rect_word(0x25, 2 << 2, 1 << 2, 0, 0);
    draw_texture_rectangle(&mut rdp, &[flip, deriv], &mut ram_flip).unwrap();
    assert_eq!(px32(&ram_flip, 0, 0) >> 24, 255);
    assert_eq!(px32(&ram_flip, 1, 0) >> 24, 255);

    // FILL ignores the texel and stores the fill color.
    rdp.other_modes = modes(3);
    rdp.scissor = scissor_fill(W);
    rdp.color_image = image(2, 0);
    rdp.fill_color = 0xABCD_ABCD;
    let mut ram_fill = rdram();
    draw_texture_rectangle(
        &mut rdp,
        &[rect_word(0x24, 0, 0, 0, 0), deriv],
        &mut ram_fill,
    )
    .unwrap();
    assert_eq!(px16(&ram_fill, 0, 0), 0xABCD);
}

#[test]
fn unconfigured_tile_traps_before_a_write() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    let mut ram = rdram();
    ram.write_u8(0, 0x5A);
    let words = [
        tri_header(0x0A, true, 1 << 2, 1 << 2, 0),
        edge(2, 0),
        edge(0, 0),
        edge(2, 0),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let err = draw_triangle(&mut rdp, &words, &mut ram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x35, .. }
    ));
    assert_eq!(ram.read_u8(0), 0x5A);
}

#[test]
fn combine_mode_replaces_the_shade_passthrough() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    rdp.prim_color = 0x8000_0000;
    // 1-cycle RGB D = primitive. Alpha C = primitive alpha (not LOD, which traps).
    rdp.combine_mode = (3 << 6) | (3 << 18);
    let mut words = [0u64; 12];
    words[0] = tri_header(0x0C, true, 1 << 2, 1 << 2, 0);
    words[1] = edge(1, 0);
    words[2] = edge(0, 0);
    words[3] = edge(1, 0);
    words[4..12].copy_from_slice(&shade_words(0x10, 0));
    let mut ram = rdram();
    draw_primitive(&mut rdp, &words, &mut ram).unwrap();
    assert_eq!(px32(&ram, 0, 0) >> 24, 0x80);
}

#[test]
fn short_triangle_traps_without_writing() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(3, 0);
    rdp.scissor = scissor_exclusive(W);
    rdp.other_modes = modes(0);
    rdp.prim_color = 0xFFFF_FFFF;
    let mut ram = rdram();
    let words = [tri_header(0x0C, true, 4, 4, 0), 0, 0, 0];
    let err = draw_primitive(&mut rdp, &words, &mut ram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x0C, .. }
    ));
    assert_eq!(px32(&ram, 0, 0), 0);
}

#[test]
fn magic_square_dither_changes_neighboring_low_bits() {
    let mut rdp = CpuRdpLle::new();
    rdp.color_image = image(2, 0);
    rdp.scissor = scissor_exclusive(W);
    // Cycle 1, magic-square RGB dither (selector 0), AA off.
    rdp.other_modes = 0;
    rdp.prim_color = 0x0400_0000;
    let mut ram = rdram();
    draw_fill_rectangle(&mut rdp, rect_word(0x36, 2 << 2, 1 << 2, 0, 0), &mut ram).unwrap();
    assert_eq!(px16(&ram, 0, 0), 0x0001);
    assert_eq!(px16(&ram, 1, 0), 0x0801);
}

fn write_cvg16(ram: &mut Rdram, addr: u32, cvg: u8) {
    let cvg = cvg & 7;
    let px = u16::from(cvg >> 2);
    ram.write_u16(addr, px).unwrap();
    ram.write_hidden_bit(addr as usize, (cvg & 2) != 0);
    ram.write_hidden_bit(addr as usize + 1, (cvg & 1) != 0);
}
