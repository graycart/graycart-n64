//! Reality Signal Processor (RSP) Vector Unit Compare, Clip, Merge, Logic, and Move (COP2).
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

#[cfg(test)]
#[path = "vu_logic_cmp_tests.rs"]
mod tests;

use crate::rsp::InterpRsp;
use crate::trap::Trap;

/// Execute an RSP COP2 Vector Unit Compare, Clip, Merge, Logic, or Move instruction.
pub fn exec_vu_logic_cmp(rsp: &mut InterpRsp, pc: u16, word: u32) -> Result<(), Trap> {
    let e = ((word >> 21) & 0x0F) as usize;
    let vt = ((word >> 16) & 0x1F) as usize;
    let vs = ((word >> 11) & 0x1F) as usize;
    let vd = ((word >> 6) & 0x1F) as usize;
    let funct = word & 0x3F;

    let vs_v = rsp.vu.vpr[vs];
    let vt_v = rsp.vu.get_vt_swizzled(vt, e);
    let mut out = [0u16; 8];
    let mut new_vcc = 0u16;

    match funct {
        0x20 => {
            // VLT (Less Than)
            for i in 0..8 {
                let eq = vs_v[i] == vt_v[i];
                let carry = ((rsp.vu.vco >> i) & 1) != 0;
                let ne = ((rsp.vu.vco >> (i + 8)) & 1) != 0;
                let cond = (vs_v[i] as i16) < (vt_v[i] as i16) || (eq && ne && carry);
                let res = if cond { vs_v[i] } else { vt_v[i] };
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
                if cond {
                    new_vcc |= 1 << i;
                }
            }
            rsp.vu.vcc = new_vcc;
            rsp.vu.vco = 0;
        }
        0x21 => {
            // VEQ (Equal)
            for i in 0..8 {
                let ne = ((rsp.vu.vco >> (i + 8)) & 1) != 0;
                let cond = vs_v[i] == vt_v[i] && !ne;
                let res = if cond { vs_v[i] } else { vt_v[i] };
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
                if cond {
                    new_vcc |= 1 << i;
                }
            }
            rsp.vu.vcc = new_vcc;
            rsp.vu.vco = 0;
        }
        0x22 => {
            // VNE (Not Equal)
            for i in 0..8 {
                let ne = ((rsp.vu.vco >> (i + 8)) & 1) != 0;
                let cond = vs_v[i] != vt_v[i] || ne;
                let res = if cond { vs_v[i] } else { vt_v[i] };
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
                if cond {
                    new_vcc |= 1 << i;
                }
            }
            rsp.vu.vcc = new_vcc;
            rsp.vu.vco = 0;
        }
        0x23 => {
            // VGE (Greater or Equal)
            for i in 0..8 {
                let eq = vs_v[i] == vt_v[i];
                let carry = ((rsp.vu.vco >> i) & 1) != 0;
                let ne = ((rsp.vu.vco >> (i + 8)) & 1) != 0;
                let cond = (vs_v[i] as i16) > (vt_v[i] as i16) || (eq && !(ne && carry));
                let res = if cond { vs_v[i] } else { vt_v[i] };
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
                if cond {
                    new_vcc |= 1 << i;
                }
            }
            rsp.vu.vcc = new_vcc;
            rsp.vu.vco = 0;
        }
        0x24 => {
            // VCL (Vector Clip Test Low)
            let old_vco = rsp.vu.vco;
            let old_vcc = rsp.vu.vcc;
            let old_vce = rsp.vu.vce;
            let mut new_vcc = 0u16;
            for i in 0..8 {
                let mut le = ((old_vcc >> i) & 1) != 0;
                let mut ge = ((old_vcc >> (i + 8)) & 1) != 0;
                let sign = ((old_vco >> i) & 1) != 0;
                let eq = ((old_vco >> (i + 8)) & 1) == 0;
                let ce = ((old_vce >> i) & 1) != 0;
                let vs_u = vs_v[i];
                let vt_u = vt_v[i];
                let eff_vt = if sign { vt_u.wrapping_neg() } else { vt_u };
                if sign {
                    if eq {
                        let sum = u32::from(vs_u) + u32::from(vt_u);
                        let carry = sum > 0xFFFF;
                        let sum16 = sum as u16;
                        le = if ce {
                            sum16 == 0 || !carry
                        } else {
                            sum16 == 0 && !carry
                        };
                    }
                    let res = if le { eff_vt } else { vs_u };
                    rsp.vu.acc_lo[i] = res;
                    out[i] = res;
                } else {
                    if eq {
                        ge = vs_u >= vt_u;
                    }
                    let res = if ge { eff_vt } else { vs_u };
                    rsp.vu.acc_lo[i] = res;
                    out[i] = res;
                }
                if le {
                    new_vcc |= 1 << i;
                }
                if ge {
                    new_vcc |= 1 << (i + 8);
                }
            }
            rsp.vu.vcc = new_vcc;
            rsp.vu.vco = 0;
            rsp.vu.vce = 0;
        }
        0x25 => {
            // VCH (Vector Clip Test High)
            let mut new_vco = 0u16;
            let mut new_vcc = 0u16;
            let mut new_vce = 0u8;
            for i in 0..8 {
                let s = vs_v[i] as i16;
                let t = vt_v[i] as i16;
                let sign = (s ^ t) < 0;
                let diff = i32::from(s) + if sign { i32::from(t) } else { -i32::from(t) };
                let ce = sign && diff == -1;
                let eq = diff == 0 || ce;
                if sign {
                    let le = diff <= 0;
                    let ge = t < 0;
                    let res = if le {
                        (-(t as i32)) as i16 as u16
                    } else {
                        vs_v[i]
                    };
                    rsp.vu.acc_lo[i] = res;
                    out[i] = res;
                    if le {
                        new_vcc |= 1 << i;
                    }
                    if ge {
                        new_vcc |= 1 << (i + 8);
                    }
                } else {
                    let le = t < 0;
                    let ge = diff >= 0;
                    let res = if ge { vt_v[i] } else { vs_v[i] };
                    rsp.vu.acc_lo[i] = res;
                    out[i] = res;
                    if le {
                        new_vcc |= 1 << i;
                    }
                    if ge {
                        new_vcc |= 1 << (i + 8);
                    }
                }
                if sign {
                    new_vco |= 1 << i;
                }
                if !eq {
                    new_vco |= 1 << (i + 8);
                }
                if ce {
                    new_vce |= 1 << i;
                }
            }
            rsp.vu.vco = new_vco;
            rsp.vu.vcc = new_vcc;
            rsp.vu.vce = new_vce;
        }
        0x26 => {
            // VCR (Vector C-Register / single-precision 1's-complement clip)
            let mut new_vcc = 0u16;
            for i in 0..8 {
                let s = vs_v[i] as i16;
                let t = vt_v[i] as i16;
                let sign = (s ^ t) < 0;
                let (le, ge, res) = if sign {
                    let le = (i32::from(s) + i32::from(t)) < 0;
                    (le, t < 0, if le { !vt_v[i] } else { vs_v[i] })
                } else {
                    let ge = (i32::from(s) - i32::from(t)) >= 0;
                    (t < 0, ge, if ge { vt_v[i] } else { vs_v[i] })
                };
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
                if le {
                    new_vcc |= 1 << i;
                }
                if ge {
                    new_vcc |= 1 << (i + 8);
                }
            }
            rsp.vu.vcc = new_vcc;
            rsp.vu.vco = 0;
            rsp.vu.vce = 0;
        }
        0x27 => {
            // VMRG (Vector Merge using VCC low bits 0..7, clears VCO)
            for i in 0..8 {
                let sel = ((rsp.vu.vcc >> i) & 1) != 0;
                let res = if sel { vs_v[i] } else { vt_v[i] };
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
            rsp.vu.vco = 0;
        }
        0x28 => {
            // VAND
            for i in 0..8 {
                let res = vs_v[i] & vt_v[i];
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
        }
        0x29 => {
            // VNAND
            for i in 0..8 {
                let res = !(vs_v[i] & vt_v[i]);
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
        }
        0x2A => {
            // VOR
            for i in 0..8 {
                let res = vs_v[i] | vt_v[i];
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
        }
        0x2B => {
            // VNOR
            for i in 0..8 {
                let res = !(vs_v[i] | vt_v[i]);
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
        }
        0x2C => {
            // VXOR
            for i in 0..8 {
                let res = vs_v[i] ^ vt_v[i];
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
        }
        0x2D => {
            // VNXOR
            for i in 0..8 {
                let res = !(vs_v[i] ^ vt_v[i]);
                rsp.vu.acc_lo[i] = res;
                out[i] = res;
            }
        }
        0x33 => {
            // VMOV: writes all 8 lanes of acc_lo, and ONLY lane `vs & 7` of `vd`.
            rsp.vu.acc_lo = vt_v;
            let de = vs & 7;
            rsp.vu.vpr[vd][de] = vt_v[de];
            return Ok(());
        }
        0x37 | 0x3F => {
            // VNOP / VNULL
            return Ok(());
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc: u64::from(pc),
                word,
            });
        }
    }

    rsp.vu.vpr[vd] = out;
    Ok(())
}
