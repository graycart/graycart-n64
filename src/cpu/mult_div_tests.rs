//! Unit tests for NEC VR4300 multiply, divide, and HI/LO instructions.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use super::*;
use crate::trap::Trap;

fn make_inst(opcode: u8, rs: usize, rt: usize, rd: usize, funct: u8) -> Inst {
    let raw = ((opcode as u32) << 26)
        | ((rs as u32) << 21)
        | ((rt as u32) << 16)
        | ((rd as u32) << 11)
        | (funct as u32);
    Inst(raw)
}

#[test]
fn test_mult_signed_extension() {
    let mut cpu = InterpCpu::new();
    // rs = 1 (-2), rt = 2 (-3) -> product = 6
    cpu.set_gpr(1, 0xFFFF_FFFF_FFFF_FFFE);
    cpu.set_gpr(2, 0xFFFF_FFFF_FFFF_FFFD);
    let inst = make_inst(0, 1, 2, 0, 0x18);
    let cycles = exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cycles, 5);
    assert_eq!(cpu.lo(), 6);
    assert_eq!(cpu.hi(), 0);

    // Negative * positive
    cpu.set_gpr(1, 0xFFFF_FFFF_FFFF_FFFE); // -2
    cpu.set_gpr(2, 3); // 3
    let cycles = exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cycles, 5);
    // product = -6 sign-extended into 64-bit HI/LO
    assert_eq!(cpu.lo(), 0xFFFF_FFFF_FFFF_FFFA);
    assert_eq!(cpu.hi(), 0xFFFF_FFFF_FFFF_FFFF);
}

#[test]
fn test_multu_max() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(1, 0xFFFF_FFFF);
    cpu.set_gpr(2, 0xFFFF_FFFF);
    let inst = make_inst(0, 1, 2, 0, 0x19);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 1);
    assert_eq!(cpu.hi(), 0xFFFF_FFFF_FFFF_FFFE);
}

#[test]
fn test_div_edge_cases() {
    let mut cpu = InterpCpu::new();
    // DIV normal: -10 / 3 -> q = -3, r = -1
    cpu.set_gpr(1, 0xFFFF_FFFF_FFFF_FFF6); // -10
    cpu.set_gpr(2, 3);
    let inst = make_inst(0, 1, 2, 0, 0x1A);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 0xFFFF_FFFF_FFFF_FFFD); // -3 sign-extended
    assert_eq!(cpu.hi(), 0xFFFF_FFFF_FFFF_FFFF); // -1 sign-extended

    // DIV by zero (positive n)
    cpu.set_gpr(1, 42);
    cpu.set_gpr(2, 0);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), u64::MAX);
    assert_eq!(cpu.hi(), 42);

    // DIV by zero (negative n)
    cpu.set_gpr(1, 0xFFFF_FFFF_FFFF_FFD6); // -42
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 1);
    assert_eq!(cpu.hi(), 0xFFFF_FFFF_FFFF_FFD6);

    // i32::MIN / -1
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0000);
    cpu.set_gpr(2, 0xFFFF_FFFF_FFFF_FFFF); // -1 as 32-bit
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 0xFFFF_FFFF_8000_0000);
    assert_eq!(cpu.hi(), 0);
}

#[test]
fn test_divu_edge_cases() {
    let mut cpu = InterpCpu::new();
    // DIVU by zero with 0x8000_0001
    cpu.set_gpr(1, 0x8000_0001);
    cpu.set_gpr(2, 0);
    let inst = make_inst(0, 1, 2, 0, 0x1B);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), u64::MAX);
    assert_eq!(cpu.hi(), 0xFFFF_FFFF_8000_0001);

    // DIVU quotient bit 31 set sign-extending LO
    cpu.set_gpr(1, 0x8000_0000);
    cpu.set_gpr(2, 1);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 0xFFFF_FFFF_8000_0000);
    assert_eq!(cpu.hi(), 0);
}

#[test]
fn test_dmult_dmultu() {
    let mut cpu = InterpCpu::new();
    // DMULT: i64::MIN * i64::MIN
    cpu.set_gpr(1, i64::MIN as u64);
    cpu.set_gpr(2, i64::MIN as u64);
    let inst = make_inst(0, 1, 2, 0, 0x1C);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 0);
    assert_eq!(cpu.hi(), 0x4000_0000_0000_0000);

    // DMULTU: u64::MAX * u64::MAX
    cpu.set_gpr(1, u64::MAX);
    cpu.set_gpr(2, u64::MAX);
    let inst = make_inst(0, 1, 2, 0, 0x1D);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 1);
    assert_eq!(cpu.hi(), 0xFFFF_FFFF_FFFF_FFFE);
}

#[test]
fn test_ddiv_edge_cases() {
    let mut cpu = InterpCpu::new();
    // DDIV by zero (positive)
    cpu.set_gpr(1, 100);
    cpu.set_gpr(2, 0);
    let inst = make_inst(0, 1, 2, 0, 0x1E);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), u64::MAX);
    assert_eq!(cpu.hi(), 100);

    // DDIV by zero (negative)
    cpu.set_gpr(1, -100i64 as u64);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 1);
    assert_eq!(cpu.hi(), -100i64 as u64);

    // i64::MIN / -1
    cpu.set_gpr(1, i64::MIN as u64);
    cpu.set_gpr(2, -1i64 as u64);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), i64::MIN as u64);
    assert_eq!(cpu.hi(), 0);
}

#[test]
fn test_ddivu_edge_cases() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(1, 12345);
    cpu.set_gpr(2, 0);
    let inst = make_inst(0, 1, 2, 0, 0x1F);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), u64::MAX);
    assert_eq!(cpu.hi(), 12345);
}

#[test]
fn test_mthi_mtlo_mfhi_mflo() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(1, 0x1234_5678_9ABC_DEF0);
    cpu.set_gpr(2, 0xFEDC_BA98_7654_3210);

    // MTHI r1
    let inst = make_inst(0, 1, 0, 0, 0x11);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.hi(), 0x1234_5678_9ABC_DEF0);

    // MTLO r2
    let inst = make_inst(0, 2, 0, 0, 0x13);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.lo(), 0xFEDC_BA98_7654_3210);

    // MFHI r3
    let inst = make_inst(0, 0, 0, 3, 0x10);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(3), 0x1234_5678_9ABC_DEF0);

    // MFLO r4
    let inst = make_inst(0, 0, 0, 4, 0x12);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(4), 0xFEDC_BA98_7654_3210);

    // MFHI r0 (should discard write)
    let inst = make_inst(0, 0, 0, 0, 0x10);
    exec_special(&mut cpu, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(0), 0);
}

#[test]
fn test_unimplemented_funct() {
    let mut cpu = InterpCpu::new();
    let inst = make_inst(0, 0, 0, 0, 0x17);
    let res = exec_special(&mut cpu, 0x1000, inst);
    assert_eq!(
        res,
        Err(Trap::UnimplementedOpcode {
            pc: 0x1000,
            word: inst.raw()
        })
    );
}
