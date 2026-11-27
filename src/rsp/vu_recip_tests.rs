//! Unit tests for RSP Vector Unit COP2 moves and reciprocal/inverse-sqrt lookup tables.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit & Reciprocal Tables)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

#![allow(clippy::identity_op)]

use super::*;
use crate::trap::Trap;

fn enc_vu(e: u32, vt: u32, vs: u32, vd: u32, funct: u32) -> u32 {
    (0x12 << 26) | (1 << 25) | (e << 21) | (vt << 16) | (vs << 11) | (vd << 6) | funct
}

#[test]
fn test_cop2_moves_mfc_mtc() {
    let mut rsp = InterpRsp::new();
    // MTC2 transfers low 16 bits (0x5678) into v1 at byte offset 0
    rsp.set_gpr(1, 0x1234_5678);
    let mtc2_word = (0x12 << 26) | (4 << 21) | (1 << 16) | (1 << 11) | (0 << 7);
    exec_cop2_move(&mut rsp, 0, mtc2_word).unwrap();
    assert_eq!(rsp.vu.get_byte(1, 0), 0x56);
    assert_eq!(rsp.vu.get_byte(1, 1), 0x78);

    // MFC2 sign-extends negative 16-bit halfword and wraps byte 15 -> 0
    rsp.set_gpr(3, 0xABCC);
    let mtc2_e15 = (0x12 << 26) | (4 << 21) | (3 << 16) | (1 << 11) | (15 << 7);
    exec_cop2_move(&mut rsp, 4, mtc2_e15).unwrap();
    // MTC2 at e=15 writes ONLY byte 15 (0xAB), leaving byte 0 untouched (0x56)!
    assert_eq!(rsp.vu.get_byte(1, 15), 0xAB);
    assert_eq!(rsp.vu.get_byte(1, 0), 0x56);

    // MFC2 at e=15 wraps byte 15..0 -> 0xAB56 sign-extended to 0xFFFF_AB56
    let mfc2_e15 = (0x12 << 26) | (0 << 21) | (2 << 16) | (1 << 11) | (15 << 7);
    exec_cop2_move(&mut rsp, 8, mfc2_e15).unwrap();
    assert_eq!(rsp.gpr(2), 0xFFFF_AB56);
}

#[test]
fn test_cop2_control_transfers() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vco = 0xFFF0;
    rsp.vu.vcc = 0x00FF;
    rsp.vu.vce = 0x5A;

    let cfc2_vco = (0x12 << 26) | (2 << 21) | (1 << 16) | (0 << 11);
    exec_cop2_move(&mut rsp, 0, cfc2_vco).unwrap();
    assert_eq!(rsp.gpr(1), 0xFFFF_FFF0);

    let cfc2_vce = (0x12 << 26) | (2 << 21) | (2 << 16) | (2 << 11);
    exec_cop2_move(&mut rsp, 4, cfc2_vce).unwrap();
    assert_eq!(rsp.gpr(2), 0x5A);

    rsp.set_gpr(3, 0x1234);
    let ctc2_vcc = (0x12 << 26) | (6 << 21) | (3 << 16) | (1 << 11);
    exec_cop2_move(&mut rsp, 8, ctc2_vcc).unwrap();
    assert_eq!(rsp.vu.vcc, 0x1234);
}

#[test]
fn test_vrcp_vrcpl_vrcph_and_vrsq_vrsql_vrsqh() {
    let mut rsp = InterpRsp::new();
    rsp.vu.vpr[2] = [0xBEEF; 8];

    // Special values
    assert_eq!(compute_rcp(0), 0x7FFF_FFFF);
    assert_eq!(compute_rcp(-32768), 0xFFFF_0000);
    assert_eq!(compute_rcp(-1), 0x8000_0000);
    assert_eq!(compute_rcp(2), 0x3FFF_E000);
    assert_eq!(compute_rsq(0), 0x7FFF_FFFF);
    assert_eq!(compute_rsq(-1), 0x8000_0000);
    assert_eq!(compute_rsq(4), 0x3FFF_E000);

    // 32-bit VRCPH + VRCPL + VRCPH readback for 0x0001_0000 (1.0 in s15.16 -> ~0x0000_7FFF)
    rsp.vu.vpr[1][0] = 0x0001; // hi
    rsp.vu.vpr[1][1] = 0x0000; // lo
    exec_vu_recip(&mut rsp, 0, enc_vu(8, 1, 0, 2, 0x32)).unwrap(); // VRCPH loads div_in=1
    exec_vu_recip(&mut rsp, 4, enc_vu(9, 1, 3, 2, 0x31)).unwrap(); // VRCPL computes rcp(0x0001_0000) -> lane 3
    exec_vu_recip(&mut rsp, 8, enc_vu(8, 1, 4, 2, 0x32)).unwrap(); // VRCPH reads hi -> lane 4
    let rcp_1 = (u32::from(rsp.vu.vpr[2][4]) << 16) | u32::from(rsp.vu.vpr[2][3]);
    assert_eq!(rcp_1, 0x0000_7FFF);
    // Unwritten lane 7 preserved!
    assert_eq!(rsp.vu.vpr[2][7], 0xBEEF);

    // 32-bit VRSQH + VRSQL + VRSQH for 0x0004_0000 (4.0 -> ~0x003F_FFE0)
    rsp.vu.vpr[1][0] = 0x0004;
    rsp.vu.vpr[1][1] = 0x0000;
    exec_vu_recip(&mut rsp, 12, enc_vu(8, 1, 0, 3, 0x36)).unwrap(); // VRSQH
    exec_vu_recip(&mut rsp, 16, enc_vu(9, 1, 1, 3, 0x35)).unwrap(); // VRSQL -> lo lane 1
    exec_vu_recip(&mut rsp, 20, enc_vu(8, 1, 2, 3, 0x36)).unwrap(); // VRSQH -> hi lane 2
    let rsq_4 = (u32::from(rsp.vu.vpr[3][2]) << 16) | u32::from(rsp.vu.vpr[3][1]);
    assert_eq!(rsq_4, 0x003F_FFE0);
}

#[test]
fn test_unimplemented_traps() {
    let mut rsp = InterpRsp::new();
    let bad_cop2 = (0x12 << 26) | (0x1F << 21);
    assert!(matches!(
        exec_cop2_move(&mut rsp, 0, bad_cop2),
        Err(Trap::UnimplementedOpcode { .. })
    ));

    let bad_recip = enc_vu(0, 0, 0, 0, 0x3E);
    assert!(matches!(
        exec_vu_recip(&mut rsp, 0, bad_recip),
        Err(Trap::UnimplementedOpcode { .. })
    ));
}
