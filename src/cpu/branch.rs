//! NEC VR4300 (MIPS III) Jumps, Branches, and Branch Likely execution backend.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use super::{decode::Inst, InterpCpu};
use crate::trap::Trap;

#[cfg(test)]
#[path = "branch_tests.rs"]
mod tests;

/// Execute primary-opcode jumps and branches (Opcodes 0x02, 0x03, 0x04..=0x07, 0x14..=0x17).
pub fn exec_primary(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let delay_slot_pc = cpu.next_pc();
    let branch_target = delay_slot_pc.wrapping_add((inst.simm16() << 2) as u64);
    let jump_target = (delay_slot_pc & 0xFFFF_FFFF_F000_0000) | (u64::from(inst.target26()) << 2);

    match inst.opcode() {
        0x02 => {
            // J
            cpu.take_branch(jump_target);
        }
        0x03 => {
            // JAL
            let link_addr = delay_slot_pc.wrapping_add(4);
            cpu.set_gpr(31, link_addr);
            cpu.take_branch(jump_target);
        }
        0x04 => {
            // BEQ
            if cpu.gpr(inst.rs()) == cpu.gpr(inst.rt()) {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x05 => {
            // BNE
            if cpu.gpr(inst.rs()) != cpu.gpr(inst.rt()) {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x06 => {
            // BLEZ
            if (cpu.gpr(inst.rs()) as i64) <= 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x07 => {
            // BGTZ
            if (cpu.gpr(inst.rs()) as i64) > 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x14 => {
            // BEQL
            if cpu.gpr(inst.rs()) == cpu.gpr(inst.rt()) {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
        }
        0x15 => {
            // BNEL
            if cpu.gpr(inst.rs()) != cpu.gpr(inst.rt()) {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
        }
        0x16 => {
            // BLEZL
            if (cpu.gpr(inst.rs()) as i64) <= 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
        }
        0x17 => {
            // BGTZL
            if (cpu.gpr(inst.rs()) as i64) > 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
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

/// Execute SPECIAL-opcode register jumps (JR = 0x08, JALR = 0x09).
pub fn exec_special(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let delay_slot_pc = cpu.next_pc();
    let link_addr = delay_slot_pc.wrapping_add(4);

    match inst.funct() {
        0x08 => {
            // JR
            let target = cpu.gpr(inst.rs());
            cpu.take_branch(target);
        }
        0x09 => {
            // JALR
            // CRITICAL: read rs BEFORE writing rd so JALR r1, r1 jumps to old r1 and writes pc + 8 to r1!
            let target = cpu.gpr(inst.rs());
            cpu.set_gpr(inst.rd(), link_addr);
            cpu.take_branch(target);
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

/// Execute REGIMM-opcode branch-on-register-vs-zero (Opcode 0x01).
pub fn exec_regimm(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let delay_slot_pc = cpu.next_pc();
    let branch_target = delay_slot_pc.wrapping_add((inst.simm16() << 2) as u64);
    let link_addr = delay_slot_pc.wrapping_add(4);
    let rs_val = cpu.gpr(inst.rs()) as i64;

    match inst.rt() {
        0x00 => {
            // BLTZ
            if rs_val < 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x01 => {
            // BGEZ
            if rs_val >= 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x02 => {
            // BLTZL
            if rs_val < 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
        }
        0x03 => {
            // BGEZL
            if rs_val >= 0 {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
        }
        0x10 => {
            // BLTZAL
            let cond = rs_val < 0;
            cpu.set_gpr(31, link_addr);
            if cond {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x11 => {
            // BGEZAL
            let cond = rs_val >= 0;
            cpu.set_gpr(31, link_addr);
            if cond {
                cpu.take_branch(branch_target);
            } else {
                cpu.not_taken_branch();
            }
        }
        0x12 => {
            // BLTZALL
            let cond = rs_val < 0;
            cpu.set_gpr(31, link_addr);
            if cond {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
        }
        0x13 => {
            // BGEZALL
            let cond = rs_val >= 0;
            cpu.set_gpr(31, link_addr);
            if cond {
                cpu.take_branch(branch_target);
            } else {
                cpu.nullify_delay_slot();
            }
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
