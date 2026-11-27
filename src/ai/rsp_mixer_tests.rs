// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
// Cited: N64brew — Reality Signal Processor (Vector Unit Audio Mixer Tests & Oracles)
//   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md · docs/DEVELOPMENT-PLAN.md §3.9

use super::*;
use crate::bus::Bus;
use crate::rsp::{InterpRsp, RspBackend, Sp};

#[test]
fn test_stereo_mixer_ucode_and_oracle() {
    let mut bus = Bus::new(true, false);
    let mut rsp = InterpRsp::new();

    let num_blocks: u16 = 8; // 8 blocks * 8 samples/block = 64 samples per voice
    let dram_dst: u32 = 0x0001_2000;

    let voice_a = generate_pcm16_test_wave(64, 0x1234, 16383);
    let voice_b = generate_pcm16_test_wave(64, 0x4321, 24576);
    let gains = [24576, 16383, 8192, 24576]; // [gA_L, gA_R, gB_L, gB_R] in Q15

    load_stereo_mixer_inputs_to_dmem(&mut bus.sp_dmem, &voice_a, &voice_b, gains);

    let ucode = build_rsp_stereo_mixer_ucode(num_blocks, dram_dst);
    upload_rsp_ucode(&mut bus.sp_imem, &ucode);

    // Clear HALT so RSP runs
    bus.sp.status &= !Sp::STATUS_HALT;
    bus.sp.pc = 0;

    // Run RSP until BREAK
    let cycles = rsp
        .run(&mut bus, 10_000)
        .expect("RSP stereo mixer execution failed");
    assert!(cycles > 0);
    assert!(bus.sp.is_halted());
    assert_ne!(bus.sp.status & Sp::STATUS_BROKE, 0);

    // Verify RDRAM bytes at dram_dst match oracle_stereo_mix bit-for-bit
    let oracle_res = oracle_stereo_mix(&voice_a, &voice_b, gains);

    for (i, &expected_sample) in oracle_res.iter().enumerate() {
        let rdram_addr = (dram_dst as usize) + i * 2;
        let b0 = bus.rdram.read_u8(rdram_addr as u32);
        let b1 = bus.rdram.read_u8((rdram_addr + 1) as u32);
        let actual_sample = i16::from_be_bytes([b0, b1]);
        assert_eq!(
            actual_sample, expected_sample,
            "Stereo mixer mismatch at sample index {}: expected {}, got {}",
            i, expected_sample, actual_sample
        );
    }
}

#[test]
fn test_phase_and_predictor_ucode_and_oracle() {
    let mut bus = Bus::new(true, false);
    let mut rsp = InterpRsp::new();

    let phase_lo = [
        0x5000, 0xFFFF, 0x1234, 0x8000, 0x0000, 0x1111, 0x2222, 0x3333,
    ];
    let step_lo = [
        0xB000, 0x0002, 0x0FED, 0x7FFF, 0xFFFF, 0x1000, 0x2000, 0x3000,
    ];
    let phase_hi = [
        0x0010, 0x0020, 0x0030, 0x0040, 0x0050, 0x0060, 0x0070, 0x0080,
    ];
    let step_hi = [
        0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008,
    ];

    let s0 = [1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000];
    let c0 = [16384, 8192, 4096, 2048, 1024, 512, 256, 128];
    let s1 = [-1000, -2000, 1500, -2500, 3000, -3500, 4000, -4500];
    let c1 = [8192, 16384, 2048, 4096, 512, 1024, 128, 256];

    let write_vec_to_dmem = |dmem: &mut [u8], base_addr: u32, v: &[u16; 8]| {
        for (i, &val) in v.iter().enumerate() {
            let addr = (base_addr as usize) + i * 2;
            let bytes = val.to_be_bytes();
            dmem[addr] = bytes[0];
            dmem[addr + 1] = bytes[1];
        }
    };

    write_vec_to_dmem(&mut bus.sp_dmem, 0x000, &phase_lo);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x010, &step_lo);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x020, &phase_hi);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x030, &step_hi);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x040, &s0);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x050, &c0);
    let s1_u16: [u16; 8] = s1.map(|x| x as u16);
    let c1_u16: [u16; 8] = c1.map(|x| x as u16);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x060, &s1_u16);
    write_vec_to_dmem(&mut bus.sp_dmem, 0x070, &c1_u16);

    let ucode = build_rsp_phase_and_predictor_ucode();
    upload_rsp_ucode(&mut bus.sp_imem, &ucode);

    // Clear HALT so RSP runs
    bus.sp.status &= !Sp::STATUS_HALT;
    bus.sp.pc = 0;

    let cycles = rsp
        .run(&mut bus, 10_000)
        .expect("RSP phase & predictor execution failed");
    assert!(cycles > 0);

    let oracle = oracle_phase_and_predictor(&PhasePredictorInput {
        phase_lo,
        step_lo,
        phase_hi,
        step_hi,
        s0,
        c0,
        s1: s1_u16,
        c1: c1_u16,
    });

    let read_vec_from_dmem = |dmem: &[u8], base_addr: u32| -> [u16; 8] {
        let mut v = [0u16; 8];
        for (i, slot) in v.iter_mut().enumerate() {
            let addr = (base_addr as usize) + i * 2;
            *slot = u16::from_be_bytes([dmem[addr], dmem[addr + 1]]);
        }
        v
    };

    let actual_phase_lo = read_vec_from_dmem(&bus.sp_dmem, 0x100);
    let actual_phase_hi = read_vec_from_dmem(&bus.sp_dmem, 0x110);
    let actual_pred = read_vec_from_dmem(&bus.sp_dmem, 0x120);

    assert!(bus.sp.is_halted());
    assert_ne!(bus.sp.status & Sp::STATUS_BROKE, 0);
    assert_eq!(actual_phase_lo, oracle.phase_lo);
    assert_eq!(actual_phase_hi, oracle.phase_hi);
    assert_ne!(oracle.vco, 0);

    for (i, &acc_val) in oracle.acc.iter().enumerate() {
        let expected_lo = (acc_val & 0xFFFF) as u16;
        assert_eq!(rsp.vu.acc_lo[i], expected_lo, "Acc lo mismatch at lane {i}");
    }

    assert_eq!(actual_pred, oracle.pred_lo);
}
