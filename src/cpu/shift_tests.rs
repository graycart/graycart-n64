//! Unit tests for NEC VR4300 shift instructions.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

#![allow(clippy::identity_op)]

use crate::bus::Bus;
use crate::cpu::decode::Inst;
use crate::cpu::shift::exec_special;
use crate::cpu::InterpCpu;
use crate::schedule::Scheduler;
use crate::trap::Trap;

fn helper_exec(cpu: &mut InterpCpu, word: u32) -> Result<u64, Trap> {
    let _bus = Bus::new(false, false);
    let _sched = Scheduler::new();
    exec_special(cpu, 0x8000_0000, Inst(word))
}

#[test]
fn test_sll_sign_extension() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(2, 0x0000_0000_8000_0001);
    // SLL r1, r2, 0 -> opcode 0, rs=0, rt=2, rd=1, sa=0, funct=0
    let word = (0 << 26) | (0 << 21) | (2 << 16) | (1 << 11) | (0 << 6) | 0x00;
    helper_exec(&mut cpu, word).unwrap();
    assert_eq!(cpu.gpr(1), 0xFFFF_FFFF_8000_0001);
}

#[test]
fn test_srl_edge_cases() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(2, 0x0000_0000_8000_0001);
    // SRL r1, r2, 0 -> funct=0x02, sa=0
    let word_zero = (0 << 26) | (0 << 21) | (2 << 16) | (1 << 11) | (0 << 6) | 0x02;
    helper_exec(&mut cpu, word_zero).unwrap();
    assert_eq!(cpu.gpr(1), 0xFFFF_FFFF_8000_0001);

    // SRL r3, r2, 1 -> funct=0x02, sa=1
    let word_one = (0 << 26) | (0 << 21) | (2 << 16) | (3 << 11) | (1 << 6) | 0x02;
    helper_exec(&mut cpu, word_one).unwrap();
    assert_eq!(cpu.gpr(3), 0x0000_0000_4000_0000);
}

#[test]
fn test_sra_sign_extension() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(2, 0x0000_0000_8000_0000);
    // SRA r1, r2, 31 -> funct=0x03, sa=31
    let word = (0 << 26) | (0 << 21) | (2 << 16) | (1 << 11) | (31 << 6) | 0x03;
    helper_exec(&mut cpu, word).unwrap();
    assert_eq!(cpu.gpr(1), u64::MAX);
}

#[test]
fn test_shift_variable_masking_5bit() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(3, 33); // rs = 33 -> mask to 1
    cpu.set_gpr(2, 0x0000_0000_0000_0001);
    // SLLV r1, r2, r3 -> funct=0x04, rs=3, rt=2, rd=1
    let word = (0 << 26) | (3 << 21) | (2 << 16) | (1 << 11) | (0 << 6) | 0x04;
    helper_exec(&mut cpu, word).unwrap();
    assert_eq!(cpu.gpr(1), 0x0000_0000_0000_0002);
}

#[test]
fn test_dshift_variable_masking_6bit() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(3, 65); // rs = 65 -> mask to 1
    cpu.set_gpr(2, 0x0000_0000_0000_0001);
    // DSLLV r1, r2, r3 -> funct=0x14, rs=3, rt=2, rd=1
    let word = (0 << 26) | (3 << 21) | (2 << 16) | (1 << 11) | (0 << 6) | 0x14;
    helper_exec(&mut cpu, word).unwrap();
    assert_eq!(cpu.gpr(1), 0x0000_0000_0000_0002);

    cpu.set_gpr(3, 63);
    cpu.set_gpr(2, 1);
    let word63 = (0 << 26) | (3 << 21) | (2 << 16) | (1 << 11) | (0 << 6) | 0x14;
    helper_exec(&mut cpu, word63).unwrap();
    assert_eq!(cpu.gpr(1), 0x8000_0000_0000_0000);
}

#[test]
fn test_dsll32_dsrl32_dsra32() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(2, 1);
    // DSLL32 r1, r2, 0 -> sa=0 -> shift by 32
    let word_dsll32 = (0 << 26) | (0 << 21) | (2 << 16) | (1 << 11) | (0 << 6) | 0x3C;
    helper_exec(&mut cpu, word_dsll32).unwrap();
    assert_eq!(cpu.gpr(1), 0x0000_0001_0000_0000);

    // DSLL32 r3, r2, 31 -> sa=31 -> shift by 63
    let word_dsll63 = (0 << 26) | (0 << 21) | (2 << 16) | (3 << 11) | (31 << 6) | 0x3C;
    helper_exec(&mut cpu, word_dsll63).unwrap();
    assert_eq!(cpu.gpr(3), 0x8000_0000_0000_0000);
}

#[test]
fn test_nop_executes_cleanly() {
    let mut cpu = InterpCpu::new();
    cpu.set_gpr(0, 123);
    // NOP is SLL r0, r0, 0 -> 0x0000_0000
    helper_exec(&mut cpu, 0x0000_0000).unwrap();
    assert_eq!(cpu.gpr(0), 0);
}

#[test]
fn test_unknown_funct_trap() {
    let mut cpu = InterpCpu::new();
    let word = 0x39; // unknown funct
    let res = helper_exec(&mut cpu, word);
    assert!(matches!(res, Err(Trap::UnimplementedOpcode { .. })));
}
