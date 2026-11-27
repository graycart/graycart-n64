//! VR4300 integer ALU instruction family.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use crate::trap::Trap;

use super::decode::Inst;
use super::{CpuException, InterpCpu};

/// Execute `SPECIAL` integer ALU instructions (`ADD`, `ADDU`, `SUB`, `SUBU`, `DADD`, `DADDU`,
/// `DSUB`, `DSUBU`, `SLT`, `SLTU`, `AND`, `OR`, `XOR`, `NOR`).
pub fn exec_special(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let rs = inst.rs();
    let rt = inst.rt();
    let rd = inst.rd();

    match inst.funct() {
        0x20 => {
            // ADD
            let v_rs = cpu.gpr(rs) as i32;
            let v_rt = cpu.gpr(rt) as i32;
            match v_rs.checked_add(v_rt) {
                Some(v) => cpu.set_gpr(rd, v as i64 as u64),
                None => return Err(cpu.raise_exception(pc, CpuException::ArithmeticOverflow)),
            }
        }
        0x21 => {
            // ADDU
            let v_rs = cpu.gpr(rs) as u32;
            let v_rt = cpu.gpr(rt) as u32;
            let res = v_rs.wrapping_add(v_rt) as i32 as i64 as u64;
            cpu.set_gpr(rd, res);
        }
        0x22 => {
            // SUB
            let v_rs = cpu.gpr(rs) as i32;
            let v_rt = cpu.gpr(rt) as i32;
            match v_rs.checked_sub(v_rt) {
                Some(v) => cpu.set_gpr(rd, v as i64 as u64),
                None => return Err(cpu.raise_exception(pc, CpuException::ArithmeticOverflow)),
            }
        }
        0x23 => {
            // SUBU
            let v_rs = cpu.gpr(rs) as u32;
            let v_rt = cpu.gpr(rt) as u32;
            let res = v_rs.wrapping_sub(v_rt) as i32 as i64 as u64;
            cpu.set_gpr(rd, res);
        }
        0x24 => {
            // AND
            cpu.set_gpr(rd, cpu.gpr(rs) & cpu.gpr(rt));
        }
        0x25 => {
            // OR
            cpu.set_gpr(rd, cpu.gpr(rs) | cpu.gpr(rt));
        }
        0x26 => {
            // XOR
            cpu.set_gpr(rd, cpu.gpr(rs) ^ cpu.gpr(rt));
        }
        0x27 => {
            // NOR
            cpu.set_gpr(rd, !(cpu.gpr(rs) | cpu.gpr(rt)));
        }
        0x2A => {
            // SLT
            let res = if (cpu.gpr(rs) as i64) < (cpu.gpr(rt) as i64) {
                1
            } else {
                0
            };
            cpu.set_gpr(rd, res);
        }
        0x2B => {
            // SLTU
            let res = if cpu.gpr(rs) < cpu.gpr(rt) { 1 } else { 0 };
            cpu.set_gpr(rd, res);
        }
        0x2C => {
            // DADD
            let v_rs = cpu.gpr(rs) as i64;
            let v_rt = cpu.gpr(rt) as i64;
            match v_rs.checked_add(v_rt) {
                Some(v) => cpu.set_gpr(rd, v as u64),
                None => return Err(cpu.raise_exception(pc, CpuException::ArithmeticOverflow)),
            }
        }
        0x2D => {
            // DADDU
            let res = cpu.gpr(rs).wrapping_add(cpu.gpr(rt));
            cpu.set_gpr(rd, res);
        }
        0x2E => {
            // DSUB
            let v_rs = cpu.gpr(rs) as i64;
            let v_rt = cpu.gpr(rt) as i64;
            match v_rs.checked_sub(v_rt) {
                Some(v) => cpu.set_gpr(rd, v as u64),
                None => return Err(cpu.raise_exception(pc, CpuException::ArithmeticOverflow)),
            }
        }
        0x2F => {
            // DSUBU
            let res = cpu.gpr(rs).wrapping_sub(cpu.gpr(rt));
            cpu.set_gpr(rd, res);
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc,
                word: inst.raw(),
            })
        }
    }

    Ok(1)
}

/// Execute immediate integer ALU instructions (`ADDI`, `ADDIU`, `DADDI`, `DADDIU`,
/// `SLTI`, `SLTIU`, `ANDI`, `ORI`, `XORI`, `LUI`).
pub fn exec_imm(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let rs = inst.rs();
    let rt = inst.rt();
    let simm = inst.simm16();
    let imm16 = inst.imm16();

    match inst.opcode() {
        0x08 => {
            // ADDI
            let v_rs = cpu.gpr(rs) as i32;
            match v_rs.checked_add(simm as i32) {
                Some(v) => cpu.set_gpr(rt, v as i64 as u64),
                None => return Err(cpu.raise_exception(pc, CpuException::ArithmeticOverflow)),
            }
        }
        0x09 => {
            // ADDIU
            let v_rs = cpu.gpr(rs) as u32;
            let res = v_rs.wrapping_add(simm as i32 as u32) as i32 as i64 as u64;
            cpu.set_gpr(rt, res);
        }
        0x0A => {
            // SLTI
            let res = if (cpu.gpr(rs) as i64) < simm { 1 } else { 0 };
            cpu.set_gpr(rt, res);
        }
        0x0B => {
            // SLTIU
            let imm_u64 = simm as u64;
            let res = if cpu.gpr(rs) < imm_u64 { 1 } else { 0 };
            cpu.set_gpr(rt, res);
        }
        0x0C => {
            // ANDI
            cpu.set_gpr(rt, cpu.gpr(rs) & u64::from(imm16));
        }
        0x0D => {
            // ORI
            cpu.set_gpr(rt, cpu.gpr(rs) | u64::from(imm16));
        }
        0x0E => {
            // XORI
            cpu.set_gpr(rt, cpu.gpr(rs) ^ u64::from(imm16));
        }
        0x0F => {
            // LUI
            let res = ((u32::from(imm16) << 16) as i32 as i64) as u64;
            cpu.set_gpr(rt, res);
        }
        0x18 => {
            // DADDI
            let v_rs = cpu.gpr(rs) as i64;
            match v_rs.checked_add(simm) {
                Some(v) => cpu.set_gpr(rt, v as u64),
                None => return Err(cpu.raise_exception(pc, CpuException::ArithmeticOverflow)),
            }
        }
        0x19 => {
            // DADDIU
            let res = cpu.gpr(rs).wrapping_add(simm as u64);
            cpu.set_gpr(rt, res);
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc,
                word: inst.raw(),
            })
        }
    }

    Ok(1)
}

#[cfg(test)]
#[path = "alu_tests.rs"]
mod tests;
