//! Unit tests for VR4300 COP0, ERET, exception delivery, and traps.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapters 5 & 6
//!   via docs/01-cpu-r4300i.md

#![allow(clippy::identity_op)]

use super::*;
use crate::bus::{Bus, CacheAttr};
use crate::cpu::CpuBackend;
use crate::schedule::Scheduler;
use crate::trap::Trap;

#[test]
fn test_cop0_1_move_sign_extension() {
    let mut cpu = InterpCpu::new();
    let mut sched = Scheduler::new();

    // Set EPC to an address with bit 31 set: 0x0000_0000_8000_0004
    cpu.cop0.epc = 0x8000_0004;
    cpu.cop0.entry_hi = 0xFFFF_FFFF_8000_0000;

    // MFC0 (rs=0, rt=1, rd=14 EPC) -> sign-extends lower 32 bits (0x8000_0004 -> 0xFFFF_FFFF_8000_0004)
    let mfc0_epc = Inst((0x10 << 26) | (0 << 21) | (1 << 16) | (14 << 11));
    let res = exec_cop0_move(&mut cpu, &mut sched, 0x1000, mfc0_epc);
    assert!(res.is_ok());
    assert_eq!(cpu.gpr(1), 0xFFFF_FFFF_8000_0004);

    // DMFC0 (rs=1, rt=2, rd=10 EntryHi) -> full 64 bits
    let dmfc0_eh = Inst((0x10 << 26) | (1 << 21) | (2 << 16) | (10 << 11));
    let res = exec_cop0_move(&mut cpu, &mut sched, 0x1000, dmfc0_eh);
    assert!(res.is_ok());
    assert_eq!(cpu.gpr(2), 0xFFFF_FFFF_8000_0000);
}

#[test]
fn test_cop0_2_prid_read_only() {
    let mut cpu = InterpCpu::new();
    let mut sched = Scheduler::new();

    assert_eq!(cpu.cop0.read_reg(15, 0), u64::from(Cop0::PRID_VR4300));

    // Try writing PRId via MTC0 (rd=15)
    cpu.set_gpr(5, 0x1234_5678);
    let mtc0_prid = Inst((0x10 << 26) | (4 << 21) | (5 << 16) | (15 << 11));
    let _ = exec_cop0_move(&mut cpu, &mut sched, 0x1000, mtc0_prid);
    assert_eq!(cpu.cop0.read_reg(15, 0), u64::from(Cop0::PRID_VR4300));
}

#[test]
fn test_cop0_3_wired_and_random() {
    let mut cpu = InterpCpu::new();
    let mut sched = Scheduler::new();

    cpu.cop0.random = 10;
    // Write Wired = 5
    cpu.set_gpr(3, 5);
    let mtc0_wired = Inst((0x10 << 26) | (4 << 21) | (3 << 16) | (6 << 11));
    let _ = exec_cop0_move(&mut cpu, &mut sched, 0x1000, mtc0_wired);

    assert_eq!(cpu.cop0.wired, 5);
    assert_eq!(cpu.cop0.random, 31); // Resets random to 31

    // Tick random down
    cpu.cop0.random = 6;
    cpu.cop0.tick_random();
    assert_eq!(cpu.cop0.random, 5);

    // Next tick wraps to 31 because random <= wired (5 <= 5)
    cpu.cop0.tick_random();
    assert_eq!(cpu.cop0.random, 31);
}

#[test]
fn test_cop0_4_count_compare_timer() {
    let mut cpu = InterpCpu::new();
    let mut sched = Scheduler::new();

    // Write Compare = 100
    cpu.set_gpr(4, 100);
    let mtc0_cmp = Inst((0x10 << 26) | (4 << 21) | (4 << 16) | (11 << 11));
    let _ = exec_cop0_move(&mut cpu, &mut sched, 0x1000, mtc0_cmp);

    // IP7 should be cleared by write_compare
    assert_eq!(cpu.cop0.cause & (1 << 15), 0);

    // Trigger compare event and enable IE + IM7 (with EXL=0, ERL=0)
    cpu.cop0.status = (1 << 15) | 1;
    cpu.cop0.count_clock.on_compare_event(&mut sched);
    cpu.cop0.sync_timer_ip7();
    assert_ne!(cpu.cop0.cause & (1 << 15), 0);
    assert!(has_pending_interrupt(&cpu.cop0));
}

#[test]
fn test_cop0_5_cause_software_interrupts_only() {
    let mut cpu = InterpCpu::new();
    let mut sched = Scheduler::new();

    cpu.cop0.cause = 0x8000_8000; // HW IP7 set, BD set
                                  // Write Cause with bits 9..8 = 3, and try to overwrite BD (bit 31) and IP7 (bit 15)
    cpu.set_gpr(6, 0xFFFF_FFFF);
    let mtc0_cause = Inst((0x10 << 26) | (4 << 21) | (6 << 16) | (13 << 11));
    let _ = exec_cop0_move(&mut cpu, &mut sched, 0x1000, mtc0_cause);

    // Only IP1..0 (bits 9..8) should be modified; BD and IP7 preserved
    assert_eq!(cpu.cop0.cause & (1 << 31), 1 << 31);
    assert_eq!(cpu.cop0.cause & (1 << 15), 1 << 15);
    assert_eq!((cpu.cop0.cause >> 8) & 3, 3);
}

#[test]
fn test_cop0_6_deliver_exception_variants() {
    let mut cpu = InterpCpu::new();

    // 1. Normal exception, BEV=0 -> vector 0x8000_0180
    cpu.cop0.status |= 1 << 22; // BEV = 1
    deliver_exception(&mut cpu, 0x2000, CpuException::Syscall);
    assert_eq!(cpu.pc(), 0xFFFF_FFFF_BFC0_0380); // BEV=1 -> 0xBFC0_0200 + 0x180
    assert_eq!(cpu.cop0.epc, 0x2000);
    assert_eq!(cpu.cop0.cause & (1 << 31), 0); // BD = 0
    assert_eq!((cpu.cop0.cause >> 2) & 0x1F, 8); // Syscall exc code = 8
    assert_ne!(cpu.cop0.status & (1 << 1), 0); // EXL = 1

    // 2. Delay slot exception (simulate delay slot)
    cpu.reset();
    cpu.cop0.status &= !(1 << 22); // BEV = 0
                                   // Force in_delay_slot by setting pc or simulating branch
                                   // deliver_exception checks cpu.in_delay_slot()
                                   // Let's test TLB Refill vector (+0x000) when EXL=0
    deliver_exception(
        &mut cpu,
        0x3004,
        CpuException::TlbLoad {
            bad_vaddr: 0x1234_5678,
            refill: true,
        },
    );
    assert_eq!(cpu.pc(), 0xFFFF_FFFF_8000_0000); // 0x8000_0000 + 0x000 (Refill vector)
    assert_eq!(cpu.cop0.bad_vaddr, 0x1234_5678);
    assert_eq!(cpu.cop0.epc, 0x3004);
}

#[test]
fn test_cop0_7_eret_execution() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);
    let mut sched = Scheduler::new();
    let start = 0xFFFF_FFFF_8000_1000u64;
    let target = 0xFFFF_FFFF_8000_2000u64;

    // Put ERET at 0x1000 and an ADDIU in the next slot (MUST NOT execute — ERET has no delay slot!)
    bus.write_u32(0x1000, 0x4200_0018, CacheAttr::Cached)
        .unwrap();
    bus.write_u32(0x1004, (0x09 << 26) | (3 << 16) | 99, CacheAttr::Cached)
        .unwrap();

    cpu.set_pc(start);
    cpu.cop0.epc = target;
    cpu.cop0.status = 1 << 1; // EXL = 1, ERL = 0
    cpu.set_ll_bit(true);

    cpu.step(&mut bus, &mut sched).unwrap();
    assert!(!cpu.cop0.exl());
    assert!(!cpu.ll_bit());
    assert_eq!(cpu.pc(), target);
    assert_eq!(cpu.next_pc(), target + 4);
    assert!(!cpu.in_delay_slot());
    assert_eq!(cpu.gpr(3), 0);
}

#[test]
fn test_cop0_8_traps() {
    let mut cpu = InterpCpu::new();

    cpu.set_gpr(1, 10);
    cpu.set_gpr(2, 20);

    // TEQ r1, r2 (10 == 20 is false) -> passes
    let teq = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (0 << 11) | (0 << 6) | 0x34);
    assert!(exec_special_trap(&mut cpu, 0x1000, teq).is_ok());

    // TNE r1, r2 (10 != 20 is true) -> traps
    let tne = Inst((0x00 << 26) | (1 << 21) | (2 << 16) | (0 << 11) | (0 << 6) | 0x36);
    let res = exec_special_trap(&mut cpu, 0x1000, tne);
    match res {
        Err(Trap::CpuException { code, .. }) => assert_eq!(code, "Tr"),
        _ => panic!("Expected Trap exception"),
    }

    // TEQI r1, 10 (10 == 10 is true) -> traps
    let teqi = Inst((0x01 << 26) | (1 << 21) | (0x0C << 16) | 10);
    let res = exec_regimm_trap(&mut cpu, 0x1000, teqi);
    match res {
        Err(Trap::CpuException { code, .. }) => assert_eq!(code, "Tr"),
        _ => panic!("Expected Trap exception"),
    }
}
