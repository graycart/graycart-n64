//! NEC VR4300, VR4305, VR4310 User's Manual U10504EJ7V0UMJ1 and docs/01-cpu-r4300i.md.
//! Unit tests for Video Interface (VI).

use crate::bus::Rdram;
use crate::trap::Trap;
use crate::vi::{Vi, ViAction};

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
