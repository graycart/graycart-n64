//! RSP Scalar Unit (SU) 32-bit MIPS subset decoder and executor.
//!
//! Cited: N64brew — Reality Signal Processor (CPU / Scalar Unit)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/CPU_Core · docs/03-rsp.md §2
//!
//! Note: Vector Unit (`COP2`, `LWC2`, `SWC2`) opcodes land in M4 and MUST trap cleanly
//! with `Trap::UnimplementedOpcode` here (Standing Rule #1: Trap, don't stub).

#[cfg(test)]
#[path = "scalar_tests.rs"]
mod tests;

use crate::bus::{Bus, CacheAttr};
use crate::mi::MI_INTR_SP;
use crate::rsp::Sp;
use crate::trap::Trap;

use super::InterpRsp;

/// Execute a single RSP Scalar Unit instruction at `bus.sp.pc` (updating `bus.sp.pc`
/// and delay-slot state). Returns `Ok(1)` on normal instruction retirement.
pub fn step_scalar(rsp: &mut InterpRsp, bus: &mut Bus) -> Result<u64, Trap> {
    let pc = bus.sp.pc & 0x0FFC;
    let word = rsp.fetch_imem(bus, pc);

    // PC update semantics for delay slots:
    // If there is a pending `next_pc` from a previous branch/jump, this instruction
    // is the delay slot. After this instruction executes, `bus.sp.pc` becomes `next_pc`.
    // Otherwise, normal next PC is `pc.wrapping_add(4) & 0x0FFC`.
    let normal_next_pc = if let Some(target) = rsp.next_pc.take() {
        target & 0x0FFC
    } else {
        pc.wrapping_add(4) & 0x0FFC
    };

    let mut branch_to: Option<u16> = None;

    let op = (word >> 26) & 0x3F;
    let rs = ((word >> 21) & 0x1F) as usize;
    let rt = ((word >> 16) & 0x1F) as usize;
    let rd = ((word >> 11) & 0x1F) as usize;
    let sa = (word >> 6) & 0x1F;
    let funct = word & 0x3F;
    let imm = (word & 0xFFFF) as u16;
    let simm = (imm as i16) as i32 as u32;
    let zimm = u32::from(imm);
    let branch_target = pc.wrapping_add(4).wrapping_add((simm << 2) as u16) & 0x0FFC;
    let jump_target = ((word & 0x03FF_FFFF) << 2) as u16 & 0x0FFC;
    let link = u32::from(normal_next_pc.wrapping_add(4) & 0x0FFC);

    match op {
        0x00 => {
            // SPECIAL
            match funct {
                0x00 => {
                    // SLL
                    let val = rsp.gpr(rt).wrapping_shl(sa);
                    rsp.set_gpr(rd, val);
                }
                0x02 => {
                    // SRL
                    let val = rsp.gpr(rt).wrapping_shr(sa);
                    rsp.set_gpr(rd, val);
                }
                0x03 => {
                    // SRA
                    let val = ((rsp.gpr(rt) as i32) >> sa) as u32;
                    rsp.set_gpr(rd, val);
                }
                0x04 => {
                    // SLLV
                    let shift = rsp.gpr(rs) & 0x1F;
                    let val = rsp.gpr(rt).wrapping_shl(shift);
                    rsp.set_gpr(rd, val);
                }
                0x06 => {
                    // SRLV
                    let shift = rsp.gpr(rs) & 0x1F;
                    let val = rsp.gpr(rt).wrapping_shr(shift);
                    rsp.set_gpr(rd, val);
                }
                0x07 => {
                    // SRAV
                    let shift = rsp.gpr(rs) & 0x1F;
                    let val = ((rsp.gpr(rt) as i32) >> shift) as u32;
                    rsp.set_gpr(rd, val);
                }
                0x08 => {
                    // JR
                    let target = (rsp.gpr(rs) as u16) & 0x0FFC;
                    branch_to = Some(target);
                }
                0x09 => {
                    // JALR
                    let target = (rsp.gpr(rs) as u16) & 0x0FFC;
                    // Note: read rs before writing rd in case rs == rd
                    rsp.set_gpr(rd, link);
                    branch_to = Some(target);
                }
                0x0D => {
                    // BREAK
                    bus.sp.status |= Sp::STATUS_HALT | Sp::STATUS_BROKE;
                    if bus.sp.intr_on_break() {
                        bus.mi.set_intr(MI_INTR_SP);
                    }
                }
                0x20 | 0x21 => {
                    // ADD | ADDU (no overflow trap on RSP)
                    let val = rsp.gpr(rs).wrapping_add(rsp.gpr(rt));
                    rsp.set_gpr(rd, val);
                }
                0x22 | 0x23 => {
                    // SUB | SUBU
                    let val = rsp.gpr(rs).wrapping_sub(rsp.gpr(rt));
                    rsp.set_gpr(rd, val);
                }
                0x24 => {
                    // AND
                    let val = rsp.gpr(rs) & rsp.gpr(rt);
                    rsp.set_gpr(rd, val);
                }
                0x25 => {
                    // OR
                    let val = rsp.gpr(rs) | rsp.gpr(rt);
                    rsp.set_gpr(rd, val);
                }
                0x26 => {
                    // XOR
                    let val = rsp.gpr(rs) ^ rsp.gpr(rt);
                    rsp.set_gpr(rd, val);
                }
                0x27 => {
                    // NOR
                    let val = !(rsp.gpr(rs) | rsp.gpr(rt));
                    rsp.set_gpr(rd, val);
                }
                0x2A => {
                    // SLT
                    let val = u32::from((rsp.gpr(rs) as i32) < (rsp.gpr(rt) as i32));
                    rsp.set_gpr(rd, val);
                }
                0x2B => {
                    // SLTU
                    let val = u32::from(rsp.gpr(rs) < rsp.gpr(rt));
                    rsp.set_gpr(rd, val);
                }
                _ => {
                    return Err(Trap::UnimplementedOpcode {
                        pc: u64::from(pc),
                        word,
                    })
                }
            }
        }
        0x01 => {
            // REGIMM
            let rs_val = rsp.gpr(rs);
            match rt {
                0x00 => {
                    // BLTZ
                    if (rs_val as i32) < 0 {
                        branch_to = Some(branch_target);
                    }
                }
                0x01 => {
                    // BGEZ
                    if (rs_val as i32) >= 0 {
                        branch_to = Some(branch_target);
                    }
                }
                0x10 => {
                    // BLTZAL
                    rsp.set_gpr(31, link);
                    if (rs_val as i32) < 0 {
                        branch_to = Some(branch_target);
                    }
                }
                0x11 => {
                    // BGEZAL
                    rsp.set_gpr(31, link);
                    if (rs_val as i32) >= 0 {
                        branch_to = Some(branch_target);
                    }
                }
                _ => {
                    return Err(Trap::UnimplementedOpcode {
                        pc: u64::from(pc),
                        word,
                    })
                }
            }
        }
        0x02 => {
            // J
            branch_to = Some(jump_target);
        }
        0x03 => {
            // JAL
            rsp.set_gpr(31, link);
            branch_to = Some(jump_target);
        }
        0x04 => {
            // BEQ
            if rsp.gpr(rs) == rsp.gpr(rt) {
                branch_to = Some(branch_target);
            }
        }
        0x05 => {
            // BNE
            if rsp.gpr(rs) != rsp.gpr(rt) {
                branch_to = Some(branch_target);
            }
        }
        0x06 => {
            // BLEZ
            if (rsp.gpr(rs) as i32) <= 0 {
                branch_to = Some(branch_target);
            }
        }
        0x07 => {
            // BGTZ
            if (rsp.gpr(rs) as i32) > 0 {
                branch_to = Some(branch_target);
            }
        }
        0x08 | 0x09 => {
            // ADDI | ADDIU
            let val = rsp.gpr(rs).wrapping_add(simm);
            rsp.set_gpr(rt, val);
        }
        0x0A => {
            // SLTI
            let val = u32::from((rsp.gpr(rs) as i32) < (simm as i32));
            rsp.set_gpr(rt, val);
        }
        0x0B => {
            // SLTIU
            let val = u32::from(rsp.gpr(rs) < simm);
            rsp.set_gpr(rt, val);
        }
        0x0C => {
            // ANDI
            let val = rsp.gpr(rs) & zimm;
            rsp.set_gpr(rt, val);
        }
        0x0D => {
            // ORI
            let val = rsp.gpr(rs) | zimm;
            rsp.set_gpr(rt, val);
        }
        0x0E => {
            // XORI
            let val = rsp.gpr(rs) ^ zimm;
            rsp.set_gpr(rt, val);
        }
        0x0F => {
            // LUI
            let val = zimm << 16;
            rsp.set_gpr(rt, val);
        }
        0x10 => {
            // COP0
            match rs {
                0x00 => {
                    // MFC0 rt, rd
                    let val = match rd {
                        0..=7 => {
                            let pa = 0x0404_0000 + (rd as u32) * 4;
                            bus.read_u32(pa, CacheAttr::Uncached)?
                        }
                        8..=15 => {
                            let pa = 0x0410_0000 + ((rd - 8) as u32) * 4;
                            bus.read_u32(pa, CacheAttr::Uncached)?
                        }
                        _ => {
                            return Err(Trap::UnimplementedOpcode {
                                pc: u64::from(pc),
                                word,
                            })
                        }
                    };
                    rsp.set_gpr(rt, val);
                }
                0x04 => {
                    // MTC0 rt, rd
                    let val = rsp.gpr(rt);
                    match rd {
                        0..=7 => {
                            let pa = 0x0404_0000 + (rd as u32) * 4;
                            bus.write_u32(pa, val, CacheAttr::Uncached)?;
                        }
                        8..=15 => {
                            let pa = 0x0410_0000 + ((rd - 8) as u32) * 4;
                            bus.write_u32(pa, val, CacheAttr::Uncached)?;
                        }
                        _ => {
                            return Err(Trap::UnimplementedOpcode {
                                pc: u64::from(pc),
                                word,
                            })
                        }
                    }
                }
                _ => {
                    return Err(Trap::UnimplementedOpcode {
                        pc: u64::from(pc),
                        word,
                    })
                }
            }
        }
        0x12 => {
            // COP2 (RSP Vector Unit)
            if (word & (1 << 25)) == 0 {
                super::vu_recip::exec_cop2_move(rsp, pc, word)?;
            } else {
                match funct {
                    0x00..=0x11 | 0x13..=0x15 | 0x1D => {
                        super::vu_arith::exec_vu_arith(rsp, pc, word)?;
                    }
                    0x20..=0x2D | 0x33 | 0x37 | 0x3F => {
                        super::vu_logic_cmp::exec_vu_logic_cmp(rsp, pc, word)?;
                    }
                    0x30..=0x32 | 0x34..=0x36 => {
                        super::vu_recip::exec_vu_recip(rsp, pc, word)?;
                    }
                    _ => {
                        return Err(Trap::UnimplementedOpcode {
                            pc: u64::from(pc),
                            word,
                        });
                    }
                }
            }
        }
        0x20 => {
            // LB
            let addr = rsp.gpr(rs).wrapping_add(simm);
            let val = (rsp.read_dmem_u8(bus, addr) as i8) as i32 as u32;
            rsp.set_gpr(rt, val);
        }
        0x21 => {
            // LH
            let addr = rsp.gpr(rs).wrapping_add(simm);
            let val = (rsp.read_dmem_u16(bus, addr) as i16) as i32 as u32;
            rsp.set_gpr(rt, val);
        }
        0x23 | 0x27 => {
            // LW | LWU
            let addr = rsp.gpr(rs).wrapping_add(simm);
            let val = rsp.read_dmem_u32(bus, addr);
            rsp.set_gpr(rt, val);
        }
        0x24 => {
            // LBU
            let addr = rsp.gpr(rs).wrapping_add(simm);
            let val = u32::from(rsp.read_dmem_u8(bus, addr));
            rsp.set_gpr(rt, val);
        }
        0x25 => {
            // LHU
            let addr = rsp.gpr(rs).wrapping_add(simm);
            let val = u32::from(rsp.read_dmem_u16(bus, addr));
            rsp.set_gpr(rt, val);
        }
        0x28 => {
            // SB
            let addr = rsp.gpr(rs).wrapping_add(simm);
            rsp.write_dmem_u8(bus, addr, rsp.gpr(rt) as u8);
        }
        0x29 => {
            // SH
            let addr = rsp.gpr(rs).wrapping_add(simm);
            rsp.write_dmem_u16(bus, addr, rsp.gpr(rt) as u16);
        }
        0x2B => {
            // SW
            let addr = rsp.gpr(rs).wrapping_add(simm);
            rsp.write_dmem_u32(bus, addr, rsp.gpr(rt));
        }
        0x32 => {
            // LWC2 (RSP Vector Load)
            super::vu_lsu::exec_lwc2(rsp, bus, pc, word)?;
        }
        0x3A => {
            // SWC2 (RSP Vector Store)
            super::vu_lsu::exec_swc2(rsp, bus, pc, word)?;
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc: u64::from(pc),
                word,
            })
        }
    }

    // Update PC and delay slot state
    bus.sp.pc = normal_next_pc;
    if let Some(target) = branch_to {
        rsp.next_pc = Some(target & 0x0FFC);
    }

    Ok(1)
}
