//! VR4300 integer load/store, unaligned, atomic LL/SC, and SYNC family.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use crate::bus::Bus;
use crate::trap::Trap;

use super::decode::Inst;
use super::mmu;
use super::InterpCpu;

#[cfg(test)]
#[path = "load_store_tests.rs"]
mod tests;

/// Execute primary integer load/store instructions (`LB`, `LBU`, `LH`, `LHU`, `LW`, `LWU`, `LD`,
/// `SB`, `SH`, `SW`, `SD`, `LWL`, `LWR`, `LDL`, `LDR`, `SWL`, `SWR`, `SDL`, `SDR`, `LL`, `SC`, `LLD`, `SCD`).
pub fn exec_primary(cpu: &mut InterpCpu, bus: &mut Bus, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let opcode = inst.opcode();
    let rs = inst.rs();
    let rt = inst.rt();
    let simm16 = inst.simm16();

    let va = cpu.gpr(rs).wrapping_add(simm16 as u64);

    // Alignment check BEFORE MMU translation (MUST NOT modify destination register or memory on exception!)
    match opcode {
        // Halfword: LH (0x21), LHU (0x25), SH (0x29)
        0x21 | 0x25 | 0x29 if (va & 1) != 0 => {
            let exc = if opcode == 0x29 {
                crate::cpu::exceptions::CpuException::AddressErrorStore { bad_vaddr: va }
            } else {
                crate::cpu::exceptions::CpuException::AddressErrorLoad { bad_vaddr: va }
            };
            return Err(cpu.raise_exception(pc, exc));
        }
        // Word: LW (0x23), LWU (0x27), SW (0x2B), LL (0x30), SC (0x38)
        0x23 | 0x27 | 0x2B | 0x30 | 0x38 if (va & 3) != 0 => {
            let exc = if opcode == 0x2B || opcode == 0x38 {
                crate::cpu::exceptions::CpuException::AddressErrorStore { bad_vaddr: va }
            } else {
                crate::cpu::exceptions::CpuException::AddressErrorLoad { bad_vaddr: va }
            };
            return Err(cpu.raise_exception(pc, exc));
        }
        // Doubleword: LD (0x37), LLD (0x34), SD (0x3F), SCD (0x3C)
        0x37 | 0x34 | 0x3F | 0x3C if (va & 7) != 0 => {
            let exc = if opcode == 0x3F || opcode == 0x3C {
                crate::cpu::exceptions::CpuException::AddressErrorStore { bad_vaddr: va }
            } else {
                crate::cpu::exceptions::CpuException::AddressErrorLoad { bad_vaddr: va }
            };
            return Err(cpu.raise_exception(pc, exc));
        }
        _ => {}
    }

    // Determine whether access is a write for MMU translation
    let is_write = matches!(
        opcode,
        0x28 // SB
        | 0x29 // SH
        | 0x2B // SW
        | 0x3F // SD
        | 0x2A // SWL
        | 0x2E // SWR
        | 0x2C // SDL
        | 0x2D // SDR
        | 0x38 // SC
        | 0x3C // SCD
    );

    // For unaligned word ops (LWL/LWR/SWL/SWR), translate base aligned address (va & !3)
    // For unaligned doubleword ops (LDL/LDR/SDL/SDR), translate base aligned address (va & !7)
    let trans_va = match opcode {
        0x22 | 0x26 | 0x2A | 0x2E => va & !3,
        0x1A | 0x1B | 0x2C | 0x2D => va & !7,
        _ => va,
    };

    let (pa, cache) = match mmu::translate_va(trans_va, is_write) {
        Ok(pair) => pair,
        Err(exc) => return Err(cpu.raise_exception(pc, exc)),
    };

    match opcode {
        0x20 => {
            // LB
            let val = bus.read_u8(pa, cache)?;
            cpu.set_gpr(rt, (val as i8 as i64) as u64);
        }
        0x24 => {
            // LBU
            let val = bus.read_u8(pa, cache)?;
            cpu.set_gpr(rt, u64::from(val));
        }
        0x21 => {
            // LH
            let val = bus.read_u16(pa, cache)?;
            cpu.set_gpr(rt, (val as i16 as i64) as u64);
        }
        0x25 => {
            // LHU
            let val = bus.read_u16(pa, cache)?;
            cpu.set_gpr(rt, u64::from(val));
        }
        0x23 => {
            // LW
            let val = bus.read_u32(pa, cache)?;
            cpu.set_gpr(rt, (val as i32 as i64) as u64);
        }
        0x27 => {
            // LWU
            let val = bus.read_u32(pa, cache)?;
            cpu.set_gpr(rt, u64::from(val));
        }
        0x37 => {
            // LD
            let val = bus.read_u64(pa, cache)?;
            cpu.set_gpr(rt, val);
        }
        0x28 => {
            // SB
            let val = cpu.gpr(rt) as u8;
            bus.write_u8(pa, val, cache)?;
        }
        0x29 => {
            // SH
            let val = cpu.gpr(rt) as u16;
            bus.write_u16(pa, val, cache)?;
        }
        0x2B => {
            // SW
            let val = cpu.gpr(rt) as u32;
            bus.write_u32(pa, val, cache)?;
        }
        0x3F => {
            // SD
            let val = cpu.gpr(rt);
            bus.write_u64(pa, val, cache)?;
        }
        0x22 => {
            // LWL (big-endian)
            let off = (va & 3) as u32;
            let mem = bus.read_u32(pa, cache)?;
            let old = cpu.gpr(rt) as u32;
            let res32 = match off {
                0 => mem,
                1 => (mem << 8) | (old & 0x0000_00FF),
                2 => (mem << 16) | (old & 0x0000_FFFF),
                3 => (mem << 24) | (old & 0x00FF_FFFF),
                _ => unreachable!(),
            };
            cpu.set_gpr(rt, (res32 as i32 as i64) as u64);
        }
        0x26 => {
            // LWR (big-endian)
            let off = (va & 3) as u32;
            let mem = bus.read_u32(pa, cache)?;
            let old = cpu.gpr(rt) as u32;
            let res32 = match off {
                0 => (old & 0xFFFF_FF00) | (mem >> 24),
                1 => (old & 0xFFFF_0000) | (mem >> 16),
                2 => (old & 0xFF00_0000) | (mem >> 8),
                3 => mem,
                _ => unreachable!(),
            };
            cpu.set_gpr(rt, (res32 as i32 as i64) as u64);
        }
        0x2A => {
            // SWL (big-endian)
            let off = (va & 3) as u32;
            let mem = bus.read_u32(pa, cache)?;
            let reg = cpu.gpr(rt) as u32;
            let new_mem = match off {
                0 => reg,
                1 => (mem & 0xFF00_0000) | (reg >> 8),
                2 => (mem & 0xFFFF_0000) | (reg >> 16),
                3 => (mem & 0xFFFF_FF00) | (reg >> 24),
                _ => unreachable!(),
            };
            bus.write_u32(pa, new_mem, cache)?;
        }
        0x2E => {
            // SWR (big-endian)
            let off = (va & 3) as u32;
            let mem = bus.read_u32(pa, cache)?;
            let reg = cpu.gpr(rt) as u32;
            let new_mem = match off {
                0 => (reg << 24) | (mem & 0x00FF_FFFF),
                1 => (reg << 16) | (mem & 0x0000_FFFF),
                2 => (reg << 8) | (mem & 0x0000_00FF),
                3 => reg,
                _ => unreachable!(),
            };
            bus.write_u32(pa, new_mem, cache)?;
        }
        0x1A => {
            // LDL (big-endian)
            let off = (va & 7) as u32;
            let shift = off * 8;
            let mem = bus.read_u64(pa, cache)?;
            let old = cpu.gpr(rt);
            let mask = if shift == 0 {
                0
            } else {
                u64::MAX >> (64 - shift)
            };
            let res = (mem << shift) | (old & mask);
            cpu.set_gpr(rt, res);
        }
        0x1B => {
            // LDR (big-endian)
            let off = (va & 7) as u32;
            let shift = (7 - off) * 8;
            let mem = bus.read_u64(pa, cache)?;
            let old = cpu.gpr(rt);
            let mask = if shift == 0 {
                0
            } else {
                u64::MAX << (64 - shift)
            };
            let res = (old & mask) | (mem >> shift);
            cpu.set_gpr(rt, res);
        }
        0x2C => {
            // SDL (big-endian)
            let off = (va & 7) as u32;
            let shift = off * 8;
            let mem = bus.read_u64(pa, cache)?;
            let reg = cpu.gpr(rt);
            let mask = if shift == 0 {
                0
            } else {
                u64::MAX << (64 - shift)
            };
            let new_mem = (mem & mask) | (reg >> shift);
            bus.write_u64(pa, new_mem, cache)?;
        }
        0x2D => {
            // SDR (big-endian)
            let off = (va & 7) as u32;
            let shift = (7 - off) * 8;
            let mem = bus.read_u64(pa, cache)?;
            let reg = cpu.gpr(rt);
            let mask = if shift == 0 {
                0
            } else {
                u64::MAX >> (64 - shift)
            };
            let new_mem = (reg << shift) | (mem & mask);
            bus.write_u64(pa, new_mem, cache)?;
        }
        0x30 => {
            // LL
            let val = bus.read_u32(pa, cache)?;
            cpu.set_gpr(rt, (val as i32 as i64) as u64);
            cpu.set_ll_bit(true);
            cpu.set_ll_addr(pa);
        }
        0x34 => {
            // LLD
            let val = bus.read_u64(pa, cache)?;
            cpu.set_gpr(rt, val);
            cpu.set_ll_bit(true);
            cpu.set_ll_addr(pa);
        }
        0x38 => {
            // SC
            if cpu.ll_bit() {
                bus.write_u32(pa, cpu.gpr(rt) as u32, cache)?;
                cpu.set_gpr(rt, 1);
                cpu.set_ll_bit(false);
            } else {
                cpu.set_gpr(rt, 0);
            }
        }
        0x3C => {
            // SCD
            if cpu.ll_bit() {
                bus.write_u64(pa, cpu.gpr(rt), cache)?;
                cpu.set_gpr(rt, 1);
                cpu.set_ll_bit(false);
            } else {
                cpu.set_gpr(rt, 0);
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

/// Execute `SYNC` (`SPECIAL` funct `0x0F`).
pub fn exec_sync(_cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    if inst.funct() == 0x0F {
        Ok(1)
    } else {
        Err(Trap::UnimplementedOpcode {
            pc,
            word: inst.raw(),
        })
    }
}
