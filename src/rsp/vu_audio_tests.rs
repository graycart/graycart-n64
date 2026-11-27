// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Exhaustive RSP VU Audio Precision Stress Suite
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit Audio & Precision)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

use crate::bus::Bus;
use crate::rsp::vu_arith::exec_vu_arith;
use crate::rsp::vu_lsu::{exec_lwc2, exec_swc2};
use crate::rsp::InterpRsp;

fn enc_vu(e: u32, vt: u32, vs: u32, vd: u32, funct: u32) -> u32 {
    (0x12 << 26) | (1 << 25) | (e << 21) | (vt << 16) | (vs << 11) | (vd << 6) | funct
}

#[test]
fn test_vmulf_vmulu_vmacf_vmacu_audio_saturation_matrix() {
    let mut rsp = InterpRsp::new();

    rsp.vu.vpr[1] = [
        0x8000, 0x7FFF, 0x8000, 0xFFFF, 0x7FFF, 0x4000, 0xC000, 0x0000,
    ];
    rsp.vu.vpr[2] = [
        0x8000, 0x7FFF, 0x7FFF, 0xFFFF, 0x0001, 0x4000, 0xC000, 0x0000,
    ];

    // VMULF (funct 0x00)
    let vmulf_word = enc_vu(0, 2, 1, 3, 0x00);
    exec_vu_arith(&mut rsp, 0, vmulf_word).unwrap();

    assert_eq!(rsp.vu.vpr[3][0], 32767);
    assert_eq!(rsp.vu.vpr[3][1], 32766);
    assert_eq!(rsp.vu.vpr[3][2], 32769);

    // VMULU (funct 0x01)
    let vmulu_word = enc_vu(0, 2, 1, 4, 0x01);
    exec_vu_arith(&mut rsp, 0, vmulu_word).unwrap();
    assert_eq!(rsp.vu.vpr[4][2], 0x0000);

    // VMACF / VMACU multi-tap chains crossing acc_md -> acc_hi positive/negative saturation
    rsp.vu.reset();
    rsp.vu.vpr[1] = [0x7FFF, 0x7FFF, 0x8000, 0x8000, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [0x7FFF, 0x7FFF, 0x8000, 0x7FFF, 0, 0, 0, 0];
    let vmacf_word = enc_vu(0, 2, 1, 3, 0x08);
    for _ in 0..10 {
        exec_vu_arith(&mut rsp, 0, vmacf_word).unwrap();
    }
    assert_eq!(rsp.vu.vpr[3][0], 32767);
    assert_eq!(rsp.vu.vpr[3][2], 32767);
    assert_eq!(rsp.vu.vpr[3][3], 0x8000);
}

#[test]
fn test_vmudl_vmacl_vmudn_vmacn_48bit_carry_and_low_clamps() {
    let mut rsp = InterpRsp::new();

    rsp.vu.reset();
    rsp.vu.vpr[1] = [0xFFFF, 0, 0, 0, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [0x0001, 0, 0, 0, 0, 0, 0, 0];

    // VMUDL (funct 0x04)
    let vmudl_word = enc_vu(0, 2, 1, 3, 0x04);
    exec_vu_arith(&mut rsp, 0, vmudl_word).unwrap();
    assert_eq!(rsp.vu.acc_lo[0], 0);

    // Set acc_lo to 0xFFFF and execute VMACL (funct 0x0C) with prod = 1 (0x0100 * 0x0100 >> 16)
    rsp.vu.acc_lo[0] = 0xFFFF;
    rsp.vu.acc_md[0] = 0;
    rsp.vu.acc_hi[0] = 0;
    rsp.vu.vpr[1][0] = 0x0100;
    rsp.vu.vpr[2][0] = 0x0100;
    let vmacl_word = enc_vu(0, 2, 1, 3, 0x0C);
    exec_vu_arith(&mut rsp, 0, vmacl_word).unwrap();
    assert_eq!(rsp.vu.acc_lo[0], 0);
    assert_eq!(rsp.vu.acc_md[0], 1);

    // Test clamp_acc_low boundary values around hi_md = -32769, -32768, 0, 32767, 32768
    rsp.vu.acc_hi[0] = 0xFFFF;
    rsp.vu.acc_md[0] = 0x7FFF; // -32769 -> clamps low to 0x0000
    rsp.vu.acc_lo[0] = 0x1234;
    assert_eq!(rsp.vu.clamp_acc_low(0), 0x0000);

    rsp.vu.acc_hi[0] = 0xFFFF;
    rsp.vu.acc_md[0] = 0x8000; // -32768 -> passes acc_lo
    rsp.vu.acc_lo[0] = 0x1234;
    assert_eq!(rsp.vu.clamp_acc_low(0), 0x1234);

    rsp.vu.acc_hi[0] = 0xFFFF;
    rsp.vu.acc_md[0] = 0x8001; // -32767 -> passes acc_lo
    rsp.vu.acc_lo[0] = 0x1234;
    assert_eq!(rsp.vu.clamp_acc_low(0), 0x1234);

    rsp.vu.acc_hi[0] = 0x0000;
    rsp.vu.acc_md[0] = 0x0000;
    rsp.vu.acc_lo[0] = 0x5678;
    assert_eq!(rsp.vu.clamp_acc_low(0), 0x5678);

    rsp.vu.acc_hi[0] = 0x0000;
    rsp.vu.acc_md[0] = 0x7FFF;
    rsp.vu.acc_lo[0] = 0xABCD;
    assert_eq!(rsp.vu.clamp_acc_low(0), 0xABCD);

    rsp.vu.acc_hi[0] = 0x0000;
    rsp.vu.acc_md[0] = 0x8000;
    rsp.vu.acc_lo[0] = 0xABCD;
    assert_eq!(rsp.vu.clamp_acc_low(0), 0xFFFF);
}

#[test]
fn test_vaddc_vadd_and_vsubc_vsub_8lane_32bit_audio_phase() {
    let mut rsp = InterpRsp::new();

    rsp.vu.vpr[1] = [
        0xFFFF, 0x8000, 0x1234, 0xFFFE, 0x0000, 0x7FFF, 0x0100, 0xFFFF,
    ];
    rsp.vu.vpr[2] = [
        0x0001, 0x8000, 0x0001, 0x0002, 0x0000, 0x0001, 0x0100, 0xFFFF,
    ];

    // VADDC (funct 0x14)
    let vaddc_word = enc_vu(0, 2, 1, 3, 0x14);
    exec_vu_arith(&mut rsp, 0, vaddc_word).unwrap();
    assert_eq!(rsp.vu.vco & 0xFF, 139);

    // VADD (funct 0x10)
    let vadd_word = enc_vu(0, 2, 1, 3, 0x10);
    exec_vu_arith(&mut rsp, 0, vadd_word).unwrap();
    assert_eq!(rsp.vu.vco, 0);

    // VSUBC (funct 0x15) and VSUB (funct 0x11)
    rsp.vu.reset();
    rsp.vu.vpr[1] = [0x0000, 0x1000, 0x5000, 0x0000, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [0x0001, 0x2000, 0x4000, 0x0000, 0, 0, 0, 0];
    let vsubc_word = enc_vu(0, 2, 1, 3, 0x15);
    exec_vu_arith(&mut rsp, 0, vsubc_word).unwrap();
    assert!((rsp.vu.vco & 1) != 0);
    assert!((rsp.vu.vco & 0x100) != 0);

    let vsub_word = enc_vu(0, 2, 1, 3, 0x11);
    exec_vu_arith(&mut rsp, 0, vsub_word).unwrap();
    assert_eq!(rsp.vu.vco, 0);
}

#[test]
fn test_vsaw_filter_accumulator_readout() {
    let mut rsp = InterpRsp::new();

    rsp.vu.vpr[1] = [100, 200, 300, 400, 500, 600, 700, 800];
    rsp.vu.vpr[2] = [10, 20, 30, 40, 50, 60, 70, 80];

    // VMULF (funct 0x00)
    let vmulf_word = enc_vu(0, 2, 1, 3, 0x00);
    exec_vu_arith(&mut rsp, 0, vmulf_word).unwrap();

    // 3x VMACF (funct 0x08)
    let vmacf_word = enc_vu(0, 2, 1, 3, 0x08);
    for _ in 0..3 {
        exec_vu_arith(&mut rsp, 0, vmacf_word).unwrap();
    }

    // VSAW with e = 8 (acc_hi), e = 9 (acc_md), e = 10 (acc_lo)
    let vsaw_hi = enc_vu(8, 0, 0, 3, 0x1D);
    exec_vu_arith(&mut rsp, 0, vsaw_hi).unwrap();
    assert_eq!(rsp.vu.vpr[3], rsp.vu.acc_hi);

    let vsaw_md = enc_vu(9, 0, 0, 3, 0x1D);
    exec_vu_arith(&mut rsp, 0, vsaw_md).unwrap();
    assert_eq!(rsp.vu.vpr[3], rsp.vu.acc_md);

    let vsaw_lo = enc_vu(10, 0, 0, 3, 0x1D);
    exec_vu_arith(&mut rsp, 0, vsaw_lo).unwrap();
    assert_eq!(rsp.vu.vpr[3], rsp.vu.acc_lo);
}

#[test]
fn test_lqv_lsv_ssv_sqv_stereo_dmem_wrapping_interleave() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, true);

    let test_data: [u8; 32] = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99,
    ];
    for (i, &b) in test_data.iter().enumerate() {
        bus.sp_dmem[(0x0FF8 + i) & 0x0FFF] = b;
    }

    rsp.set_gpr(1, 0x0FF8);

    // LSV and SSV at unaligned DMEM wrapping boundaries
    let lsv_word = (0x32 << 26) | (1 << 21) | (5 << 16) | (0x01 << 11);
    exec_lwc2(&mut rsp, &bus, 0, lsv_word).unwrap();
    assert_eq!(rsp.vu.vpr[5][0], 0x1234);

    let ssv_word = (0x3A << 26) | (1 << 21) | (5 << 16) | (0x01 << 11) | 1;
    exec_swc2(&mut rsp, &mut bus, 0, ssv_word).unwrap();
    assert_eq!(bus.sp_dmem[0x0FFA], 0x12);
    assert_eq!(bus.sp_dmem[0x0FFB], 0x34);

    // LQV and SQV wrapping interleave across 0x0FFF -> 0x0000
    let lqv_word = (0x32 << 26) | (1 << 21) | (6 << 16) | (0x04 << 11);
    exec_lwc2(&mut rsp, &bus, 0, lqv_word).unwrap();
    assert_eq!(rsp.vu.vpr[6][0], 0x1234);
    assert_eq!(rsp.vu.vpr[6][1], 0x1234);

    rsp.set_gpr(1, 0x0FFF);
    let ssv_wrap = (0x3A << 26) | (1 << 21) | (6 << 16) | (0x01 << 11);
    exec_swc2(&mut rsp, &mut bus, 0, ssv_wrap).unwrap();
    assert_eq!(bus.sp_dmem[0x0FFF], 0x12);
    assert_eq!(bus.sp_dmem[0x0000], 0x34);

    let sqv_word = (0x3A << 26) | (1 << 21) | (6 << 16) | (0x04 << 11);
    exec_swc2(&mut rsp, &mut bus, 0, sqv_word).unwrap();
    assert_eq!(bus.sp_dmem[0x0FFF], 0x12);
}
