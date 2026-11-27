// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Unit tests for Audio Interface (AI) double-buffered DMA controller.

use super::*;
use crate::bus::Rdram;

#[test]
fn test_ai_registers_and_traps() {
    let mut ai = AiDma::new();
    assert_eq!(ai.read_reg(0x00).unwrap(), 0);
    assert_eq!(ai.read_reg(0x04).unwrap(), 0);
    assert_eq!(ai.read_reg(0x08).unwrap(), 0);
    assert_eq!(ai.read_reg(0x0C).unwrap(), 0x0010_0000); // full=0, busy=0, enable=0, 1<<20, full=0

    assert!(matches!(
        ai.read_reg(0x18),
        Err(Trap::UnimplementedRegister {
            offset: 0x18,
            is_write: false,
            ..
        })
    ));
    assert!(matches!(
        ai.write_reg_action(0x18, 0),
        Err(Trap::UnimplementedRegister {
            offset: 0x18,
            is_write: true,
            ..
        })
    ));
}

#[test]
fn test_ai_dma_fifo_and_irq_on_start() {
    let mut ai = AiDma::new();
    ai.control = 1; // enable AI so StartDma returns Some(delay_cycles)

    // Write dram addr
    assert_eq!(ai.write_reg_action(0x00, 0x1000).unwrap(), AiAction::None);

    // Write len -> fifo 0 (start dma)
    let action = ai.write_reg_action(0x04, 0x100).unwrap();
    match action {
        AiAction::StartDma {
            raise_intr,
            delay_cycles,
        } => {
            assert!(raise_intr);
            assert!(delay_cycles.is_some());
        }
        _ => panic!("Expected StartDma"),
    }
    assert_eq!(ai.fifo_count, 1);
    assert_eq!(ai.read_reg(0x00).unwrap(), 0x100);

    // Write dram addr for second slot
    assert_eq!(ai.write_reg_action(0x00, 0x2000).unwrap(), AiAction::None);
    // Write len -> fifo 1 (no start dma / no immediate IRQ)
    let action = ai.write_reg_action(0x04, 0x200).unwrap();
    assert_eq!(action, AiAction::None);
    assert_eq!(ai.fifo_count, 2);
}

#[test]
fn test_ai_quirk_1_length_latching() {
    let mut ai = AiDma::new();
    ai.write_reg_action(0x00, 0x1000).unwrap();
    ai.write_reg_action(0x04, 0x100).unwrap(); // fifo[0]
    ai.write_reg_action(0x00, 0x2000).unwrap();
    ai.write_reg_action(0x04, 0x200).unwrap(); // fifo[1], fifo now full (2)

    // Try writing length = 0 or writing while full: should not corrupt fifo[0].len (0x100)
    ai.write_reg_action(0x04, 0).unwrap();
    ai.write_reg_action(0x04, 0x300).unwrap();
    assert_eq!(ai.fifo[0].len, 0x100);
}

#[test]
fn test_ai_quirk_2_boundary_delayed_carry() {
    let mut rdram = Rdram::new(true);
    let mut ai = AiDma::new();
    ai.control = 1; // enable AI so StartDma returns Some(delay_cycles)

    // 1. Mid-transfer 8 KiB boundary crossing MUST NOT trigger delayed_carry.
    rdram.write_u32(0x1FF8, 0x1111_2222).unwrap();
    rdram.write_u32(0x1FFC, 0x3333_4444).unwrap();
    rdram.write_u32(0x2000, 0x5555_6666).unwrap();
    rdram.write_u32(0x2004, 0x7777_8888).unwrap();
    ai.write_reg_action(0x00, 0x1FF8).unwrap();
    ai.write_reg_action(0x04, 16).unwrap();
    let out_mid = ai.step_burst(&rdram);
    assert_eq!(out_mid.frames, Some([(0x1111, 0x2222), (0x3333, 0x4444)]));
    assert!(!ai.delayed_carry);
    assert_eq!(ai.fifo[0].addr, 0x2000);
    let out_mid2 = ai.step_burst(&rdram);
    assert_eq!(
        out_mid2.frames,
        Some([(0x5555, 0x6666), (0x7777, 0x8888u16 as i16)])
    );
    assert!(!ai.delayed_carry);
    assert_eq!(ai.fifo_count, 0);

    // 2. Transfer ending exactly on 0x2000 boundary sets delayed_carry and shifts next QUEUED slot (2 -> 1).
    ai.write_reg_action(0x00, 0x1FF8).unwrap();
    ai.write_reg_action(0x04, 8).unwrap();
    ai.write_reg_action(0x00, 0x4000).unwrap();
    ai.write_reg_action(0x04, 16).unwrap();
    let out_queued = ai.step_burst(&rdram);
    assert!(out_queued.raise_intr);
    assert_eq!(ai.fifo_count, 1);
    assert_eq!(ai.fifo[0].addr, 0x6000);
    assert!(!ai.delayed_carry);
    assert_eq!(ai.read_reg(0x04).unwrap(), 16);
    ai.step_burst(&rdram);
    assert_eq!(ai.read_reg(0x04).unwrap(), 8);
    ai.step_burst(&rdram);
    assert_eq!(ai.read_reg(0x04).unwrap(), 0);

    // 3. Transfer ending on 0x2000 when idle shifts the NEXT queued transfer.
    ai.write_reg_action(0x00, 0x1FF8).unwrap();
    ai.write_reg_action(0x04, 8).unwrap();
    let outcome = ai.step_burst(&rdram);
    assert!(outcome.frames.is_some());
    assert!(!outcome.raise_intr);
    assert_eq!(ai.fifo_count, 0);
    assert!(ai.delayed_carry);

    ai.write_reg_action(0x00, 0x1000).unwrap();
    ai.write_reg_action(0x04, 8).unwrap();
    assert_eq!(ai.fifo[0].addr, 0x3000);
    assert!(!ai.delayed_carry);
}

#[test]
fn test_ai_control_pause_resume_and_rational_pacing() {
    let mut ai = AiDma::new();
    ai.write_reg_action(0x10, 1102).unwrap(); // ~44136 Hz NTSC
    assert_eq!(ai.sample_rate_hz(), 44_135);

    // Queue while AI_CONTROL = 0: StartDma returns raise_intr=true, delay_cycles=None.
    ai.write_reg_action(0x00, 0x1000).unwrap();
    assert_eq!(
        ai.write_reg_action(0x04, 16).unwrap(),
        AiAction::StartDma {
            raise_intr: true,
            delay_cycles: None,
        }
    );
    // Turning AI_CONTROL 0 -> 1 schedules pending FIFO burst.
    assert!(matches!(
        ai.write_reg_action(0x08, 1).unwrap(),
        AiAction::ScheduleDma { delay_cycles } if delay_cycles > 0
    ));

    // Exact zero-drift rational pacing over 44,100 frames (22,050 bursts * 2 samples).
    for std_mode in [AiTvStandard::Ntsc, AiTvStandard::Pal, AiTvStandard::Mpal] {
        let mut paced = AiDma::new();
        paced.tv_standard = std_mode;
        paced.dacrate = 1102;
        let mut total_cycles = 0u128;
        for _ in 0..22_050 {
            total_cycles += u128::from(paced.next_burst_cycles());
        }
        let expected_exact = (44_100u128 * u128::from(crate::rcp::CPU_CLOCK_HZ_NTSC) * 1103u128
            + u128::from(paced.cycle_rem))
            / u128::from(std_mode.vi_clock_hz());
        // cycle_rem conservation guarantees zero accumulated fractional error:
        let lhs = total_cycles * u128::from(std_mode.vi_clock_hz()) + u128::from(paced.cycle_rem);
        let rhs = 44_100u128 * u128::from(crate::rcp::CPU_CLOCK_HZ_NTSC) * 1103u128;
        assert_eq!(lhs, rhs);
        assert!(expected_exact > 0);
    }

    let json = serde_json::to_string(&ai).unwrap();
    let decoded: AiDma = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.dacrate, ai.dacrate);
    assert_eq!(decoded.tv_standard, ai.tv_standard);
}
