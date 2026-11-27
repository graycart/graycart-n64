//! MIPS III / VR4300 instruction bitfield wrapper and complete primary decoder dispatch.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapters 16 & 17
//!   via docs/01-cpu-r4300i.md

use crate::bus::Bus;
use crate::schedule::Scheduler;
use crate::trap::Trap;

use super::{alu, branch, cache, cop0, cop1, load_store, mult_div, shift, tlb, InterpCpu};

/// Decoded MIPS III 32-bit instruction word with zero-cost bitfield extractors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inst(pub u32);

impl Inst {
    /// Raw 32-bit instruction word.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Primary 6-bit opcode (`bits 31..=26`).
    #[must_use]
    pub const fn opcode(self) -> u8 {
        ((self.0 >> 26) & 0x3F) as u8
    }

    /// Source register index `rs` (`bits 25..=21`) / COP format `fmt`.
    #[must_use]
    pub const fn rs(self) -> usize {
        ((self.0 >> 21) & 0x1F) as usize
    }

    /// Target register index `rt` (`bits 20..=16`) / COP1 `ft`.
    #[must_use]
    pub const fn rt(self) -> usize {
        ((self.0 >> 16) & 0x1F) as usize
    }

    /// Destination register index `rd` (`bits 15..=11`) / COP1 `fs`.
    #[must_use]
    pub const fn rd(self) -> usize {
        ((self.0 >> 11) & 0x1F) as usize
    }

    /// 5-bit shift amount `sa` (`bits 10..=6`) / COP1 `fd`.
    #[must_use]
    pub const fn sa(self) -> u32 {
        (self.0 >> 6) & 0x1F
    }

    /// COP1 destination register `fd` (`bits 10..=6`).
    #[must_use]
    pub const fn fd(self) -> usize {
        ((self.0 >> 6) & 0x1F) as usize
    }

    /// Secondary 6-bit function code `funct` (`bits 5..=0`).
    #[must_use]
    pub const fn funct(self) -> u8 {
        (self.0 & 0x3F) as u8
    }

    /// Unsigned 16-bit immediate (`bits 15..=0`).
    #[must_use]
    pub const fn imm16(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    /// Sign-extended 16-bit immediate as `i64`.
    #[must_use]
    pub const fn simm16(self) -> i64 {
        (self.0 as i16) as i64
    }

    /// 26-bit jump target (`bits 25..=0`).
    #[must_use]
    pub const fn target26(self) -> u32 {
        self.0 & 0x03FF_FFFF
    }
}

/// Decode and execute one VR4300 instruction word, returning approximate CPU cycles elapsed.
///
/// Unimplemented encodings MUST return `Err(Trap::UnimplementedOpcode { pc, word })`.
pub fn execute_instruction(
    cpu: &mut InterpCpu,
    bus: &mut Bus,
    sched: &mut Scheduler,
    pc: u64,
    word: u32,
) -> Result<u64, Trap> {
    let inst = Inst(word);
    match inst.opcode() {
        // SPECIAL (0x00): secondary dispatch on `funct` (bits 5..=0).
        0x00 => match inst.funct() {
            // Shifts
            0x00 | 0x02 | 0x03 | 0x04 | 0x06 | 0x07 | 0x14 | 0x16 | 0x17 | 0x38 | 0x3A | 0x3B
            | 0x3C | 0x3E | 0x3F => shift::exec_special(cpu, pc, inst),
            // Register jumps
            0x08 | 0x09 => branch::exec_special(cpu, pc, inst),
            // Syscall / Break / Register Traps
            0x0C | 0x0D | 0x30..=0x34 | 0x36 => cop0::exec_special_trap(cpu, pc, inst),
            // Memory barrier
            0x0F => load_store::exec_sync(cpu, pc, inst),
            // Multiply / divide & HI/LO moves
            0x10..=0x13 | 0x18..=0x1F => mult_div::exec_special(cpu, pc, inst),
            // Integer ALU
            0x20..=0x27 | 0x2A | 0x2B | 0x2C..=0x2F => alu::exec_special(cpu, pc, inst),
            _ => Err(Trap::UnimplementedOpcode { pc, word }),
        },
        // REGIMM (0x01): branches & immediate traps (`rt` selects op).
        0x01 => match inst.rt() {
            0x00 | 0x01 | 0x02 | 0x03 | 0x10 | 0x11 | 0x12 | 0x13 => {
                branch::exec_regimm(cpu, pc, inst)
            }
            0x08..=0x0C | 0x0E => cop0::exec_regimm_trap(cpu, pc, inst),
            _ => Err(Trap::UnimplementedOpcode { pc, word }),
        },
        // Primary jumps & branches (including Branch Likely).
        0x02..=0x07 | 0x14..=0x17 => branch::exec_primary(cpu, pc, inst),
        // Immediate ALU (32-bit & 64-bit).
        0x08..=0x0F | 0x18 | 0x19 => alu::exec_imm(cpu, pc, inst),
        // COP0 (0x10): moves, TLB ops, and ERET.
        0x10 => {
            if (inst.rs() & 0x10) != 0 {
                match inst.funct() {
                    0x01 | 0x02 | 0x06 | 0x08 => tlb::exec_tlb_op(cpu, pc, inst),
                    0x18 => cop0::exec_eret(cpu, pc, inst),
                    _ => Err(Trap::UnimplementedOpcode { pc, word }),
                }
            } else {
                cop0::exec_cop0_move(cpu, sched, pc, inst)
            }
        }
        // COP1 (0x11): FPU moves, branches, arithmetic, convert, compare.
        0x11 => cop1::exec_cop1(cpu, pc, inst),
        // CACHE (0x2F): L1 I-cache / D-cache maintenance.
        0x2F => cache::exec_cache(cpu, bus, pc, inst),
        // COP1 loads & stores (LWC1, LDC1, SWC1, SDC1).
        0x31 | 0x35 | 0x39 | 0x3D => cop1::exec_load_store(cpu, bus, pc, inst),
        // Integer loads & stores (including unaligned & LL/SC/LLD/SCD).
        0x20..=0x2B
        | 0x1A
        | 0x1B
        | 0x2C
        | 0x2D
        | 0x2E
        | 0x30
        | 0x34
        | 0x37
        | 0x38
        | 0x3C
        | 0x3F => load_store::exec_primary(cpu, bus, pc, inst),
        _ => Err(Trap::UnimplementedOpcode { pc, word }),
    }
}
