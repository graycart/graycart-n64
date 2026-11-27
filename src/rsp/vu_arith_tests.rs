//! Reality Signal Processor (RSP) Vector Unit Arithmetic Unit Tests.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit Arithmetic)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

use crate::rsp::vu_arith::exec_vu_arith;
use crate::rsp::InterpRsp;

fn enc_vu(e: u32, vt: u32, vs: u32, vd: u32, funct: u32) -> u32 {
    (0x12 << 26) | (1 << 25) | (e << 21) | (vt << 16) | (vs << 11) | (vd << 6) | funct
}

#[test]
fn test_vmulf_vmulu_clamping() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vpr[1] = [0x8000; 8]; // -32768
    rsp.vu.vpr[2] = [0x8000; 8]; // -32768

    exec_vu_arith(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x00)).unwrap(); // VMULF
    assert_eq!(rsp.vu.vpr[3], [0x7FFF; 8]);

    exec_vu_arith(&mut rsp, 0, enc_vu(0, 2, 1, 4, 0x01)).unwrap(); // VMULU
    assert_eq!(rsp.vu.vpr[4], [0xFFFF; 8]);
}

#[test]
fn test_32bit_fixed_point_multiply_chain_and_vsaw() {
    let mut rsp = InterpRsp::new();
    // Compute 0x0001_0002 * 0x0003_0004 (s15.16):
    // hi_a = 1, lo_a = 2; hi_b = 3, lo_b = 4
    // Product = 0x0003_000A_0008 (shifted >> 16 = 0x0003_000A)
    rsp.vu.vpr[1] = [1; 8]; // a_hi
    rsp.vu.vpr[2] = [2; 8]; // a_lo
    rsp.vu.vpr[3] = [3; 8]; // b_hi
    rsp.vu.vpr[4] = [4; 8]; // b_lo

    // VMUDL v5, v2, v4 -> (2 * 4) >> 16 = 0
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 4, 2, 5, 0x04)).unwrap();
    // VMACM v5, v1, v4 -> acc += 1 * 4 = 4
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 4, 1, 5, 0x0D)).unwrap();
    // VMACN v5, v2, v3 -> acc += 2 * 3 = 10
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 3, 2, 5, 0x0E)).unwrap();
    // VMACH v6, v1, v3 -> acc += (1 * 3) << 16 = 0x0003_000A
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 3, 1, 6, 0x0F)).unwrap();

    // VSAW: e=8 acc_hi, e=9 acc_md, e=10 acc_lo
    exec_vu_arith(&mut rsp, 0, enc_vu(8, 0, 0, 7, 0x1D)).unwrap();
    exec_vu_arith(&mut rsp, 0, enc_vu(9, 0, 0, 8, 0x1D)).unwrap();
    exec_vu_arith(&mut rsp, 0, enc_vu(10, 0, 0, 9, 0x1D)).unwrap();
    assert_eq!(rsp.vu.vpr[7], [0; 8]);
    assert_eq!(rsp.vu.vpr[8], [3; 8]);
    assert_eq!(rsp.vu.vpr[9], [10; 8]);
}

#[test]
fn test_vaddc_vadd_and_vsubc_vsub_chains() {
    let mut rsp = InterpRsp::new();
    // Low half carry: 0xFFFF + 0x0002 -> sum = 1, carry = 1
    rsp.vu.vpr[1] = [0xFFFF, 10, 0, 0, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [0x0002, 20, 0, 0, 0, 0, 0, 0];
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x14)).unwrap(); // VADDC
    assert_eq!(rsp.vu.vpr[3][0], 1);
    assert_eq!(rsp.vu.vco, 0x0001);

    // High half add with carry + clamp
    rsp.vu.vpr[4] = [0x7FFF, 100, 0, 0, 0, 0, 0, 0];
    rsp.vu.vpr[5] = [0, 50, 0, 0, 0, 0, 0, 0];
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 5, 4, 6, 0x10)).unwrap(); // VADD
    assert_eq!(rsp.vu.vpr[6][0], 0x7FFF); // 0x7FFF + 0 + 1 clamped
    assert_eq!(rsp.vu.vpr[6][1], 150);
    assert_eq!(rsp.vu.vco, 0);

    // VSUBC sets borrow + not-equal
    rsp.vu.vpr[1] = [5, 5, 10, 0, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [10, 5, 3, 0, 0, 0, 0, 0];
    exec_vu_arith(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x15)).unwrap(); // VSUBC
    assert_eq!(rsp.vu.vco & 0xFF, 0b001); // lane 0 borrowed
    assert_eq!(rsp.vu.vco >> 8, 0b101); // lanes 0 and 2 not equal
}

#[test]
fn test_vabs_edge() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vpr[1] = [0xFFFF, 0, 0x0001, 0, 0, 0, 0, 0];
    rsp.vu.vpr[2] = [0x8000, 50, 0x8000, 0, 0, 0, 0, 0];

    exec_vu_arith(&mut rsp, 0, enc_vu(0, 2, 1, 3, 0x13)).unwrap(); // VABS
    assert_eq!(rsp.vu.vpr[3][0], 0x7FFF);
    assert_eq!(rsp.vu.acc_lo[0], 0x8000);
    assert_eq!(rsp.vu.vpr[3][1], 0);
    assert_eq!(rsp.vu.vpr[3][2], 0x8000);
}

#[test]
fn test_unknown_funct_trap() {
    let mut rsp = InterpRsp::new();
    assert!(exec_vu_arith(&mut rsp, 0x100, enc_vu(0, 2, 1, 3, 0x3E)).is_err());
}
