//! Unit tests for NEC VR4300 Jumps, Branches, and Branch Likely instructions.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use super::*;
use crate::bus::{Bus, CacheAttr};
use crate::cpu::CpuBackend;
use crate::schedule::Scheduler;
use crate::trap::Trap;

fn make_bus_with_words(base_pa: u32, words: &[u32]) -> Bus {
    let mut bus = Bus::new(false, false);
    for (i, &w) in words.iter().enumerate() {
        bus.write_u32(base_pa + (i as u32) * 4, w, CacheAttr::Cached)
            .unwrap();
    }
    bus
}

#[test]
fn test_beq_taken_executes_delay_slot_then_jumps() {
    let start = 0xFFFF_FFFF_8000_1000u64;
    // 0x1000: BEQ r1, r2, +2 -> target = 0x1004 + 8 = 0x100C
    // 0x1004: ADDIU r3, r0, 7 (delay slot!)
    // 0x1008: ADDIU r4, r0, 99 (skipped)
    // 0x100C: ADDIU r5, r0, 42 (target)
    let mut bus = make_bus_with_words(
        0x1000,
        &[
            (0x04 << 26) | (1 << 21) | (2 << 16) | 2,
            (0x09 << 26) | (3 << 16) | 7,
            (0x09 << 26) | (4 << 16) | 99,
            (0x09 << 26) | (5 << 16) | 42,
        ],
    );
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(1, 123);
    cpu.set_gpr(2, 123);

    cpu.step(&mut bus, &mut sched).unwrap(); // BEQ
    assert_eq!(cpu.pc(), start + 4);
    assert_eq!(cpu.next_pc(), start + 12);
    assert!(cpu.in_delay_slot());

    cpu.step(&mut bus, &mut sched).unwrap(); // delay slot executes!
    assert_eq!(cpu.gpr(3), 7);
    assert_eq!(cpu.pc(), start + 12);
    assert!(!cpu.in_delay_slot());

    cpu.step(&mut bus, &mut sched).unwrap(); // target executes!
    assert_eq!(cpu.gpr(4), 0);
    assert_eq!(cpu.gpr(5), 42);
}

#[test]
fn test_beq_not_taken_enters_delay_slot() {
    let start = 0xFFFF_FFFF_8000_1000u64;
    let mut bus = make_bus_with_words(
        0x1000,
        &[
            (0x04 << 26) | (1 << 21) | (2 << 16) | 4,
            (0x09 << 26) | (3 << 16) | 11,
        ],
    );
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(1, 1);
    cpu.set_gpr(2, 2);

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.pc(), start + 4);
    assert_eq!(cpu.next_pc(), start + 8);
    assert!(cpu.in_delay_slot());

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.gpr(3), 11);
    assert_eq!(cpu.pc(), start + 8);
    assert!(!cpu.in_delay_slot());
}

#[test]
fn test_branch_likely_not_taken_nullifies_delay_slot() {
    let start = 0xFFFF_FFFF_8000_1000u64;
    // 0x1000: BEQL r1, r2, +4 (not taken -> nullify 0x1004, jump straight to 0x1008)
    // 0x1004: ADDIU r3, r0, 99 (MUST NOT execute!)
    // 0x1008: ADDIU r4, r0, 55
    let mut bus = make_bus_with_words(
        0x1000,
        &[
            (0x14 << 26) | (1 << 21) | (2 << 16) | 4,
            (0x09 << 26) | (3 << 16) | 99,
            (0x09 << 26) | (4 << 16) | 55,
        ],
    );
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(1, 10);
    cpu.set_gpr(2, 20);

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.pc(), start + 8);
    assert!(!cpu.in_delay_slot());
    assert_eq!(cpu.gpr(3), 0);

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.gpr(4), 55);
}

#[test]
fn test_beql_taken_executes_delay_slot() {
    let start = 0xFFFF_FFFF_8000_1000u64;
    let mut bus = make_bus_with_words(
        0x1000,
        &[
            (0x14 << 26) | (1 << 21) | (2 << 16) | 2,
            (0x09 << 26) | (3 << 16) | 77,
        ],
    );
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(1, 10);
    cpu.set_gpr(2, 10);

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.pc(), start + 4);
    assert_eq!(cpu.next_pc(), start + 12);
    assert!(cpu.in_delay_slot());

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.gpr(3), 77);
    assert_eq!(cpu.pc(), start + 12);
}

#[test]
fn test_negative_branch_offset() {
    let start = 0xFFFF_FFFF_8000_1010u64;
    let mut bus = make_bus_with_words(0x1010, &[(0x04 << 26) | (1 << 21) | (2 << 16) | 0xFFFE]);
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(1, 5);
    cpu.set_gpr(2, 5);

    cpu.step(&mut bus, &mut sched).unwrap();
    // delay_slot_pc = 0x1014, simm16 = -2 -> target = 0x1014 - 8 = 0x100C = start - 4
    assert_eq!(cpu.next_pc(), start - 4);
}

#[test]
fn test_j_and_jal_use_delay_slot_upper_bits() {
    let mut cpu = InterpCpu::new();
    let start = 0xFFFF_FFFF_8FFF_FFFCu64;
    cpu.set_pc(start);
    // delay_slot_pc = 0xFFFF_FFFF_9000_0000 -> upper bits 0xFFFF_FFFF_9000_0000
    // JAL target26 = 0x40 -> target = 0xFFFF_FFFF_9000_0100, r31 = delay_slot_pc + 4 = start + 8
    let word = (0x03 << 26) | 0x40;
    exec_primary(&mut cpu, start, Inst(word)).unwrap();
    assert_eq!(cpu.gpr(31), start.wrapping_add(8));
}

#[test]
fn test_jalr_same_reg_reads_rs_before_writing_rd() {
    let start = 0xFFFF_FFFF_8000_1000u64;
    let target = 0xFFFF_FFFF_8000_2000u64;
    // JALR rd=5, rs=5 -> opcode 0, rs=5, rd=5, funct=0x09
    let mut bus = make_bus_with_words(0x1000, &[(5 << 21) | (5 << 11) | 0x09, 0]);
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(5, target);

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.gpr(5), start + 8);
    assert_eq!(cpu.next_pc(), target);
}

#[test]
fn test_regimm_al_links_even_when_not_taken_and_reads_old_r31() {
    let start = 0xFFFF_FFFF_8000_1000u64;
    // BLTZAL r31, +2 (rs=31, rt=0x10): old r31 = -1 (negative -> taken!)
    let mut bus = make_bus_with_words(
        0x1000,
        &[
            (0x01 << 26) | (31 << 21) | (0x10 << 16) | 2,
            0, // delay slot
        ],
    );
    let mut sched = Scheduler::new();
    let mut cpu = InterpCpu::new();
    cpu.set_pc(start);
    cpu.set_gpr(31, u64::MAX); // -1 -> BLTZAL must be TAKEN using OLD r31!

    cpu.step(&mut bus, &mut sched).unwrap();
    assert_eq!(cpu.gpr(31), start + 8);
    assert_eq!(cpu.next_pc(), start + 12);

    // BLTZALL r5, +2 (rs=5, rt=0x12) with r5 = +10 -> NOT taken, r31 STILL written, delay slot nullified!
    let mut bus2 = make_bus_with_words(0x1000, &[(0x01 << 26) | (5 << 21) | (0x12 << 16) | 2]);
    let mut cpu2 = InterpCpu::new();
    cpu2.set_pc(start);
    cpu2.set_gpr(5, 10);
    cpu2.step(&mut bus2, &mut sched).unwrap();
    assert_eq!(cpu2.gpr(31), start + 8);
    assert_eq!(cpu2.pc(), start + 8);
    assert!(!cpu2.in_delay_slot());
}

#[test]
fn test_unknown_opcode_and_all_remaining_variants() {
    let mut cpu = InterpCpu::new();
    let start = 0xFFFF_FFFF_8000_1000u64;
    cpu.set_pc(start);
    cpu.set_gpr(1, 0);
    cpu.set_gpr(2, 1);

    exec_primary(
        &mut cpu,
        start,
        Inst((0x05 << 26) | (1 << 21) | (2 << 16) | 1),
    )
    .unwrap(); // BNE
    exec_primary(&mut cpu, start, Inst((0x06 << 26) | (1 << 21) | 1)).unwrap(); // BLEZ
    exec_primary(&mut cpu, start, Inst((0x07 << 26) | (2 << 21) | 1)).unwrap(); // BGTZ
    exec_primary(
        &mut cpu,
        start,
        Inst((0x15 << 26) | (1 << 21) | (2 << 16) | 1),
    )
    .unwrap(); // BNEL
    exec_primary(&mut cpu, start, Inst((0x16 << 26) | (1 << 21) | 1)).unwrap(); // BLEZL
    exec_primary(&mut cpu, start, Inst((0x17 << 26) | (2 << 21) | 1)).unwrap(); // BGTZL
    exec_regimm(&mut cpu, start, Inst((0x01 << 26) | (1 << 21) | 1)).unwrap(); // BLTZ
    exec_regimm(
        &mut cpu,
        start,
        Inst((0x01 << 26) | (2 << 21) | (1 << 16) | 1),
    )
    .unwrap(); // BGEZ
    exec_regimm(
        &mut cpu,
        start,
        Inst((0x01 << 26) | (1 << 21) | (2 << 16) | 1),
    )
    .unwrap(); // BLTZL
    exec_regimm(
        &mut cpu,
        start,
        Inst((0x01 << 26) | (2 << 21) | (3 << 16) | 1),
    )
    .unwrap(); // BGEZL
    exec_regimm(
        &mut cpu,
        start,
        Inst((0x01 << 26) | (2 << 21) | (0x11 << 16) | 1),
    )
    .unwrap(); // BGEZAL
    exec_regimm(
        &mut cpu,
        start,
        Inst((0x01 << 26) | (2 << 21) | (0x13 << 16) | 1),
    )
    .unwrap(); // BGEZALL
    exec_special(&mut cpu, start, Inst((2 << 21) | 0x08)).unwrap(); // JR

    assert!(matches!(
        exec_primary(&mut cpu, start, Inst(0xFFFF_FFFF)),
        Err(Trap::UnimplementedOpcode { .. })
    ));
    assert!(matches!(
        exec_special(&mut cpu, start, Inst(0x0000_003F)),
        Err(Trap::UnimplementedOpcode { .. })
    ));
    assert!(matches!(
        exec_regimm(&mut cpu, start, Inst((0x01 << 26) | (0x1F << 16))),
        Err(Trap::UnimplementedOpcode { .. })
    ));
}
