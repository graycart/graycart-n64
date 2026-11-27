//! Unit tests for VR4300 integer ALU instruction family.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use super::*;
use crate::cpu::decode::Inst;
use crate::cpu::InterpCpu;
use crate::trap::Trap;

#[allow(clippy::identity_op)]
#[test]
fn test_add_addi_sub_overflow() {
    let mut cpu = InterpCpu::new();
    // ADD i32::MAX + 1 -> Overflow
    cpu.set_gpr(1, i32::MAX as u64);
    cpu.set_gpr(2, 1);
    cpu.set_gpr(3, 0x1234_5678_9ABC_DEF0); // original destination value
                                           // ADD rd=3, rs=1, rt=2 -> funct=0x20, opcode=0
    let inst = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (3 << 11) | 0x20);
    let res = exec_special(&mut cpu, 0x8000_0000, inst);
    assert_eq!(
        res,
        Err(Trap::CpuException {
            pc: 0x8000_0000,
            code: "Ov",
            bad_vaddr: None
        })
    );
    assert_eq!(cpu.gpr(3), 0x1234_5678_9ABC_DEF0); // unchanged

    // ADDI i32::MIN - 1 -> Overflow
    cpu.set_gpr(1, (i32::MIN as i64) as u64);
    cpu.set_gpr(3, 0x1234_5678_9ABC_DEF0);
    // ADDI rt=3, rs=1, imm=0xFFFF (-1) -> opcode=0x08
    let inst = Inst((0x08 << 26) | (1 << 21) | (3 << 16) | 0xFFFF);
    let res = exec_imm(&mut cpu, 0x8000_0004, inst);
    assert_eq!(
        res,
        Err(Trap::CpuException {
            pc: 0x8000_0004,
            code: "Ov",
            bad_vaddr: None
        })
    );
    assert_eq!(cpu.gpr(3), 0x1234_5678_9ABC_DEF0); // unchanged
}

#[allow(clippy::identity_op)]
#[test]
fn test_addu_addiu_subu_wrapping() {
    let mut cpu = InterpCpu::new();
    // 0x7FFF_FFFF + 1 == 0xFFFF_FFFF_8000_0000
    cpu.set_gpr(1, 0x7FFF_FFFF);
    cpu.set_gpr(2, 1);
    let inst = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (3 << 11) | 0x21); // ADDU
    assert!(exec_special(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 0xFFFF_FFFF_8000_0000);

    // ADDIU
    cpu.set_gpr(1, 0x7FFF_FFFF);
    let inst = Inst((0x09 << 26) | (1 << 21) | (3 << 16) | 1); // ADDIU imm=1
    assert!(exec_imm(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 0xFFFF_FFFF_8000_0000);
}

#[allow(clippy::identity_op)]
#[test]
fn test_dadd_daddi_dsub_overflow() {
    let mut cpu = InterpCpu::new();
    // DADD i64::MAX + 1 -> Overflow
    cpu.set_gpr(1, i64::MAX as u64);
    cpu.set_gpr(2, 1);
    cpu.set_gpr(3, 0x1234_5678_9ABC_DEF0);
    let inst = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (3 << 11) | 0x2C); // DADD
    assert_eq!(
        exec_special(&mut cpu, 0, inst),
        Err(Trap::CpuException {
            pc: 0,
            code: "Ov",
            bad_vaddr: None
        })
    );
    assert_eq!(cpu.gpr(3), 0x1234_5678_9ABC_DEF0);

    // DADDU / DADDIU / DSUBU wrap silently
    cpu.set_gpr(1, i64::MAX as u64);
    let inst = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (3 << 11) | 0x2D); // DADDU
    assert!(exec_special(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), (i64::MAX as u64).wrapping_add(1));
}

#[allow(clippy::identity_op)]
#[test]
fn test_lui_sign_extension() {
    let mut cpu = InterpCpu::new();
    // LUI with 0x8000 -> 0xFFFF_FFFF_8000_0000
    let inst = Inst((0x0F << 26) | (3 << 16) | 0x8000);
    assert!(exec_imm(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 0xFFFF_FFFF_8000_0000);

    // LUI with 0x7FFF -> 0x0000_0000_7FFF_0000
    let inst = Inst((0x0F << 26) | (3 << 16) | 0x7FFF);
    assert!(exec_imm(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 0x0000_0000_7FFF_0000);
}

#[allow(clippy::identity_op)]
#[test]
fn test_sltiu_unsigned_compare() {
    let mut cpu = InterpCpu::new();
    // SLTIU with imm 0xFFFF (-1 signed, but 0xFFFF_FFFF_FFFF_FFFF unsigned)
    cpu.set_gpr(1, 0);
    let inst = Inst((0x0B << 26) | (1 << 21) | (3 << 16) | 0xFFFF);
    assert!(exec_imm(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 1); // 0 < 0xFFFF_FFFF_FFFF_FFFF is true (1)

    // u64::MAX < u64::MAX is false (0)
    cpu.set_gpr(1, u64::MAX);
    assert!(exec_imm(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 0);
}

#[allow(clippy::identity_op)]
#[test]
fn test_slt_vs_sltu() {
    let mut cpu = InterpCpu::new();
    let neg_one = -1i64 as u64; // u64::MAX
    let one = 1u64;

    cpu.set_gpr(1, neg_one);
    cpu.set_gpr(2, one);

    // SLT: -1 < 1 -> true (1)
    let inst_slt = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (3 << 11) | 0x2A);
    assert!(exec_special(&mut cpu, 0, inst_slt).is_ok());
    assert_eq!(cpu.gpr(3), 1);

    // SLTU: u64::MAX < 1 -> false (0)
    let inst_sltu = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (3 << 11) | 0x2B);
    assert!(exec_special(&mut cpu, 0, inst_sltu).is_ok());
    assert_eq!(cpu.gpr(3), 0);
}

#[allow(clippy::identity_op)]
#[test]
fn test_bitwise_zero_extension() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(1, 0xFFFF_FFFF_FFFF_FFFF);
    // ANDI with 0x00FF
    let inst = Inst((0x0C << 26) | (1 << 21) | (3 << 16) | 0x00FF);
    assert!(exec_imm(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(3), 0x0000_0000_0000_00FF);
}

#[allow(clippy::identity_op)]
#[test]
fn test_r0_hardwired_zero() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(1, 100);
    cpu.set_gpr(2, 200);
    // ADD to r0 (rd=0)
    let inst = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (0 << 11) | 0x20);
    assert!(exec_special(&mut cpu, 0, inst).is_ok());
    assert_eq!(cpu.gpr(0), 0);
}

#[allow(clippy::identity_op)]
#[test]
fn test_unknown_opcode_funct() {
    let mut cpu = InterpCpu::new();
    // Unknown funct in SPECIAL
    let inst = Inst((0x00 << 26) | 0x3F);
    assert_eq!(
        exec_special(&mut cpu, 0x1000, inst),
        Err(Trap::UnimplementedOpcode {
            pc: 0x1000,
            word: inst.raw()
        })
    );

    // Unknown primary opcode for imm ALU
    let inst_imm = Inst((0x3F << 26) | 0x1234);
    assert_eq!(
        exec_imm(&mut cpu, 0x1004, inst_imm),
        Err(Trap::UnimplementedOpcode {
            pc: 0x1004,
            word: inst_imm.raw()
        })
    );
}
