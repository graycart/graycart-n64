//! NEC VR4300 shift instructions (SLL, SRL, SRA, SLLV, SRLV, SRAV, DSLL, DSRL, DSRA, DSLLV, DSRLV, DSRAV, DSLL32, DSRL32, DSRA32).
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md; N64brew — VR4300 32-bit Shift Right Arithmetic Bug
//!   https://n64brew.dev/wiki/VR4300

use crate::trap::Trap;

use super::decode::Inst;
use super::InterpCpu;

#[cfg(test)]
#[path = "shift_tests.rs"]
mod tests;

/// Execute `SPECIAL` shift instructions.
pub fn exec_special(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let rs = inst.rs();
    let rt = inst.rt();
    let rd = inst.rd();
    let sa = inst.sa();

    let result = match inst.funct() {
        0x00 => {
            // SLL
            let val = cpu.gpr(rt) as u32;
            (((val << sa) as i32) as i64) as u64
        }
        0x02 => {
            // SRL
            let val = cpu.gpr(rt) as u32;
            (((val >> sa) as i32) as i64) as u64
        }
        0x03 => {
            // SRA — VR4300 hardware shifts the full 64-bit rt before sign-extending bit 31
            // (matches standard 32-bit SRA for sign-extended inputs, and leaks bits 63..32
            // on non-sign-extended 64-bit inputs per N64brew VR4300 hardware errata).
            ((((cpu.gpr(rt) as i64) >> sa) as i32) as i64) as u64
        }
        0x04 => {
            // SLLV
            let sh = (cpu.gpr(rs) as u32) & 0x1F;
            let val = cpu.gpr(rt) as u32;
            (((val << sh) as i32) as i64) as u64
        }
        0x06 => {
            // SRLV
            let sh = (cpu.gpr(rs) as u32) & 0x1F;
            let val = cpu.gpr(rt) as u32;
            (((val >> sh) as i32) as i64) as u64
        }
        0x07 => {
            // SRAV — same VR4300 64-bit right-shift datapath as SRA.
            let sh = (cpu.gpr(rs) as u32) & 0x1F;
            ((((cpu.gpr(rt) as i64) >> sh) as i32) as i64) as u64
        }
        0x14 => {
            // DSLLV
            let sh = (cpu.gpr(rs) as u32) & 0x3F;
            cpu.gpr(rt) << sh
        }
        0x16 => {
            // DSRLV
            let sh = (cpu.gpr(rs) as u32) & 0x3F;
            cpu.gpr(rt) >> sh
        }
        0x17 => {
            // DSRAV
            let sh = (cpu.gpr(rs) as u32) & 0x3F;
            ((cpu.gpr(rt) as i64) >> sh) as u64
        }
        0x38 => {
            // DSLL
            cpu.gpr(rt) << sa
        }
        0x3A => {
            // DSRL
            cpu.gpr(rt) >> sa
        }
        0x3B => {
            // DSRA
            ((cpu.gpr(rt) as i64) >> sa) as u64
        }
        0x3C => {
            // DSLL32
            cpu.gpr(rt) << (sa + 32)
        }
        0x3E => {
            // DSRL32
            cpu.gpr(rt) >> (sa + 32)
        }
        0x3F => {
            // DSRA32
            ((cpu.gpr(rt) as i64) >> (sa + 32)) as u64
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc,
                word: inst.raw(),
            })
        }
    };

    cpu.set_gpr(rd, result);
    Ok(1)
}
