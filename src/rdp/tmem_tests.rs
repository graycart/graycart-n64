use super::*;
use crate::bus::Rdram;
use crate::rdp::{CpuRdpLle, RdpImageState};
use crate::trap::Trap;

fn q10(n: u16) -> u16 {
    n << 2
}

fn rect_word(op: u8, uls: u16, ult: u16, tile: u8, lrs: u16, lrt: u16) -> u64 {
    (u64::from(op) << 56)
        | (u64::from(uls) << 44)
        | (u64::from(ult) << 32)
        | (u64::from(tile) << 24)
        | (u64::from(lrs) << 12)
        | u64::from(lrt)
}

/// S or T clamp / mirror / mask / shift fields of a Set Tile word.
#[derive(Clone, Copy)]
struct TileAxis {
    clamp: bool,
    mirror: bool,
    mask: u8,
    shift: u8,
}

fn tile_axis(clamp: bool, mirror: bool, mask: u8, shift: u8) -> TileAxis {
    TileAxis {
        clamp,
        mirror,
        mask,
        shift,
    }
}

fn set_tile_word(
    format: u8,
    size: u8,
    line: u16,
    addr: u16,
    index: u8,
    palette: u8,
    axes: (TileAxis, TileAxis),
) -> u64 {
    let (t, s) = axes;
    let mut word = u64::from(CMD_SET_TILE) << 56;
    word |= u64::from(format) << 53;
    word |= u64::from(size) << 51;
    word |= u64::from(line) << 41;
    word |= u64::from(addr) << 32;
    word |= u64::from(index) << 24;
    word |= u64::from(palette) << 20;
    if t.clamp {
        word |= 1 << 19;
    }
    if t.mirror {
        word |= 1 << 18;
    }
    word |= u64::from(t.mask) << 14;
    word |= u64::from(t.shift) << 10;
    if s.clamp {
        word |= 1 << 9;
    }
    if s.mirror {
        word |= 1 << 8;
    }
    word |= u64::from(s.mask) << 4;
    word |= u64::from(s.shift);
    word
}

fn set_image(rdp: &mut CpuRdpLle, size: u8, width_m1: u16, addr: u32) {
    rdp.texture_image = RdpImageState {
        format: 0,
        size,
        width: width_m1,
        addr,
    };
}

fn write_pixels(rdram: &mut Rdram, addr: u32, pixels: &[u16]) {
    for (i, px) in pixels.iter().enumerate() {
        rdram
            .write_u16(addr + (i as u32) * 2, *px)
            .expect("pixel store");
    }
}

fn configure_tile(rdp: &mut CpuRdpLle, index: u8, format: u8, size: u8, line: u16, addr: u16) {
    exec_set_tile(
        rdp,
        set_tile_word(
            format,
            size,
            line,
            addr,
            index,
            0,
            (tile_axis(false, false, 0, 0), tile_axis(false, false, 0, 0)),
        ),
    )
    .expect("set tile");
}

#[test]
fn clear_tmem_zeros_buffer() {
    let mut rdp = CpuRdpLle::new();
    rdp.tmem[0] = 0xFF;
    clear_tmem(&mut rdp);
    assert!(rdp.tmem.iter().all(|&b| b == 0));
    assert_eq!(rdp.tiles.len(), TILE_COUNT);
}

#[test]
fn set_tile_decodes_fields_and_keeps_extents() {
    let mut rdp = CpuRdpLle::new();
    exec_set_tile_size(
        &mut rdp,
        rect_word(CMD_SET_TILE_SIZE, q10(1), q10(2), 3, q10(9), q10(8)),
    )
    .unwrap();
    let word = set_tile_word(
        2,
        0,
        0x1AB,
        0x100,
        3,
        0xA,
        (
            tile_axis(true, false, 0x5, 0xC),
            tile_axis(false, true, 0x3, 0xF),
        ),
    );
    // Bit 50 sits in the documented gap between size and line and must not join `line`.
    exec_set_tile(&mut rdp, word | (1 << 50)).unwrap();
    let tile = &rdp.tiles[3];
    assert_eq!(tile.format, 2);
    assert_eq!(tile.size, 0);
    assert_eq!(tile.line, 0x1AB);
    assert_eq!(tile.tmem_addr, 0x100);
    assert_eq!(tile.palette, 0xA);
    assert!(tile.clamp_t);
    assert!(!tile.mirror_t);
    assert_eq!(tile.mask_t, 0x5);
    assert_eq!(tile.shift_t, 0xC);
    assert!(!tile.clamp_s);
    assert!(tile.mirror_s);
    assert_eq!(tile.mask_s, 0x3);
    assert_eq!(tile.shift_s, 0xF);
    assert_eq!(tile.sl, q10(1));
    assert_eq!(tile.tl, q10(2));
    assert_eq!(tile.sh, q10(9));
    assert_eq!(tile.th, q10(8));
    assert_eq!(rdp.tiles[0].format, 0);
}

#[test]
fn set_tile_rejects_wrong_opcode() {
    let mut rdp = CpuRdpLle::new();
    let word = rect_word(CMD_LOAD_TILE, 0, 0, 0, 0, 0);
    let err = exec_set_tile(&mut rdp, word).unwrap_err();
    assert_eq!(
        err,
        Trap::UnimplementedRdpCommand {
            cmd_id: CMD_LOAD_TILE,
            word
        }
    );
}

#[test]
fn unimplemented_tmem_op_stays_a_trap() {
    let word = u64::from(CMD_LOAD_TILE) << 56;
    assert_eq!(
        unimplemented_tmem_op(CMD_LOAD_TILE, word).unwrap_err(),
        Trap::UnimplementedRdpCommand {
            cmd_id: CMD_LOAD_TILE,
            word
        }
    );
}

#[test]
fn load_tile_rgba16_odd_row_swaps_and_sample_restores() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    let pixels = [
        0xF801u16, 0x07C1, 0x003F, 0x0001, // row 0
        0xF801, 0x07C1, 0x003F, 0x0001, // row 1
    ];
    write_pixels(&mut rdram, 0, &pixels);
    set_image(&mut rdp, 2, 3, 0);
    configure_tile(&mut rdp, 0, 0, 2, 1, 0);
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            0,
            2,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_load_tile(
        &mut rdp,
        rect_word(CMD_LOAD_TILE, 0, 0, 0, q10(3), q10(1)),
        &rdram,
    )
    .unwrap();

    assert_eq!(&rdp.tmem[0..8], &pixels_be(&pixels[0..4]));
    // Odd row: 32-bit halves swapped. F8 01 07 C1 00 3F 00 01 -> 00 3F 00 01 F8 01 07 C1.
    assert_eq!(
        &rdp.tmem[8..16],
        &[0x00, 0x3F, 0x00, 0x01, 0xF8, 0x01, 0x07, 0xC1]
    );
    assert_eq!(rdp.tiles[0].sh, q10(3));
    assert_eq!(rdp.tiles[0].th, q10(1));

    let row1_s0 = sample_texel(&rdp, 0, 0, 1 << 5).unwrap();
    assert_eq!(
        row1_s0,
        TexelRgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255
        }
    );
    let row1_s2 = sample_texel(&rdp, 0, 2 << 5, 1 << 5).unwrap();
    assert_eq!(
        row1_s2,
        TexelRgba {
            r: 0,
            g: 0,
            b: 255,
            a: 255
        }
    );
}

#[test]
fn load_block_dxt_swaps_second_line_and_caps_at_2048() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    let pixels = [
        0x1111u16, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888,
    ];
    write_pixels(&mut rdram, 0, &pixels);
    set_image(&mut rdp, 2, 7, 0);
    configure_tile(&mut rdp, 0, 0, 2, 1, 0);
    // 8 texels, one 64-bit word per 4 texels, dxt = 1.0 so word 1 is an odd line.
    exec_load_block(
        &mut rdp,
        rect_word(CMD_LOAD_BLOCK, 0, 0, 0, 7, 0x800),
        &rdram,
    )
    .unwrap();
    assert_eq!(&rdp.tmem[0..8], &pixels_be(&pixels[0..4]));
    assert_eq!(
        &rdp.tmem[8..16],
        &[0x77, 0x77, 0x88, 0x88, 0x55, 0x55, 0x66, 0x66]
    );
    // Line stride is one word, so row 1 is that odd line. Sampling must undo the swap
    // and return the first texel of the line (0x5555), not the swapped-in 0x7777.
    assert_eq!(
        sample_texel(&rdp, 0, 0, 1 << 5).unwrap(),
        TexelRgba {
            r: 82,
            g: 173,
            b: 82,
            a: 255
        }
    );
    assert_eq!(rdp.tiles[0].sh, 7);
    assert_eq!(rdp.tiles[0].th, 0x800);

    // dxt = 0 keeps every word on line 0 (even): no swap.
    clear_tmem(&mut rdp);
    exec_load_block(&mut rdp, rect_word(CMD_LOAD_BLOCK, 0, 0, 0, 7, 0), &rdram).unwrap();
    assert_eq!(&rdp.tmem[8..16], &pixels_be(&pixels[4..8]));

    rdp.tmem.fill(0xAB);
    exec_load_block(
        &mut rdp,
        rect_word(CMD_LOAD_BLOCK, 0, 0, 0, 2048, 0),
        &rdram,
    )
    .unwrap();
    assert!(rdp.tmem.iter().all(|&b| b == 0xAB));
    assert_eq!(rdp.tiles[0].sh, 2048);

    rdram.write_u8(0, 0x5A);
    rdram.write_u8(2047, 0xA5);
    set_image(&mut rdp, 1, 2047, 0);
    configure_tile(&mut rdp, 1, 4, 1, 256, 0);
    exec_load_block(
        &mut rdp,
        rect_word(CMD_LOAD_BLOCK, 0, 0, 1, 2047, 0),
        &rdram,
    )
    .unwrap();
    assert_eq!(rdp.tmem[0], 0x5A);
    assert_eq!(rdp.tmem[2047], 0xA5);
}

#[test]
fn load_block_rgba32_and_bad_spans_trap() {
    let mut rdp = CpuRdpLle::new();
    let rdram = Rdram::new(false);
    set_image(&mut rdp, 3, 0, 0);
    configure_tile(&mut rdp, 0, 0, 3, 1, 0);
    let err =
        exec_load_block(&mut rdp, rect_word(CMD_LOAD_BLOCK, 0, 0, 0, 0, 0), &rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand {
            cmd_id: CMD_LOAD_BLOCK,
            ..
        }
    ));
    assert_eq!(rdp.tiles[0].sh, 0);

    set_image(&mut rdp, 2, 0, 4);
    configure_tile(&mut rdp, 0, 0, 2, 1, 0);
    assert!(exec_load_block(&mut rdp, rect_word(CMD_LOAD_BLOCK, 0, 0, 0, 0, 0), &rdram).is_err());

    set_image(&mut rdp, 2, 3, 0);
    let frac = exec_load_tile(&mut rdp, rect_word(CMD_LOAD_TILE, 0, 0, 0, 1, 0), &rdram);
    assert!(frac.is_err());
    assert_eq!(rdp.tiles[0].sh, 0);
}

#[test]
fn load_tile_rgba32_splits_halves_and_wraps() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    rdram.as_mut_slice()[0..8].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0xAA, 0xBB, 0xCC, 0xDD]);
    set_image(&mut rdp, 3, 0, 0);
    configure_tile(&mut rdp, 0, 0, 3, 1, 0);
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            0,
            3,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_load_tile(
        &mut rdp,
        rect_word(CMD_LOAD_TILE, 0, 0, 0, q10(0), q10(1)),
        &rdram,
    )
    .unwrap();
    assert_eq!(&rdp.tmem[0..2], &[0x11, 0x22]);
    assert_eq!(&rdp.tmem[0x800..0x802], &[0x33, 0x44]);
    // Row 1 is odd: low address 8 ^ 4 = 12, high half at 0x800 + 12.
    assert_eq!(&rdp.tmem[12..14], &[0xAA, 0xBB]);
    assert_eq!(&rdp.tmem[0x80C..0x80E], &[0xCC, 0xDD]);
    assert_eq!(
        sample_texel(&rdp, 0, 0, 1 << 5).unwrap(),
        TexelRgba {
            r: 0xAA,
            g: 0xBB,
            b: 0xCC,
            a: 0xDD
        }
    );

    // 8 texels of 16-bit from the last TMEM word spill back to address 0.
    clear_tmem(&mut rdp);
    let pixels = [
        0x0001u16, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008,
    ];
    write_pixels(&mut rdram, 0, &pixels);
    set_image(&mut rdp, 2, 7, 0);
    configure_tile(&mut rdp, 2, 0, 2, 2, 511);
    exec_load_tile(
        &mut rdp,
        rect_word(CMD_LOAD_TILE, 0, 0, 2, q10(7), 0),
        &rdram,
    )
    .unwrap();
    assert_eq!(&rdp.tmem[4088..4096], &pixels_be(&pixels[0..4]));
    assert_eq!(&rdp.tmem[0..8], &pixels_be(&pixels[4..8]));
}

#[test]
fn load_tlut_quadruples_and_ci_samples() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    write_pixels(&mut rdram, 0, &[0xF801, 0x07C1]);
    set_image(&mut rdp, 2, 1, 0);
    // TLUT tile: 4-bit IA, base at the high half.
    configure_tile(&mut rdp, 7, 3, 0, 1, 0x100);
    exec_load_tlut(
        &mut rdp,
        rect_word(CMD_LOAD_TLUT, 0, 0, 7, q10(1), 0),
        &rdram,
    )
    .unwrap();
    assert_eq!(
        &rdp.tmem[0x800..0x808],
        &[0xF8, 0x01, 0xF8, 0x01, 0xF8, 0x01, 0xF8, 0x01]
    );
    assert_eq!(
        &rdp.tmem[0x808..0x810],
        &[0x07, 0xC1, 0x07, 0xC1, 0x07, 0xC1, 0x07, 0xC1]
    );
    assert_eq!(rdp.tiles[7].sh, q10(1));

    // CI4 indices 0 then 1 in the low half. Palette 0.
    rdp.tmem[0] = 0x01;
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            2,
            0,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, q10(1), 0)).unwrap();
    assert_eq!(
        sample_texel(&rdp, 0, 0, 0).unwrap(),
        TexelRgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255
        }
    );
    assert_eq!(
        sample_texel(&rdp, 0, 1 << 5, 0).unwrap(),
        TexelRgba {
            r: 0,
            g: 255,
            b: 0,
            a: 255
        }
    );

    // CI8 index 1 reads the second TLUT word. IA16 mode reinterprets 0x07C1.
    configure_tile(&mut rdp, 1, 2, 1, 1, 0);
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            2,
            1,
            1,
            0,
            1,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 1, 0, 0)).unwrap();
    rdp.tmem[0] = 0x01;
    rdp.other_modes |= OTHER_MODES_TLUT_IA16;
    assert_eq!(
        sample_texel(&rdp, 1, 0, 0).unwrap(),
        TexelRgba {
            r: 0x07,
            g: 0x07,
            b: 0x07,
            a: 0xC1
        }
    );
}

#[test]
fn load_tile_nibble_preserves_the_other_half() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    rdram.write_u8(0, 0xA0);
    rdp.tmem[0] = 0x0F;
    set_image(&mut rdp, 0, 1, 0);
    configure_tile(&mut rdp, 0, 4, 0, 1, 0);
    exec_load_tile(&mut rdp, rect_word(CMD_LOAD_TILE, 0, 0, 0, 0, 0), &rdram).unwrap();
    assert_eq!(rdp.tmem[0], 0xAF);
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            0,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, 0, 0)).unwrap();
    assert_eq!(
        sample_texel(&rdp, 0, 0, 0).unwrap(),
        TexelRgba {
            r: 0xAA,
            g: 0xAA,
            b: 0xAA,
            a: 0xAA
        }
    );
}

#[test]
fn sampler_clamp_wrap_mirror_shift_and_origin() {
    let mut rdp = CpuRdpLle::new();
    // I8 texels 0x10, 0x20, 0x30, 0x40 on one even row.
    rdp.tmem[0] = 0x10;
    rdp.tmem[1] = 0x20;
    rdp.tmem[2] = 0x30;
    rdp.tmem[3] = 0x40;
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            1,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, q10(1), 0)).unwrap();
    assert_eq!(sample_texel(&rdp, 0, -5 << 5, 0).unwrap().r, 0x10);
    assert_eq!(sample_texel(&rdp, 0, 9 << 5, 0).unwrap().r, 0x20);

    // Upper-left extent is the TMEM origin: coordinate 2 reads texel 0.
    exec_set_tile_size(
        &mut rdp,
        rect_word(CMD_SET_TILE_SIZE, q10(2), 0, 0, q10(4), 0),
    )
    .unwrap();
    assert_eq!(sample_texel(&rdp, 0, 2 << 5, 0).unwrap().r, 0x10);
    assert_eq!(sample_texel(&rdp, 0, 4 << 5, 0).unwrap().r, 0x30);
    assert_eq!(sample_texel(&rdp, 0, 0, 0).unwrap().r, 0x10);

    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            1,
            1,
            0,
            0,
            0,
            (tile_axis(false, false, 0, 0), tile_axis(false, false, 2, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, q10(3), 0)).unwrap();
    assert_eq!(sample_texel(&rdp, 0, 5 << 5, 0).unwrap().r, 0x20);
    assert_eq!(sample_texel(&rdp, 0, -1, 0).unwrap().r, 0x40);

    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            1,
            1,
            0,
            0,
            0,
            (tile_axis(false, false, 0, 0), tile_axis(false, true, 2, 0)),
        ),
    )
    .unwrap();
    assert_eq!(sample_texel(&rdp, 0, 0, 0).unwrap().r, 0x10);
    assert_eq!(sample_texel(&rdp, 0, 3 << 5, 0).unwrap().r, 0x40);
    assert_eq!(sample_texel(&rdp, 0, 4 << 5, 0).unwrap().r, 0x40);
    assert_eq!(sample_texel(&rdp, 0, 5 << 5, 0).unwrap().r, 0x30);
    assert_eq!(sample_texel(&rdp, 0, 7 << 5, 0).unwrap().r, 0x10);

    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            1,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 1)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, q10(3), 0)).unwrap();
    // s10.5 value 2, shifted right by 1, is texel 1.
    assert_eq!(sample_texel(&rdp, 0, 2 << 5, 0).unwrap().r, 0x20);

    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            1,
            1,
            0,
            0,
            0,
            (tile_axis(false, false, 0, 0), tile_axis(false, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, 0, 0)).unwrap();
    assert!(sample_texel(&rdp, 0, 1 << 5, 0).is_err());

    exec_set_tile(
        &mut rdp,
        set_tile_word(
            4,
            1,
            1,
            0,
            0,
            0,
            (tile_axis(false, false, 0, 0), tile_axis(false, false, 2, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(
        &mut rdp,
        rect_word(CMD_SET_TILE_SIZE, q10(1), 0, 0, q10(3), 0),
    )
    .unwrap();
    assert!(sample_texel(&rdp, 0, 0, 0).is_err());

    exec_set_tile(
        &mut rdp,
        set_tile_word(
            1,
            2,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    assert!(matches!(
        sample_texel(&rdp, 0, 0, 0).unwrap_err(),
        Trap::UnimplementedRdpCommand {
            cmd_id: CMD_SET_TILE,
            ..
        }
    ));
    assert!(sample_texel(&rdp, 8, 0, 0).is_err());
}

#[test]
fn ia_formats_expand_components() {
    let mut rdp = CpuRdpLle::new();
    rdp.tmem[0] = 0xB0; // IA4 nibble 0b1011 at texel 0: I=5, A=1. Low nibble 0 is I=0, A=0.
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            3,
            0,
            1,
            0,
            0,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 0, q10(1), 0)).unwrap();
    let i5 = (5u8 << 5) | (5 << 2) | (5 >> 1);
    assert_eq!(
        sample_texel(&rdp, 0, 0, 0).unwrap(),
        TexelRgba {
            r: i5,
            g: i5,
            b: i5,
            a: 255
        }
    );
    assert_eq!(
        sample_texel(&rdp, 0, 1 << 5, 0).unwrap(),
        TexelRgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0
        }
    );

    configure_tile(&mut rdp, 1, 3, 1, 1, 0);
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            3,
            1,
            1,
            0,
            1,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 1, 0, 0)).unwrap();
    rdp.tmem[0] = 0xF0;
    assert_eq!(
        sample_texel(&rdp, 1, 0, 0).unwrap(),
        TexelRgba {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0x00
        }
    );

    configure_tile(&mut rdp, 2, 3, 2, 1, 0);
    exec_set_tile(
        &mut rdp,
        set_tile_word(
            3,
            2,
            1,
            0,
            2,
            0,
            (tile_axis(true, false, 0, 0), tile_axis(true, false, 0, 0)),
        ),
    )
    .unwrap();
    exec_set_tile_size(&mut rdp, rect_word(CMD_SET_TILE_SIZE, 0, 0, 2, 0, 0)).unwrap();
    rdp.tmem[0] = 0x7F;
    rdp.tmem[1] = 0x80;
    assert_eq!(
        sample_texel(&rdp, 2, 0, 0).unwrap(),
        TexelRgba {
            r: 0x7F,
            g: 0x7F,
            b: 0x7F,
            a: 0x80
        }
    );
}

#[test]
fn dispatcher_routes_set_tile_size() {
    let mut rdp = CpuRdpLle::new();
    let rdram = Rdram::new(false);
    exec_tmem_command(
        &mut rdp,
        rect_word(CMD_SET_TILE_SIZE, q10(4), 0, 2, q10(6), q10(1)),
        &rdram,
    )
    .unwrap();
    assert_eq!(rdp.tiles[2].sl, q10(4));
    assert_eq!(rdp.tiles[2].sh, q10(6));
    assert_eq!(rdp.tiles[2].th, q10(1));
}

fn pixels_be(pixels: &[u16]) -> Vec<u8> {
    pixels.iter().flat_map(|p| p.to_be_bytes()).collect()
}
