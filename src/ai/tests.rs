// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Unit tests for composed Audio Interface (`Ai`: DMA + PCM ring + resampler).
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md

use super::*;
use crate::bus::Rdram;

#[test]
fn test_ai_fifo_and_dma() {
    let rdram = Rdram::new(true);
    let mut ai = Ai::new();

    // Initial status: not busy, not full, enable 0
    let status = ai.read_reg(0x0C).unwrap();
    assert_eq!(status & (1 << 30), 0); // busy = 0
    assert_eq!(status & (1 << 31), 0); // full = 0

    ai.write_reg(0x08, 1).unwrap(); // enable DMA
    assert_eq!(ai.dma.control, 1);
    ai.write_reg(0x10, 0x100).unwrap();
    assert_eq!(ai.dma.dacrate, 0x100);
    ai.write_reg(0x14, 5).unwrap();
    assert_eq!(ai.dma.bitrate, 5);

    // Write DRAM addr
    ai.write_reg(0x00, 0x1000).unwrap();
    assert_eq!(ai.dma.dram_addr, 0x1000);

    // Write LEN -> starts 1st DMA with IRQ on start
    let action = ai.write_reg_action(0x04, 16).unwrap();
    assert!(matches!(
        action,
        AiAction::StartDma {
            raise_intr: true,
            delay_cycles: Some(_),
        }
    ));
    assert_eq!(ai.dma.fifo_count, 1);

    // Status now busy (bit 30), not full
    let status1 = ai.read_reg(0x0C).unwrap();
    assert_ne!(status1 & (1 << 30), 0);
    assert_eq!(status1 & (1 << 31), 0);

    // Queue 2nd buffer (FIFO full, no extra IRQ until slot 0 drains)
    ai.write_reg(0x00, 0x2000).unwrap();
    let action2 = ai.write_reg_action(0x04, 8).unwrap();
    assert_eq!(action2, AiAction::None);
    assert_eq!(ai.dma.fifo_count, 2);

    let status2 = ai.read_reg(0x0C).unwrap();
    assert_ne!(status2 & (1 << 31), 0); // full = 1

    // Reading non-status registers returns active fifo[0].len
    assert_eq!(ai.read_reg(0x00).unwrap(), 16);

    // Write status clears interrupt
    let stat_action = ai.write_reg_action(0x0C, 0).unwrap();
    assert_eq!(stat_action, AiAction::ClearIntr);

    // First 8-byte burst decrements active length to 8
    let b1 = ai.step_burst(&rdram);
    assert!(!b1.raise_intr);
    assert!(b1.next_delay_cycles.is_some());
    assert_eq!(ai.read_reg(0x04).unwrap(), 8);

    // Second burst finishes slot 0, promotes slot 1, and raises IRQ on start
    let b2 = ai.step_burst(&rdram);
    assert!(b2.raise_intr);
    assert!(b2.next_delay_cycles.is_some());
    assert_eq!(ai.dma.fifo_count, 1);
    assert_eq!(ai.read_reg(0x04).unwrap(), 8);

    // Third burst drains slot 1 cleanly
    let b3 = ai.step_burst(&rdram);
    assert!(!b3.raise_intr);
    assert!(b3.next_delay_cycles.is_none());
    assert_eq!(ai.dma.fifo_count, 0);
    assert_eq!(ai.read_reg(0x04).unwrap(), 0);

    // Unimplemented offsets trap
    assert!(matches!(
        ai.read_reg(0x20),
        Err(Trap::UnimplementedRegister {
            device: "AI",
            offset: 0x20,
            is_write: false
        })
    ));
    assert!(matches!(
        ai.write_reg(0x20, 0),
        Err(Trap::UnimplementedRegister {
            device: "AI",
            offset: 0x20,
            is_write: true
        })
    ));
}

#[test]
fn test_ai_composed_burst_to_pcm_and_resampler() {
    let mut rdram = Rdram::new(true);
    let mut ai = Ai::new();
    ai.write_reg(0x10, 1102).unwrap(); // 44,135 Hz NTSC
    ai.write_reg(0x08, 1).unwrap();

    // Seed 24 bytes (6 stereo frames = 3 DMA bursts) at RDRAM 0x1000
    let expected: [i16; 12] = [
        100, -100, 200, -200, 300, -300, 400, -400, 500, -500, 600, -600,
    ];
    for (i, &sample) in expected.iter().enumerate() {
        rdram
            .write_u16(0x1000 + (i as u32) * 2, sample as u16)
            .unwrap();
    }

    ai.write_reg(0x00, 0x1000).unwrap();
    let _ = ai.write_reg_action(0x04, 24).unwrap();
    while ai.dma.fifo_count > 0 {
        let _ = ai.step_burst(&rdram);
    }

    assert_eq!(ai.pcm_hash(), fnv1a_pcm_i16(&expected));
    assert_eq!(ai.telemetry().peak_left, 600);
    assert_eq!(ai.telemetry().peak_right, 600);

    let mut pcm = Vec::new();
    let drained = ai.drain_pcm_i16(&mut pcm);
    assert_eq!(drained, 6);
    assert_eq!(pcm, expected);

    // Resample another burst cleanly to 48 kHz
    ai.write_reg(0x00, 0x1000).unwrap();
    let _ = ai.write_reg_action(0x04, 24).unwrap();
    while ai.dma.fifo_count > 0 {
        let _ = ai.step_burst(&rdram);
    }
    let mut resampled = Vec::new();
    let resampled_frames = ai.drain_pcm_resampled_i16(48_000, &mut resampled);
    assert!(resampled_frames > 0);
    assert_eq!(resampled.len(), resampled_frames * 2);
}
