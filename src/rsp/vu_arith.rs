//! Reality Signal Processor (RSP) Vector Unit Arithmetic Instructions.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit Arithmetic)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

#[cfg(test)]
#[path = "vu_arith_tests.rs"]
mod tests;

use crate::rsp::vu::Vu;
use crate::rsp::InterpRsp;
use crate::trap::Trap;

#[allow(clippy::needless_range_loop)]
pub fn exec_vu_arith(rsp: &mut InterpRsp, pc: u16, word: u32) -> Result<(), Trap> {
    let e = ((word >> 21) & 0x0F) as usize;
    let vt = ((word >> 16) & 0x1F) as usize;
    let vs = ((word >> 11) & 0x1F) as usize;
    let vd = ((word >> 6) & 0x1F) as usize;
    let funct = (word & 0x3F) as usize;

    let vs_v = rsp.vu.vpr[vs];
    let vt_v = rsp.vu.get_vt_swizzled(vt, e);
    let mut out = [0u16; 8];

    match funct {
        0x00 => {
            // VMULF
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i] as i16);
                let acc = (prod << 1) + 0x8000;
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x01 => {
            // VMULU
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i] as i16);
                let acc = (prod << 1) + 0x8000;
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_unsigned(i);
            }
        }
        0x02 => {
            // VRNDP
            for i in 0..8 {
                let mut delta = i64::from(vt_v[i] as i16);
                if (vs & 1) != 0 {
                    delta <<= 16;
                }
                let cur = rsp.vu.acc48(i);
                let acc = if cur >= 0 {
                    cur.wrapping_add(delta)
                } else {
                    cur
                };
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x03 => {
            // VMULQ
            for i in 0..8 {
                let mut prod = i32::from(vs_v[i] as i16) * i32::from(vt_v[i] as i16);
                if prod < 0 {
                    prod += 31;
                }
                rsp.vu.acc_hi[i] = ((prod >> 16) as i16) as u16;
                rsp.vu.acc_md[i] = (prod & 0xFFFF) as u16;
                rsp.vu.acc_lo[i] = 0;
                out[i] = (Vu::clamp_signed(i64::from(prod >> 1)) as u16) & !0x000F;
            }
        }
        0x04 => {
            // VMUDL
            for i in 0..8 {
                let prod = (u64::from(vs_v[i]) * u64::from(vt_v[i])) >> 16;
                rsp.vu.set_acc48(i, prod as i64);
                out[i] = rsp.vu.clamp_acc_low(i);
            }
        }
        0x05 => {
            // VMUDM
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i]);
                rsp.vu.set_acc48(i, prod);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x06 => {
            // VMUDN
            for i in 0..8 {
                let prod = i64::from(vs_v[i]) * i64::from(vt_v[i] as i16);
                rsp.vu.set_acc48(i, prod);
                out[i] = rsp.vu.clamp_acc_low(i);
            }
        }
        0x07 => {
            // VMUDH
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i] as i16);
                rsp.vu.set_acc48(i, prod << 16);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x08 => {
            // VMACF
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i] as i16);
                let acc = rsp.vu.acc48(i).wrapping_add(prod << 1);
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x09 => {
            // VMACU
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i] as i16);
                let acc = rsp.vu.acc48(i).wrapping_add(prod << 1);
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_unsigned(i);
            }
        }
        0x0A => {
            // VRNDN
            for i in 0..8 {
                let mut delta = i64::from(vt_v[i] as i16);
                if (vs & 1) != 0 {
                    delta <<= 16;
                }
                let cur = rsp.vu.acc48(i);
                let acc = if cur < 0 {
                    cur.wrapping_add(delta)
                } else {
                    cur
                };
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x0B => {
            // VMACQ
            for i in 0..8 {
                let mut prod =
                    (i32::from(rsp.vu.acc_hi[i] as i16) << 16) | i32::from(rsp.vu.acc_md[i]);
                if prod < 0 && (prod & 0x20) == 0 {
                    prod += 0x20;
                } else if prod >= 0x20 && (prod & 0x20) == 0 {
                    prod -= 0x20;
                }
                rsp.vu.acc_hi[i] = ((prod >> 16) as i16) as u16;
                rsp.vu.acc_md[i] = (prod & 0xFFFF) as u16;
                out[i] = (Vu::clamp_signed(i64::from(prod >> 1)) as u16) & !0x000F;
            }
        }
        0x0C => {
            // VMACL
            for i in 0..8 {
                let prod = (u64::from(vs_v[i]) * u64::from(vt_v[i])) >> 16;
                let acc = rsp.vu.acc48(i).wrapping_add(prod as i64);
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_low(i);
            }
        }
        0x0D => {
            // VMACM
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i]);
                let acc = rsp.vu.acc48(i).wrapping_add(prod);
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x0E => {
            // VMACN
            for i in 0..8 {
                let prod = i64::from(vs_v[i]) * i64::from(vt_v[i] as i16);
                let acc = rsp.vu.acc48(i).wrapping_add(prod);
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_low(i);
            }
        }
        0x0F => {
            // VMACH
            for i in 0..8 {
                let prod = i64::from(vs_v[i] as i16) * i64::from(vt_v[i] as i16);
                let acc = rsp.vu.acc48(i).wrapping_add(prod << 16);
                rsp.vu.set_acc48(i, acc);
                out[i] = rsp.vu.clamp_acc_signed(i);
            }
        }
        0x10 => {
            // VADD
            for i in 0..8 {
                let carry = i32::from((rsp.vu.vco >> i) & 1);
                let sum = i32::from(vs_v[i] as i16) + i32::from(vt_v[i] as i16) + carry;
                rsp.vu.acc_lo[i] = sum as u16;
                out[i] = sum.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16 as u16;
            }
            rsp.vu.vco = 0;
        }
        0x11 => {
            // VSUB
            for i in 0..8 {
                let borrow = i32::from((rsp.vu.vco >> i) & 1);
                let diff = i32::from(vs_v[i] as i16) - i32::from(vt_v[i] as i16) - borrow;
                rsp.vu.acc_lo[i] = diff as u16;
                out[i] = diff.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16 as u16;
            }
            rsp.vu.vco = 0;
        }
        0x13 => {
            // VABS
            for i in 0..8 {
                let s = vs_v[i] as i16;
                let t = vt_v[i] as i16;
                if s < 0 {
                    if t == i16::MIN {
                        rsp.vu.acc_lo[i] = 0x8000;
                        out[i] = 0x7FFF;
                    } else {
                        let r = (-t) as u16;
                        rsp.vu.acc_lo[i] = r;
                        out[i] = r;
                    }
                } else if s == 0 {
                    rsp.vu.acc_lo[i] = 0;
                    out[i] = 0;
                } else {
                    rsp.vu.acc_lo[i] = vt_v[i];
                    out[i] = vt_v[i];
                }
            }
        }
        0x14 => {
            // VADDC
            let mut new_vco = 0u16;
            for i in 0..8 {
                let sum = u32::from(vs_v[i]) + u32::from(vt_v[i]);
                rsp.vu.acc_lo[i] = sum as u16;
                out[i] = sum as u16;
                if sum > 0xFFFF {
                    new_vco |= 1 << i;
                }
            }
            rsp.vu.vco = new_vco;
        }
        0x15 => {
            // VSUBC
            let mut new_vco = 0u16;
            for i in 0..8 {
                let diff = i32::from(vs_v[i]) - i32::from(vt_v[i]);
                rsp.vu.acc_lo[i] = diff as u16;
                out[i] = diff as u16;
                if diff < 0 {
                    new_vco |= 1 << i;
                }
                if diff != 0 {
                    new_vco |= 1 << (i + 8);
                }
            }
            rsp.vu.vco = new_vco;
        }
        0x1D => {
            // VSAW
            out = match e {
                8 => rsp.vu.acc_hi,
                9 => rsp.vu.acc_md,
                10 => rsp.vu.acc_lo,
                _ => [0u16; 8],
            };
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc: u64::from(pc),
                word,
            })
        }
    }

    rsp.vu.vpr[vd] = out;
    Ok(())
}
