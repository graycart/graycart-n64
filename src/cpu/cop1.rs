//! VR4300 COP1 IEEE-754 Floating-Point Unit.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapters 7–9 & 17
//!   via docs/01-cpu-r4300i.md

use serde::{Deserialize, Serialize};

use crate::bus::Bus;
use crate::trap::Trap;

use super::decode::Inst;
use super::mmu;
use super::{CpuException, InterpCpu};

/// VR4300 COP1 FPU state (32 × 64-bit register slots + `FCR0` + `FCR31`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cop1 {
    /// 32 raw 64-bit FPR backing slots (`FR=0` pairs low 32 bits of `f[even]`/`f[odd]`; `FR=1` uses full 64-bit `f[i]`).
    pub fpr: [u64; 32],
    /// `FCR0` Implementation/Revision register (`0x0000_0B00` on VR4300).
    pub fcr0: u32,
    /// `FCR31` Floating-Point Control/Status register (FS, C, Cause, Enables, Flags, RM).
    pub fcr31: u32,
}

impl Cop1 {
    pub const FCR0_VR4300: u32 = 0x0000_0B00;

    #[must_use]
    pub fn new() -> Self {
        Self {
            fpr: [0; 32],
            fcr0: Self::FCR0_VR4300,
            fcr31: 0,
        }
    }

    pub fn reset(&mut self) {
        self.fpr = [0; 32];
        self.fcr0 = Self::FCR0_VR4300;
        self.fcr31 = 0;
    }

    #[inline]
    pub fn read_u32(&self, reg: usize, fr: bool) -> u32 {
        let _ = fr;
        self.fpr[reg & 0x1F] as u32
    }

    #[inline]
    pub fn write_u32(&mut self, reg: usize, val: u32, fr: bool) {
        let r = reg & 0x1F;
        if fr {
            self.fpr[r] = (self.fpr[r] & 0xFFFF_FFFF_0000_0000) | u64::from(val);
        } else {
            self.fpr[r] = u64::from(val);
        }
    }

    #[inline]
    pub fn read_u64(&self, reg: usize, fr: bool) -> u64 {
        let r = reg & 0x1F;
        if fr {
            self.fpr[r]
        } else {
            let e = r & !1;
            (self.fpr[e] & 0xFFFF_FFFF) | ((self.fpr[e + 1] & 0xFFFF_FFFF) << 32)
        }
    }

    #[inline]
    pub fn write_u64(&mut self, reg: usize, val: u64, fr: bool) {
        let r = reg & 0x1F;
        if fr {
            self.fpr[r] = val;
        } else {
            let e = r & !1;
            self.fpr[e] = val & 0xFFFF_FFFF;
            self.fpr[e + 1] = val >> 32;
        }
    }
}

impl Default for Cop1 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "cop1_tests.rs"]
mod tests;

fn check_cu1(cpu: &mut InterpCpu, pc: u64) -> Result<(), Trap> {
    if !cpu.cop0.cu1_enabled() {
        return Err(cpu.raise_exception(pc, CpuException::CoprocessorUnusable { unit: 1 }));
    }
    Ok(())
}

fn apply_rounding_f32(v: f32, rm: u32) -> f32 {
    match rm & 3 {
        0 => v.round_ties_even(),
        1 => v.trunc(),
        2 => v.ceil(),
        3 => v.floor(),
        _ => v.round_ties_even(),
    }
}

fn apply_rounding_f64(v: f64, rm: u32) -> f64 {
    match rm & 3 {
        0 => v.round_ties_even(),
        1 => v.trunc(),
        2 => v.ceil(),
        3 => v.floor(),
        _ => v.round_ties_even(),
    }
}

pub fn exec_cop1(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    check_cu1(cpu, pc)?;

    let fr = cpu.cop0.fr_mode();
    let fmt = inst.rs() as u8;
    let rt = inst.rt();
    let fs = inst.rd();
    let fd = inst.fd();
    let funct = inst.funct();

    match fmt {
        0x00 => {
            // MFC1
            let val = cpu.cop1.read_u32(fs, fr) as i32 as i64 as u64;
            cpu.set_gpr(rt, val);
            Ok(1)
        }
        0x01 => {
            // DMFC1
            let val = cpu.cop1.read_u64(fs, fr);
            cpu.set_gpr(rt, val);
            Ok(1)
        }
        0x02 => {
            // CFC1
            let val = match fs {
                0 => cpu.cop1.fcr0,
                31 => cpu.cop1.fcr31,
                _ => 0,
            };
            cpu.set_gpr(rt, val as i32 as i64 as u64);
            Ok(1)
        }
        0x04 => {
            // MTC1
            let val = cpu.gpr(rt) as u32;
            cpu.cop1.write_u32(fs, val, fr);
            Ok(1)
        }
        0x05 => {
            // DMTC1
            let val = cpu.gpr(rt);
            cpu.cop1.write_u64(fs, val, fr);
            Ok(1)
        }
        0x06 => {
            // CTC1
            if fs == 31 {
                cpu.cop1.fcr31 = (cpu.gpr(rt) as u32) & 0x0183_FFFF;
            }
            Ok(1)
        }
        0x08 => {
            // BC1
            let cond = (cpu.cop1.fcr31 & (1 << 23)) != 0;
            let variant = inst.rt() & 3;
            let delay_slot_pc = cpu.next_pc();
            let target = delay_slot_pc.wrapping_add(((inst.simm16() as i16 as i64) << 2) as u64);
            match variant {
                0 => {
                    // BC1F
                    if !cond {
                        cpu.take_branch(target);
                    } else {
                        cpu.not_taken_branch();
                    }
                }
                1 => {
                    // BC1T
                    if cond {
                        cpu.take_branch(target);
                    } else {
                        cpu.not_taken_branch();
                    }
                }
                2 => {
                    // BC1FL
                    if !cond {
                        cpu.take_branch(target);
                    } else {
                        cpu.nullify_delay_slot();
                    }
                }
                3 => {
                    // BC1TL
                    if cond {
                        cpu.take_branch(target);
                    } else {
                        cpu.nullify_delay_slot();
                    }
                }
                _ => unreachable!(),
            }
            Ok(1)
        }
        0x10 => {
            // .S (Single Precision)
            let ft_reg = inst.rt();
            match funct {
                0x00 => {
                    // ADD.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let b = f32::from_bits(cpu.cop1.read_u32(ft_reg, fr));
                    let res = a + b;
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x01 => {
                    // SUB.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let b = f32::from_bits(cpu.cop1.read_u32(ft_reg, fr));
                    let res = a - b;
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x02 => {
                    // MUL.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let b = f32::from_bits(cpu.cop1.read_u32(ft_reg, fr));
                    let res = a * b;
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x03 => {
                    // DIV.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let b = f32::from_bits(cpu.cop1.read_u32(ft_reg, fr));
                    let res = a / b;
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x04 => {
                    // SQRT.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let res = a.sqrt();
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x05 => {
                    // ABS.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let res = a.abs();
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x06 => {
                    // MOV.S
                    let a = cpu.cop1.read_u32(fs, fr);
                    cpu.cop1.write_u32(fd, a, fr);
                }
                0x07 => {
                    // NEG.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let res = -a;
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x08 => {
                    // ROUND.L.S (always round-to-nearest ties-to-even per MIPS III)
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let v = a.round_ties_even() as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x09 => {
                    // TRUNC.L.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rounded = a.trunc();
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x0A => {
                    // CEIL.L.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rounded = a.ceil();
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x0B => {
                    // FLOOR.L.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rounded = a.floor();
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x0C => {
                    // ROUND.W.S (always round-to-nearest ties-to-even per MIPS III)
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let v = a.round_ties_even() as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x0D => {
                    // TRUNC.W.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rounded = a.trunc();
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x0E => {
                    // CEIL.W.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rounded = a.ceil();
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x0F => {
                    // FLOOR.W.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rounded = a.floor();
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x21 => {
                    // CVT.D.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let res = a as f64;
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x24 => {
                    // CVT.W.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rm = cpu.cop1.fcr31 & 3;
                    let rounded = apply_rounding_f32(a, rm);
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x25 => {
                    // CVT.L.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let rm = cpu.cop1.fcr31 & 3;
                    let rounded = apply_rounding_f32(a, rm);
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x30..=0x3F => {
                    // C.<cond>.S
                    let a = f32::from_bits(cpu.cop1.read_u32(fs, fr));
                    let b = f32::from_bits(cpu.cop1.read_u32(ft_reg, fr));
                    let unordered = a.is_nan() || b.is_nan();
                    let less = !unordered && a < b;
                    let equal = !unordered && a == b;
                    let c = ((funct & 4) != 0 && less)
                        || ((funct & 2) != 0 && equal)
                        || ((funct & 1) != 0 && unordered);
                    if c {
                        cpu.cop1.fcr31 |= 1 << 23;
                    } else {
                        cpu.cop1.fcr31 &= !(1 << 23);
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
        0x11 => {
            // .D (Double Precision)
            let ft_reg = inst.rt();
            match funct {
                0x00 => {
                    // ADD.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let b = f64::from_bits(cpu.cop1.read_u64(ft_reg, fr));
                    let res = a + b;
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x01 => {
                    // SUB.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let b = f64::from_bits(cpu.cop1.read_u64(ft_reg, fr));
                    let res = a - b;
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x02 => {
                    // MUL.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let b = f64::from_bits(cpu.cop1.read_u64(ft_reg, fr));
                    let res = a * b;
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x03 => {
                    // DIV.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let b = f64::from_bits(cpu.cop1.read_u64(ft_reg, fr));
                    let res = a / b;
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x04 => {
                    // SQRT.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let res = a.sqrt();
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x05 => {
                    // ABS.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let res = a.abs();
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x06 => {
                    // MOV.D
                    let a = cpu.cop1.read_u64(fs, fr);
                    cpu.cop1.write_u64(fd, a, fr);
                }
                0x07 => {
                    // NEG.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let res = -a;
                    cpu.cop1.write_u64(fd, res.to_bits(), fr);
                }
                0x08 => {
                    // ROUND.L.D (always round-to-nearest ties-to-even per MIPS III)
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let v = a.round_ties_even() as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x09 => {
                    // TRUNC.L.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rounded = a.trunc();
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x0A => {
                    // CEIL.L.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rounded = a.ceil();
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x0B => {
                    // FLOOR.L.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rounded = a.floor();
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x0C => {
                    // ROUND.W.D (always round-to-nearest ties-to-even per MIPS III)
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let v = a.round_ties_even() as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x0D => {
                    // TRUNC.W.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rounded = a.trunc();
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x0E => {
                    // CEIL.W.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rounded = a.ceil();
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x0F => {
                    // FLOOR.W.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rounded = a.floor();
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x20 => {
                    // CVT.S.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let res = a as f32;
                    cpu.cop1.write_u32(fd, res.to_bits(), fr);
                }
                0x24 => {
                    // CVT.W.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rm = cpu.cop1.fcr31 & 3;
                    let rounded = apply_rounding_f64(a, rm);
                    let v = rounded as i32;
                    cpu.cop1.write_u32(fd, v as u32, fr);
                }
                0x25 => {
                    // CVT.L.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let rm = cpu.cop1.fcr31 & 3;
                    let rounded = apply_rounding_f64(a, rm);
                    let v = rounded as i64;
                    cpu.cop1.write_u64(fd, v as u64, fr);
                }
                0x30..=0x3F => {
                    // C.<cond>.D
                    let a = f64::from_bits(cpu.cop1.read_u64(fs, fr));
                    let b = f64::from_bits(cpu.cop1.read_u64(ft_reg, fr));
                    let unordered = a.is_nan() || b.is_nan();
                    let less = !unordered && a < b;
                    let equal = !unordered && a == b;
                    let c = ((funct & 4) != 0 && less)
                        || ((funct & 2) != 0 && equal)
                        || ((funct & 1) != 0 && unordered);
                    if c {
                        cpu.cop1.fcr31 |= 1 << 23;
                    } else {
                        cpu.cop1.fcr31 &= !(1 << 23);
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
        0x14 => {
            // .W (Word)
            match funct {
                0x20 => {
                    // CVT.S.W
                    let a = cpu.cop1.read_u32(fs, fr) as i32 as f32;
                    cpu.cop1.write_u32(fd, a.to_bits(), fr);
                }
                0x21 => {
                    // CVT.D.W
                    let a = cpu.cop1.read_u32(fs, fr) as i32 as f64;
                    cpu.cop1.write_u64(fd, a.to_bits(), fr);
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
        0x15 => {
            // .L (Long)
            match funct {
                0x20 => {
                    // CVT.S.L
                    let a = cpu.cop1.read_u64(fs, fr) as i64 as f32;
                    cpu.cop1.write_u32(fd, a.to_bits(), fr);
                }
                0x21 => {
                    // CVT.D.L
                    let a = cpu.cop1.read_u64(fs, fr) as i64 as f64;
                    cpu.cop1.write_u64(fd, a.to_bits(), fr);
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
        _ => Err(Trap::UnimplementedOpcode {
            pc,
            word: inst.raw(),
        }),
    }
}

pub fn exec_load_store(
    cpu: &mut InterpCpu,
    bus: &mut Bus,
    pc: u64,
    inst: Inst,
) -> Result<u64, Trap> {
    check_cu1(cpu, pc)?;

    let fr = cpu.cop0.fr_mode();
    let op = inst.opcode();
    let base = inst.rs();
    let ft = inst.rt();
    let offset = inst.simm16();
    let va = cpu.gpr(base).wrapping_add(offset as u64);

    match op {
        0x31 => {
            // LWC1
            if (va & 3) != 0 {
                let exc = cpu.raise_exception(pc, CpuException::AddressErrorLoad { bad_vaddr: va });
                return Err(exc);
            }
            let (pa, cache) =
                mmu::translate_va_cpu(cpu, va, false).map_err(|e| cpu.raise_exception(pc, e))?;
            let val = bus.read_u32(pa, cache)?;
            cpu.cop1.write_u32(ft, val, fr);
            Ok(1)
        }
        0x35 => {
            // LDC1
            if (va & 7) != 0 {
                let exc = cpu.raise_exception(pc, CpuException::AddressErrorLoad { bad_vaddr: va });
                return Err(exc);
            }
            let (pa, cache) =
                mmu::translate_va_cpu(cpu, va, false).map_err(|e| cpu.raise_exception(pc, e))?;
            let val = bus.read_u64(pa, cache)?;
            cpu.cop1.write_u64(ft, val, fr);
            Ok(1)
        }
        0x39 => {
            // SWC1
            if (va & 3) != 0 {
                let exc =
                    cpu.raise_exception(pc, CpuException::AddressErrorStore { bad_vaddr: va });
                return Err(exc);
            }
            let (pa, cache) =
                mmu::translate_va_cpu(cpu, va, true).map_err(|e| cpu.raise_exception(pc, e))?;
            let val = cpu.cop1.read_u32(ft, fr);
            bus.write_u32(pa, val, cache)?;
            Ok(1)
        }
        0x3D => {
            // SDC1
            if (va & 7) != 0 {
                let exc =
                    cpu.raise_exception(pc, CpuException::AddressErrorStore { bad_vaddr: va });
                return Err(exc);
            }
            let (pa, cache) =
                mmu::translate_va_cpu(cpu, va, true).map_err(|e| cpu.raise_exception(pc, e))?;
            let val = cpu.cop1.read_u64(ft, fr);
            bus.write_u64(pa, val, cache)?;
            Ok(1)
        }
        _ => Err(Trap::UnimplementedOpcode {
            pc,
            word: inst.raw(),
        }),
    }
}
