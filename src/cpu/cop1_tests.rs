//! VR4300 COP1 IEEE-754 Floating-Point Unit Unit Tests.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapters 7–9 & 17
//!   via docs/01-cpu-r4300i.md

use super::*;
use crate::bus::{Bus, CacheAttr};
use crate::cpu::decode::Inst;
use crate::cpu::{CpuBackend, InterpCpu};
use crate::schedule::Scheduler;

fn enc_fp(fmt: u32, ft: u32, fs: u32, fd: u32, funct: u32) -> Inst {
    Inst((0x11 << 26) | (fmt << 21) | (ft << 16) | (fs << 11) | (fd << 6) | funct)
}

fn enc_ls(op: u32, base: u32, ft: u32, imm: u16) -> Inst {
    Inst((op << 26) | (base << 21) | (ft << 16) | u32::from(imm))
}

#[test]
fn test_cu1_unusable_trap() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);
    cpu.cop0.status &= !(1 << 29); // CU1 = 0

    let res = exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, enc_fp(0x00, 1, 0, 0, 0));
    assert!(matches!(res, Err(Trap::CpuException { code: "CpU", .. })));

    let res = exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, enc_fp(0x10, 2, 1, 3, 0));
    assert!(matches!(res, Err(Trap::CpuException { code: "CpU", .. })));

    let res = exec_load_store(
        &mut cpu,
        &mut bus,
        0xFFFF_FFFF_8000_0000,
        enc_ls(0x31, 1, 2, 0),
    );
    assert!(matches!(res, Err(Trap::CpuException { code: "CpU", .. })));
}

#[test]
fn test_fr0_fr1_pairing() {
    let mut cpu = InterpCpu::new();
    cpu.cop0.status |= 1 << 29;
    cpu.cop0.status &= !(1 << 26); // FR = 0

    // In FR=0 mode, write 64-bit value to f2 via DMTC1 (f2 low, f3 high)
    cpu.set_gpr(1, 0x1234_5678_9ABC_DEF0);
    exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, enc_fp(0x05, 1, 2, 0, 0)).unwrap();

    assert_eq!(cpu.cop1.fpr[2], 0x9ABC_DEF0);
    assert_eq!(cpu.cop1.fpr[3], 0x1234_5678);
    assert_eq!(cpu.cop1.read_u64(2, false), 0x1234_5678_9ABC_DEF0);

    // Switch to FR = 1: f2 and f3 are independent 64-bit registers
    cpu.cop0.status |= 1 << 26;
    cpu.set_gpr(2, 0xFEDC_BA98_7654_3210);
    exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, enc_fp(0x05, 2, 3, 0, 0)).unwrap();
    assert_eq!(cpu.cop1.fpr[3], 0xFEDC_BA98_7654_3210);
    assert_eq!(cpu.cop1.fpr[2], 0x9ABC_DEF0);
}

#[test]
fn test_load_store_alignment_and_roundtrip() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);
    cpu.cop0.status |= (1 << 29) | (1 << 26); // CU1=1, FR=1

    // Unaligned LWC1 raises AdEL
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0001);
    let res = exec_load_store(
        &mut cpu,
        &mut bus,
        0xFFFF_FFFF_8000_0000,
        enc_ls(0x31, 1, 1, 0),
    );
    assert!(matches!(res, Err(Trap::CpuException { code: "AdEL", .. })));

    // Aligned SWC1 / LWC1 and SDC1 / LDC1 roundtrip in RDRAM
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0000);
    cpu.cop1.write_u32(1, 0xDEAD_BEEF, true);
    exec_load_store(
        &mut cpu,
        &mut bus,
        0xFFFF_FFFF_8000_0000,
        enc_ls(0x39, 1, 1, 0),
    )
    .unwrap();
    exec_load_store(
        &mut cpu,
        &mut bus,
        0xFFFF_FFFF_8000_0004,
        enc_ls(0x31, 1, 2, 0),
    )
    .unwrap();
    assert_eq!(cpu.cop1.read_u32(2, true), 0xDEAD_BEEF);

    cpu.cop1.write_u64(4, 0x0123_4567_89AB_CDEF, true);
    exec_load_store(
        &mut cpu,
        &mut bus,
        0xFFFF_FFFF_8000_0008,
        enc_ls(0x3D, 1, 4, 8),
    )
    .unwrap();
    exec_load_store(
        &mut cpu,
        &mut bus,
        0xFFFF_FFFF_8000_000C,
        enc_ls(0x35, 1, 5, 8),
    )
    .unwrap();
    assert_eq!(cpu.cop1.read_u64(5, true), 0x0123_4567_89AB_CDEF);
}

#[test]
fn test_cfc1_ctc1() {
    let mut cpu = InterpCpu::new();
    cpu.cop0.status |= 1 << 29;

    // CFC1 r2, fcr0 -> 0x0000_0B00
    exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, enc_fp(0x02, 2, 0, 0, 0)).unwrap();
    assert_eq!(cpu.gpr(2), u64::from(Cop1::FCR0_VR4300));

    // CTC1 FCR31 masking
    cpu.set_gpr(1, 0xFFFF_FFFF);
    exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, enc_fp(0x06, 1, 31, 0, 0)).unwrap();
    assert_eq!(cpu.cop1.fcr31, 0x0183_FFFF);
}

#[test]
fn test_float_arithmetic_single_and_double() {
    let mut cpu = InterpCpu::new();
    cpu.cop0.status |= (1 << 29) | (1 << 26);

    cpu.cop1.write_u32(1, 4.0f32.to_bits(), true);
    cpu.cop1.write_u32(2, 2.0f32.to_bits(), true);

    exec_cop1(&mut cpu, 0, enc_fp(0x10, 2, 1, 3, 0x00)).unwrap(); // ADD.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(3, true)), 6.0);

    exec_cop1(&mut cpu, 0, enc_fp(0x10, 2, 1, 3, 0x01)).unwrap(); // SUB.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(3, true)), 2.0);

    exec_cop1(&mut cpu, 0, enc_fp(0x10, 2, 1, 3, 0x02)).unwrap(); // MUL.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(3, true)), 8.0);

    exec_cop1(&mut cpu, 0, enc_fp(0x10, 2, 1, 3, 0x03)).unwrap(); // DIV.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(3, true)), 2.0);

    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 1, 3, 0x04)).unwrap(); // SQRT.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(3, true)), 2.0);

    cpu.cop1.write_u32(1, (-4.0f32).to_bits(), true);
    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 1, 3, 0x05)).unwrap(); // ABS.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(3, true)), 4.0);

    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 3, 4, 0x07)).unwrap(); // NEG.S
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(4, true)), -4.0);

    // Double precision ADD.D / MUL.D
    cpu.cop1.write_u64(6, 1.5f64.to_bits(), true);
    cpu.cop1.write_u64(7, 2.25f64.to_bits(), true);
    exec_cop1(&mut cpu, 0, enc_fp(0x11, 7, 6, 8, 0x00)).unwrap();
    assert_eq!(f64::from_bits(cpu.cop1.read_u64(8, true)), 3.75);
}

#[test]
fn test_rounding_and_cvt() {
    let mut cpu = InterpCpu::new();
    cpu.cop0.status |= (1 << 29) | (1 << 26);

    // ROUND.W.S always ties-to-even regardless of FCR31.RM
    cpu.cop1.fcr31 = 1; // RZ
    cpu.cop1.write_u32(1, 2.5f32.to_bits(), true);
    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 1, 3, 0x0C)).unwrap(); // ROUND.W.S f3, f1
    assert_eq!(cpu.cop1.read_u32(3, true) as i32, 2);

    cpu.cop1.write_u32(1, 3.5f32.to_bits(), true);
    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 1, 3, 0x0C)).unwrap();
    assert_eq!(cpu.cop1.read_u32(3, true) as i32, 4);

    // CVT.W.S honors FCR31.RM (RP = 2 -> ceil, RM = 3 -> floor)
    cpu.cop1.write_u32(1, (-2.2f32).to_bits(), true);
    cpu.cop1.fcr31 = 2; // RP (ceil) -> -2
    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 1, 3, 0x24)).unwrap();
    assert_eq!(cpu.cop1.read_u32(3, true) as i32, -2);

    cpu.cop1.fcr31 = 3; // RM (floor) -> -3
    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 1, 3, 0x24)).unwrap();
    assert_eq!(cpu.cop1.read_u32(3, true) as i32, -3);

    // CVT.S.W and CVT.D.S
    cpu.cop1.write_u32(5, (-42i32) as u32, true);
    exec_cop1(&mut cpu, 0, enc_fp(0x14, 0, 5, 6, 0x20)).unwrap(); // CVT.S.W
    assert_eq!(f32::from_bits(cpu.cop1.read_u32(6, true)), -42.0);
    exec_cop1(&mut cpu, 0, enc_fp(0x10, 0, 6, 7, 0x21)).unwrap(); // CVT.D.S
    assert_eq!(f64::from_bits(cpu.cop1.read_u64(7, true)), -42.0);
}

#[test]
fn test_compare_and_bc1_branches() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);
    let mut sched = Scheduler::new();
    let start = 0xFFFF_FFFF_8000_1000u64;
    cpu.set_pc(start);
    cpu.cop0.status |= (1 << 29) | (1 << 26);

    cpu.cop1.write_u32(1, 2.0f32.to_bits(), true);
    cpu.cop1.write_u32(2, 3.0f32.to_bits(), true);

    // C.LT.S f1, f2 (fmt=0x10, ft=2, fs=1, funct=0x3C) -> C = 1
    exec_cop1(&mut cpu, start, enc_fp(0x10, 2, 1, 0, 0x3C)).unwrap();
    assert_ne!(cpu.cop1.fcr31 & (1 << 23), 0);

    // BC1T +2 at 0x1000 -> taken, target = 0x1004 + 8 = 0x100C
    bus.write_u32(
        0x1000,
        (0x11 << 26) | (0x08 << 21) | (1 << 16) | 2,
        CacheAttr::Cached,
    )
    .unwrap();
    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.pc(), start + 4);
    assert_eq!(cpu.next_pc(), start + 12);
    assert!(cpu.in_delay_slot());

    // BC1FL +2 when C=1 -> not taken, delay slot nullified (pc advances to +8)
    cpu.set_pc(start);
    bus.write_u32(
        0x1000,
        (0x11 << 26) | (0x08 << 21) | (2 << 16) | 2,
        CacheAttr::Cached,
    )
    .unwrap();
    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.pc(), start + 8);
    assert!(!cpu.in_delay_slot());
}

#[test]
fn test_unimplemented_opcode() {
    let mut cpu = InterpCpu::new();
    cpu.cop0.status |= 1 << 29;

    let bad_inst = Inst(0x46FF_003F); // unknown funct
    let res = exec_cop1(&mut cpu, 0xFFFF_FFFF_8000_0000, bad_inst);
    assert!(matches!(res, Err(Trap::UnimplementedOpcode { .. })));
}
