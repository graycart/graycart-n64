//! Unit tests for the Video Interface.
//!
//! Cited: N64brew — Video Interface (`VI_CTRL` TYPE, `VI_V_VIDEO`, `VI_Y_SCALE`)
//!   https://n64brew.dev/wiki/Video_Interface

use crate::bus::Rdram;
use crate::trap::Trap;
use crate::vi::{hash_rgba8888_frame, Vi, ViAction, ViColorType};

#[test]
fn test_vi_registers_rw_and_masking() {
    let mut vi = Vi::new();

    // ctrl
    vi.write_reg(0x00, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x00).unwrap(), 0x0001_FFFF);

    // origin
    vi.write_reg(0x04, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x04).unwrap(), 0x00FF_FFFF);

    // width
    vi.write_reg(0x08, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x08).unwrap(), 0x0FFF);

    // v_intr
    vi.write_reg(0x0C, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x0C).unwrap(), 0x03FF);

    // v_current write action returns ClearIntr and doesn't overwrite
    vi.v_current = 42;
    let action = vi.write_reg_action(0x10, 0xFFFF_FFFF).unwrap();
    assert_eq!(action, ViAction::ClearIntr);
    assert_eq!(vi.v_current, 42);
    assert_eq!(vi.read_reg(0x10).unwrap(), 42 & 0x03FF);

    // burst
    vi.write_reg(0x14, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x14).unwrap(), 0x3FFF_FFFF);

    // v_sync
    vi.write_reg(0x18, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x18).unwrap(), 0x03FF);

    // h_sync
    vi.write_reg(0x1C, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x1C).unwrap(), 0x001F_0FFF);

    // h_sync_leap
    vi.write_reg(0x20, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x20).unwrap(), 0x0FFF_0FFF);

    // h_video
    vi.write_reg(0x24, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x24).unwrap(), 0x03FF_03FF);

    // v_video
    vi.write_reg(0x28, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x28).unwrap(), 0x03FF_03FF);

    // v_burst
    vi.write_reg(0x2C, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x2C).unwrap(), 0x03FF_03FF);

    // x_scale
    vi.write_reg(0x30, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x30).unwrap(), 0x0FFF_0FFF);

    // y_scale
    vi.write_reg(0x34, 0xFFFF_FFFF).unwrap();
    assert_eq!(vi.read_reg(0x34).unwrap(), 0x0FFF_0FFF);

    // Unimplemented offset
    assert!(matches!(
        vi.read_reg(0x38),
        Err(Trap::UnimplementedRegister {
            device: "VI",
            offset: 0x38,
            is_write: false
        })
    ));
    assert!(matches!(
        vi.write_reg(0x38, 0),
        Err(Trap::UnimplementedRegister {
            device: "VI",
            offset: 0x38,
            is_write: true
        })
    ));
}

#[test]
fn test_vi_scanline_timing_and_interrupts() {
    let mut vi = Vi::new();
    vi.v_sync = 525;
    vi.v_intr = 2;

    assert_eq!(vi.num_halflines(), 525);
    assert!(vi.cycles_per_line() > 0);

    // Step line from 0
    let (intr, frame_completed) = vi.step_line();
    assert_eq!(vi.v_current, 2);
    assert!(intr);
    assert!(!frame_completed);

    // Step until wrap
    let mut frames = 0;
    for _ in 0..300 {
        let (_, fc) = vi.step_line();
        if fc {
            frames += 1;
        }
    }
    assert!(frames > 0);
}

#[test]
fn test_vi_framebuffer_hash() {
    let vi = Vi::new();
    let rdram = Rdram::new(false);
    let h = vi.framebuffer_hash(&rdram);
    assert_ne!(h, 0);
}

fn rgba5551(r: u16, g: u16, b: u16, a: u16) -> u16 {
    ((r & 31) << 11) | ((g & 31) << 6) | ((b & 31) << 1) | (a & 1)
}

/// One active output line, `Y_SCALE = 0x400` (1:1), REPLICATE anti-alias mode.
fn program_1x1(vi: &mut Vi, ctrl: u32, width: u32) {
    vi.write_reg(0x00, ctrl).unwrap();
    vi.write_reg(0x04, 0).unwrap();
    vi.write_reg(0x08, width).unwrap();
    // V_START = 0, V_END = 2 → (2 - 0) / 2 = 1 output line.
    vi.write_reg(0x28, 2).unwrap();
    vi.write_reg(0x34, 0x400).unwrap();
}

#[test]
fn vi_ctrl_type_selects_off_rgba16_or_rgba32() {
    let mut vi = Vi::new();
    assert_eq!(vi.color_type().unwrap(), ViColorType::Off);
    vi.write_reg(0x00, 2).unwrap();
    assert_eq!(vi.color_type().unwrap(), ViColorType::Rgba16);
    vi.write_reg(0x00, 3).unwrap();
    assert_eq!(vi.color_type().unwrap(), ViColorType::Rgba32);
    assert!(matches!(
        vi.write_reg(0x00, 1).and_then(|_| vi.color_type()),
        Err(Trap::UnimplementedRegister {
            device: "VI_CTRL.TYPE",
            offset: 1,
            ..
        })
    ));
}

#[test]
fn scanout_rgba16_golden_frame() {
    let mut vi = Vi::new();
    // TYPE = 16-bit, AA_MODE = REPLICATE.
    program_1x1(&mut vi, (3 << 8) | 2, 4);
    let mut rdram = Rdram::new(false);
    let pixels = [
        rgba5551(31, 0, 0, 1),
        rgba5551(0, 31, 0, 1),
        rgba5551(0, 0, 31, 0),
        rgba5551(16, 8, 4, 1),
    ];
    for (i, pix) in pixels.iter().enumerate() {
        rdram.write_u16((i * 2) as u32, *pix).unwrap();
    }
    let frame = vi.scanout_rgba8888(&rdram).unwrap();
    assert_eq!(frame.width, 4);
    assert_eq!(frame.height, 1);
    assert_eq!(frame.color_type, ViColorType::Rgba16);
    assert_eq!(
        frame.rgba,
        [
            0xFF, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x84, 0x42,
            0x21, 0xFF,
        ]
    );
    let hash = hash_rgba8888_frame(&frame);
    assert_eq!(hash, 0xaf4e_a6cd_8e31_5cf0);
    assert_eq!(vi.scanout_frame_hash(&rdram).unwrap(), hash);
    assert_ne!(vi.framebuffer_hash(&rdram), hash);
}

#[test]
fn scanout_rgba32_golden_frame() {
    let mut vi = Vi::new();
    program_1x1(&mut vi, (3 << 8) | 3, 2);
    let mut rdram = Rdram::new(false);
    rdram.write_u32(0, 0x1122_3344).unwrap();
    rdram.write_u32(4, 0x0000_00FF).unwrap();
    let frame = vi.scanout_rgba8888(&rdram).unwrap();
    assert_eq!(frame.color_type, ViColorType::Rgba32);
    assert_eq!(frame.rgba, [0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0xFF]);
    assert_eq!(hash_rgba8888_frame(&frame), 0xddd3_277d_0c1d_9727);
    assert_eq!(
        vi.scanout_frame_hash(&rdram).unwrap(),
        0xddd3_277d_0c1d_9727
    );
}

#[test]
fn scanout_off_is_empty_and_filters_or_reserved_type_trap() {
    let vi = Vi::new();
    let rdram = Rdram::new(false);
    let frame = vi.scanout_rgba8888(&rdram).unwrap();
    assert_eq!(frame.width, 0);
    assert_eq!(frame.height, 0);
    assert!(frame.rgba.is_empty());
    assert_eq!(hash_rgba8888_frame(&frame), 0x1c09_b3ee_f006_1e32);

    let mut vi = Vi::new();
    program_1x1(&mut vi, (3 << 8) | 1, 4);
    assert!(vi.scanout_rgba8888(&rdram).is_err());

    program_1x1(&mut vi, ((3 << 8) | 2) | (1 << 3), 4); // gamma
    assert!(matches!(
        vi.scanout_rgba8888(&rdram),
        Err(Trap::UnimplementedRegister {
            device: "VI_SCANOUT",
            ..
        })
    ));
    program_1x1(&mut vi, 2, 4); // AA_MODE = 0, not REPLICATE
    assert!(vi.scanout_rgba8888(&rdram).is_err());
    program_1x1(&mut vi, ((3 << 8) | 2) | (1 << 16), 4); // dedither
    assert!(vi.scanout_rgba8888(&rdram).is_err());
}

#[test]
fn scanout_requires_aligned_origin_and_stride() {
    let mut vi = Vi::new();
    let rdram = Rdram::new(false);
    program_1x1(&mut vi, (3 << 8) | 2, 4);
    vi.write_reg(0x04, 4).unwrap();
    assert!(matches!(
        vi.scanout_rgba8888(&rdram),
        Err(Trap::MisalignedAccess { pa: 4, size: 8 })
    ));
    vi.write_reg(0x04, 0).unwrap();
    vi.write_reg(0x08, 2).unwrap(); // 16-bit stride of 4 is not a multiple of 8
    assert!(matches!(
        vi.scanout_rgba8888(&rdram),
        Err(Trap::MisalignedAccess { .. })
    ));
}

#[test]
fn scanout_height_follows_half_lines_and_y_scale() {
    let mut vi = Vi::new();
    // V_START = 2, V_END = 6 → 2 output lines. Y_SCALE = 0x800 steps two rows.
    // Last row index is 2, so the source span is 3 rows.
    vi.write_reg(0x00, (3 << 8) | 2).unwrap();
    vi.write_reg(0x08, 4).unwrap();
    vi.write_reg(0x28, (2 << 16) | 6).unwrap();
    vi.write_reg(0x34, 0x800).unwrap();
    assert_eq!(vi.active_output_lines(), 2);
    assert_eq!(vi.scanout_height(), 3);
    let rdram = Rdram::new(false);
    let frame = vi.scanout_rgba8888(&rdram).unwrap();
    assert_eq!(frame.height, 3);
    assert_eq!(frame.rgba.len(), 3 * 4 * 4);
}
