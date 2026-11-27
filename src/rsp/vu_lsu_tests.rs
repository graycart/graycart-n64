//! Unit tests for RSP Vector Unit Load/Store (`LWC2`/`SWC2`).
//!
//! Cited: N64brew — Reality Signal Processor (Vector Load/Store)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

#![allow(clippy::identity_op)]

use crate::bus::Bus;
use crate::rsp::vu_lsu::{exec_lwc2, exec_swc2};
use crate::rsp::InterpRsp;
use crate::trap::Trap;

#[test]
fn test_lbv_sbv() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);
    rsp.set_gpr(1, 0x100);

    rsp.write_dmem_u8(&mut bus, 0x100, 0x5A);
    // LBV v2[4], 0(r1): op=0x32, base=1, vt=2, sub=0x0, e=4, off=0
    let word_lbv = (0x32 << 26) | (1 << 21) | (2 << 16) | (0 << 11) | (4 << 7) | 0;
    exec_lwc2(&mut rsp, &bus, 0, word_lbv).unwrap();
    assert_eq!(rsp.vu.get_byte(2, 4), 0x5A);

    // SBV v2[4], 0(r1): op=0x3A, base=1, vt=2, sub=0x0, e=4, off=0
    let word_sbv = (0x3A << 26) | (1 << 21) | (2 << 16) | (0 << 11) | (4 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_sbv).unwrap();
    assert_eq!(rsp.read_dmem_u8(&bus, 0x100), 0x5A);
}

#[test]
fn test_lsv_ssv_llv_slv_ldv_sdv() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);
    rsp.set_gpr(1, 0x200);

    // Write test bytes into DMEM
    for i in 0..16 {
        rsp.write_dmem_u8(&mut bus, 0x200 + i, (i as u8) + 0x10);
    }

    // LSV v3[0], 0(r1) -> 2 bytes
    let word_lsv = (0x32 << 26) | (1 << 21) | (3 << 16) | (1 << 11) | (0 << 7) | 0;
    exec_lwc2(&mut rsp, &bus, 0, word_lsv).unwrap();
    assert_eq!(rsp.vu.get_byte(3, 0), 0x10);
    assert_eq!(rsp.vu.get_byte(3, 1), 0x11);

    // SSV v3[0], 0(r1) -> stores 2 bytes back at 0x200
    let word_ssv = (0x3A << 26) | (1 << 21) | (3 << 16) | (1 << 11) | (0 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_ssv).unwrap();
    assert_eq!(rsp.read_dmem_u8(&bus, 0x200), 0x10);
    assert_eq!(rsp.read_dmem_u8(&bus, 0x201), 0x11);

    // LLV v4[0], 0(r1) -> 4 bytes
    let word_llv = (0x32 << 26) | (1 << 21) | (4 << 16) | (2 << 11) | (0 << 7) | 0;
    exec_lwc2(&mut rsp, &bus, 0, word_llv).unwrap();
    for i in 0..4 {
        assert_eq!(rsp.vu.get_byte(4, i), 0x10 + i as u8);
    }

    // SLV v4[0], 0(r1) -> 4 bytes at 0x200
    let word_slv = (0x3A << 26) | (1 << 21) | (4 << 16) | (2 << 11) | (0 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_slv).unwrap();
    for i in 0..4 {
        assert_eq!(rsp.read_dmem_u8(&bus, 0x200 + i), 0x10 + i as u8);
    }

    // LDV v5[0], 0(r1) -> 8 bytes
    let word_ldv = (0x32 << 26) | (1 << 21) | (5 << 16) | (3 << 11) | (0 << 7) | 0;
    exec_lwc2(&mut rsp, &bus, 0, word_ldv).unwrap();
    for i in 0..8 {
        assert_eq!(rsp.vu.get_byte(5, i), 0x10 + i as u8);
    }

    // SDV v5[0], 0(r1) -> 8 bytes at 0x200
    let word_sdv = (0x3A << 26) | (1 << 21) | (5 << 16) | (3 << 11) | (0 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_sdv).unwrap();
    for i in 0..8 {
        assert_eq!(rsp.read_dmem_u8(&bus, 0x200 + i), 0x10 + i as u8);
    }
}

#[test]
fn test_lqv_lrv_sqv_srv_roundtrip() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);
    rsp.set_gpr(1, 0x300); // aligned to 16

    for i in 0..16 {
        rsp.write_dmem_u8(&mut bus, 0x300 + i, (0xA0_u32 + i) as u8);
    }

    // LQV v6[0], 0(r1) -> loads 16 bytes into v6[0..16]
    let word_lqv = (0x32 << 26) | (1 << 21) | (6 << 16) | (4 << 11) | (0 << 7) | 0;
    exec_lwc2(&mut rsp, &bus, 0, word_lqv).unwrap();

    for i in 0..16 {
        assert_eq!(
            rsp.vu.get_byte(6, i),
            rsp.read_dmem_u8(&bus, 0x300 + i as u32)
        );
    }

    rsp.set_gpr(2, 0x400);
    let word_sqv = (0x3A << 26) | (2 << 21) | (6 << 16) | (4 << 11) | (0 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_sqv).unwrap();

    for i in 0..16 {
        assert_eq!(
            rsp.read_dmem_u8(&bus, 0x400 + i as u32),
            rsp.read_dmem_u8(&bus, 0x300 + i as u32)
        );
    }
}

#[test]
fn test_packed_unpacked_half_fourth_swv() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);
    rsp.set_gpr(1, 0x500);

    for i in 0..16 {
        rsp.write_dmem_u8(&mut bus, 0x500 + i, 0x30 + i as u8);
    }

    // LPV
    let word_lpv = (0x32 << 26) | (1 << 21) | (7 << 16) | (6 << 11) | (0 << 7) | 0;
    exec_lwc2(&mut rsp, &bus, 0, word_lpv).unwrap();
    assert_eq!(rsp.vu.vpr[7][0], (0x30u16) << 8);

    // SPV
    let word_spv = (0x3A << 26) | (1 << 21) | (7 << 16) | (6 << 11) | (0 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_spv).unwrap();
    assert_eq!(rsp.read_dmem_u8(&bus, 0x500), 0x30);

    // SWV
    let word_swv = (0x3A << 26) | (1 << 21) | (7 << 16) | (0x0A << 11) | (0 << 7) | 0;
    exec_swc2(&mut rsp, &mut bus, 0, word_swv).unwrap();
    assert_eq!(rsp.read_dmem_u8(&bus, 0x500), 0x30);
}

#[test]
fn test_matrix_transpose_ltv_stv() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);
    rsp.set_gpr(1, 0x600);

    // Populate an asymmetric 8x8 matrix in DMEM: input[row][col] = (row << 8) | col
    for row in 0..8 {
        for col in 0..8 {
            let val = ((row << 8) | col) as u16;
            let bytes = val.to_be_bytes();
            rsp.write_dmem_u8(&mut bus, 0x600 + (row * 16) + (col * 2), bytes[0]);
            rsp.write_dmem_u8(&mut bus, 0x600 + (row * 16) + (col * 2) + 1, bytes[1]);
        }
    }

    // Execute 8 LTV instructions with e = 0, 2, 4, 6, 8, 10, 12, 14
    for row in 0..8 {
        let e = (row * 2) as u32;
        let word_ltv = (0x32 << 26) | (1 << 21) | (8 << 16) | (0x0B << 11) | (e << 7) | row as u32;
        exec_lwc2(&mut rsp, &bus, 0, word_ltv).unwrap();
    }

    // Execute 8 STV instructions with e = 0, 2, 4, 6, 8, 10, 12, 14 to store transposed matrix at 0x700
    rsp.set_gpr(2, 0x700);
    for col in 0..8 {
        let e = (col * 2) as u32;
        let word_stv = (0x3A << 26) | (2 << 21) | (8 << 16) | (0x0B << 11) | (e << 7) | col as u32;
        exec_swc2(&mut rsp, &mut bus, 0, word_stv).unwrap();
    }

    // Verify true transpose: output[col][row] == input[row][col] == (row << 8) | col
    for col in 0..8 {
        for row in 0..8 {
            let expected = ((row << 8) | col) as u16;
            let b0 = rsp.read_dmem_u8(&bus, 0x700 + (col * 16) + (row * 2));
            let b1 = rsp.read_dmem_u8(&bus, 0x700 + (col * 16) + (row * 2) + 1);
            let actual = u16::from_be_bytes([b0, b1]);
            assert_eq!(
                actual, expected,
                "Mismatch at transposed col {col} row {row}: got 0x{actual:04X}, expected 0x{expected:04X}"
            );
        }
    }
}

#[test]
fn test_unimplemented_sub_traps() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);
    let word_bad_lwc2 = (0x32 << 26) | (1 << 21) | (1 << 16) | (0x0C << 11);
    let res = exec_lwc2(&mut rsp, &bus, 0x10, word_bad_lwc2);
    assert!(matches!(
        res,
        Err(Trap::UnimplementedOpcode { pc: 0x10, .. })
    ));

    let word_bad_swc2 = (0x3A << 26) | (1 << 21) | (1 << 16) | (0x0C << 11);
    let res2 = exec_swc2(&mut rsp, &mut bus, 0x14, word_bad_swc2);
    assert!(matches!(
        res2,
        Err(Trap::UnimplementedOpcode { pc: 0x14, .. })
    ));
}
