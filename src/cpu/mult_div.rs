//! NEC VR4300 (MIPS III) Multiply, Divide, and HI/LO Move Instructions (`MULT`, `MULTU`, `DIV`, `DIVU`, `DMULT`, `DMULTU`, `DDIV`, `DDIVU`, `MFHI`, `MTHI`, `MFLO`, `MTLO`).
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use super::decode::Inst;
use super::InterpCpu;
use crate::trap::Trap;

#[cfg(test)]
#[path = "mult_div_tests.rs"]
mod tests;

/// Execute SPECIAL opcode family instructions for multiply, divide, and HI/LO accumulator access.
pub fn exec_special(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let rs = inst.rs();
    let rt = inst.rt();
    let rd = inst.rd();

    match inst.funct() {
        0x10 => {
            // MFHI
            cpu.set_gpr(rd, cpu.hi());
            Ok(1)
        }
        0x11 => {
            // MTHI
            cpu.set_hi(cpu.gpr(rs));
            Ok(1)
        }
        0x12 => {
            // MFLO
            cpu.set_gpr(rd, cpu.lo());
            Ok(1)
        }
        0x13 => {
            // MTLO
            cpu.set_lo(cpu.gpr(rs));
            Ok(1)
        }
        0x18 => {
            // MULT
            let prod = i64::from(cpu.gpr(rs) as i32) * i64::from(cpu.gpr(rt) as i32);
            cpu.set_lo((prod as i32 as i64) as u64);
            cpu.set_hi(((prod >> 32) as i32 as i64) as u64);
            Ok(5)
        }
        0x19 => {
            // MULTU
            let prod = u64::from(cpu.gpr(rs) as u32) * u64::from(cpu.gpr(rt) as u32);
            cpu.set_lo((prod as u32 as i32 as i64) as u64);
            cpu.set_hi(((prod >> 32) as u32 as i32 as i64) as u64);
            Ok(5)
        }
        0x1A => {
            // DIV (never panics or traps)
            let n = cpu.gpr(rs) as i32;
            let d = cpu.gpr(rt) as i32;
            let (lo, hi) = if d == 0 {
                let l = if n < 0 { 1u64 } else { u64::MAX };
                let h = (n as i64) as u64;
                (l, h)
            } else if n == i32::MIN && d == -1 {
                ((i32::MIN as i64) as u64, 0)
            } else {
                (((n / d) as i64) as u64, ((n % d) as i64) as u64)
            };
            cpu.set_lo(lo);
            cpu.set_hi(hi);
            Ok(37)
        }
        0x1B => {
            // DIVU (never panics or traps)
            let n = cpu.gpr(rs) as u32;
            let d = cpu.gpr(rt) as u32;
            #[allow(clippy::manual_checked_ops)]
            let (lo, hi) = if d == 0 {
                (u64::MAX, (n as i32 as i64) as u64)
            } else {
                (
                    ((n / d) as i32 as i64) as u64,
                    ((n % d) as i32 as i64) as u64,
                )
            };
            cpu.set_lo(lo);
            cpu.set_hi(hi);
            Ok(37)
        }
        0x1C => {
            // DMULT
            let prod = i128::from(cpu.gpr(rs) as i64) * i128::from(cpu.gpr(rt) as i64);
            cpu.set_lo(prod as u64);
            cpu.set_hi((prod >> 64) as u64);
            Ok(8)
        }
        0x1D => {
            // DMULTU
            let prod = u128::from(cpu.gpr(rs)) * u128::from(cpu.gpr(rt));
            cpu.set_lo(prod as u64);
            cpu.set_hi((prod >> 64) as u64);
            Ok(8)
        }
        0x1E => {
            // DDIV (never panics or traps)
            let n = cpu.gpr(rs) as i64;
            let d = cpu.gpr(rt) as i64;
            let (lo, hi) = if d == 0 {
                let l = if n < 0 { 1u64 } else { u64::MAX };
                let h = n as u64;
                (l, h)
            } else if n == i64::MIN && d == -1 {
                (i64::MIN as u64, 0)
            } else {
                ((n / d) as u64, (n % d) as u64)
            };
            cpu.set_lo(lo);
            cpu.set_hi(hi);
            Ok(69)
        }
        0x1F => {
            // DDIVU (never panics or traps)
            let n = cpu.gpr(rs);
            let d = cpu.gpr(rt);
            #[allow(clippy::manual_checked_ops)]
            let (lo, hi) = if d == 0 {
                (u64::MAX, n)
            } else {
                (n / d, n % d)
            };
            cpu.set_lo(lo);
            cpu.set_hi(hi);
            Ok(69)
        }
        _ => Err(Trap::UnimplementedOpcode {
            pc,
            word: inst.raw(),
        }),
    }
}
