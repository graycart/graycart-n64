//! Unit tests for RSP Scalar Unit (SU) 32-bit MIPS subset.
//!
//! Cited: N64brew — Reality Signal Processor (CPU / Scalar Unit)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/CPU_Core · docs/03-rsp.md §2

use crate::bus::Bus;
use crate::mi::MI_INTR_SP;
use crate::rsp::{InterpRsp, Sp};
use crate::trap::Trap;

fn write_imem_word(bus: &mut Bus, pc: u16, word: u32) {
    let a = (pc as usize) & 0x0FFC;
    let bytes = word.to_be_bytes();
    bus.sp_imem[a] = bytes[0];
    bus.sp_imem[a + 1] = bytes[1];
    bus.sp_imem[a + 2] = bytes[2];
    bus.sp_imem[a + 3] = bytes[3];
}

#[test]
fn test_alu_and_imm() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // ADDIU r1, r0, 0x1234
    write_imem_word(&mut bus, 0, 0x2401_1234);
    // ORI r2, r1, 0x00FF
    write_imem_word(&mut bus, 4, 0x3422_00FF);
    // LUI r3, 0xABCD
    write_imem_word(&mut bus, 8, 0x3C03_ABCD);
    // ADD r4, r1, r3
    write_imem_word(&mut bus, 12, 0x0023_2020);

    bus.sp.pc = 0;
    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(1), 0x1234);

    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(2), 0x12FF);

    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(3), 0xABCD_0000);

    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(4), 0x1234_u32.wrapping_add(0xABCD_0000));

    // Test r0 immutability
    rsp.set_gpr(0, 999);
    assert_eq!(rsp.gpr(0), 0);
}

#[test]
fn test_shifts_and_logic() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // ADDIU r1, r0, 0x3456
    write_imem_word(&mut bus, 0, 0x2401_3456);
    // SLL r2, r1, 4 (op=0, rs=0, rt=1, rd=2, sa=4, funct=0 -> 0x0001_1100)
    write_imem_word(&mut bus, 4, 0x0001_1100);
    // SRL r3, r1, 4 (op=0, rs=0, rt=1, rd=3, sa=4, funct=2 -> 0x0001_1902)
    write_imem_word(&mut bus, 8, 0x0001_1902);
    // NOR r4, r1, r0 (op=0, rs=1, rt=0, rd=4, sa=0, funct=0x27 -> 0x0020_2027)
    write_imem_word(&mut bus, 12, 0x0020_2027);

    bus.sp.pc = 0;
    for _ in 0..4 {
        rsp.step(&mut bus).unwrap();
    }

    assert_eq!(rsp.gpr(1), 0x3456);
    assert_eq!(rsp.gpr(2), 0x0003_4560);
    assert_eq!(rsp.gpr(3), 0x0000_0345);
    assert_eq!(rsp.gpr(4), !0x3456_u32);
}

#[test]
fn test_slt_sltiu() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // ADDIU r1, r0, 5
    write_imem_word(&mut bus, 0, 0x2401_0005);
    // SLTI r2, r1, 10
    write_imem_word(&mut bus, 4, 0x2822_000A);
    // SLTI r3, r1, 2 (should be 0)
    write_imem_word(&mut bus, 8, 0x2823_0002);

    bus.sp.pc = 0;
    rsp.step(&mut bus).unwrap();
    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(2), 1);

    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(3), 0);
}

#[test]
fn test_branches_and_jumps() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // J target (jump target encoding: jump_target = (target >> 2) & 0x03FF_FFFF. For PC=12, target = 12 >> 2 = 3)
    // J opcode = 0x02, target 3 -> 0x0800_0003
    write_imem_word(&mut bus, 0, 0x0800_0003);
    // NOP (delay slot)
    write_imem_word(&mut bus, 4, 0x0000_0000);
    // SKIPPED
    write_imem_word(&mut bus, 8, 0x2401_1111);
    // TARGET: ADDIU r2, r0, 0x2222
    write_imem_word(&mut bus, 12, 0x2402_2222);

    bus.sp.pc = 0;
    rsp.step(&mut bus).unwrap(); // J
    assert_eq!(bus.sp.pc, 4); // delay slot

    rsp.step(&mut bus).unwrap(); // NOP (delay slot executed)
    assert_eq!(bus.sp.pc, 12); // landed at target

    rsp.step(&mut bus).unwrap();
    assert_eq!(rsp.gpr(1), 0); // skipped instruction not executed
    assert_eq!(rsp.gpr(2), 0x2222);
}

#[test]
fn test_pc_wrap_12bit() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // JAL 0x0004 (target = 4 >> 2 = 1) -> 0x0C00_0001 from 0x0FFC
    write_imem_word(&mut bus, 0x0FFC, 0x0C00_0001);
    // NOP (delay slot at 0x0000)
    write_imem_word(&mut bus, 0x0000, 0x0000_0000);

    bus.sp.pc = 0x0FFC;
    rsp.step(&mut bus).unwrap(); // JAL
    assert_eq!(bus.sp.pc, 0x0000); // delay slot PC wrap

    rsp.step(&mut bus).unwrap(); // NOP
    assert_eq!(bus.sp.pc, 0x0004); // jumped to target 0x0004
    assert_eq!(rsp.gpr(31), 0x0004); // link address (0x0FFC + 8 = 0x1004 & 0x0FFC = 0x0004)
}

#[test]
fn test_dmem_wrapping() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // ADDIU r1, r0, 0x0FFE
    write_imem_word(&mut bus, 0, 0x2401_0FFE);
    // LUI r2, 0x1234
    write_imem_word(&mut bus, 4, 0x3C02_1234);
    // ORI r2, r2, 0x5678
    write_imem_word(&mut bus, 8, 0x3442_5678);
    // SW r2, 0(r1) -> spans 0x0FFE..0x0002 (wrapping 4096 DMEM)
    write_imem_word(&mut bus, 12, 0xAC22_0000);
    // LW r3, 0(r1)
    write_imem_word(&mut bus, 16, 0x8C23_0000);

    bus.sp.pc = 0;
    for _ in 0..5 {
        rsp.step(&mut bus).unwrap();
    }

    assert_eq!(rsp.gpr(3), 0x12345678);
}

#[test]
fn test_break() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // BREAK
    write_imem_word(&mut bus, 0, 0x0000_000D);

    bus.sp.pc = 0;
    rsp.step(&mut bus).unwrap();

    assert!(bus.sp.is_halted());
    assert!((bus.sp.status & Sp::STATUS_BROKE) != 0);

    // With INT on break
    bus.sp.status = Sp::STATUS_INTR_ON_BREAK;
    bus.sp.pc = 0;
    rsp.step(&mut bus).unwrap();
    assert!(bus.mi.intr & MI_INTR_SP != 0);
}

#[test]
fn test_cop2_trap() {
    let mut rsp = InterpRsp::new();
    let mut bus = Bus::new(true, false);

    // COP2 instruction
    write_imem_word(&mut bus, 0, 0x4800_0000);

    bus.sp.pc = 0;
    let res = rsp.step(&mut bus);
    assert!(matches!(res, Err(Trap::UnimplementedOpcode { .. })));
}
