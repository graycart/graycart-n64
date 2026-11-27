//! RSP Vector Unit reciprocal/inverse-sqrt ROM lookup instructions and COP2 scalar moves.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit & Reciprocal Tables)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

#[cfg(test)]
#[path = "vu_recip_tests.rs"]
mod tests;

use std::sync::OnceLock;

use crate::trap::Trap;

use super::InterpRsp;

/// Precomputed 512-entry integer Reciprocal ROM table (`[1.0, 2.0)`).
static RCP_ROM: OnceLock<[u16; 512]> = OnceLock::new();

/// Precomputed 512-entry integer Inverse Square Root ROM table (`0..256` even msb `[1, 2)`, `256..512` odd msb `[2, 4)`).
static RSQ_ROM: OnceLock<[u16; 512]> = OnceLock::new();

fn init_rcp_rom() -> [u16; 512] {
    let mut rom = [0u16; 512];
    rom[0] = 0xFFFF;
    for (i, entry) in rom.iter_mut().enumerate().skip(1) {
        let a = 512u64 + i as u64;
        let b = (1u64 << 34) / a;
        *entry = (((b + 1) >> 8) & 0xFFFF) as u16;
    }
    rom
}

/// Exact integer square root `floor(sqrt(n))` using Newton's method (zero floating point).
fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << ((128 - n.leading_zeros()).div_ceil(2));
    loop {
        let y = (x + n / x) >> 1;
        if y >= x {
            return x;
        }
        x = y;
    }
}

fn init_rsq_rom() -> [u16; 512] {
    let mut rom = [0u16; 512];
    // Entries 0..256: even msb, m in [1.0, 2.0) -> 2 / sqrt(m) in (1.4142, 2.0] mapped to 0x1_0000..=0x1_FFFF
    // Entries 256..512: odd msb, 2*m in [2.0, 4.0) -> 2 / sqrt(2*m) in (1.0, 1.4142] mapped to 0x1_0000..0x1_6A0A
    rom[0] = 0xFFFF;
    for (i, entry) in rom.iter_mut().enumerate().skip(1) {
        // Represent radicand as (rad / 256) where rad in 256..1024:
        //   i in 1..256:   rad = 256 + i       (m = rad / 256 in (1, 2))
        //   i in 256..512: rad = 512 + (i-256)*2 (2*m = rad / 256 in [2, 4))
        // Target mantissa M = floor(2^17 / sqrt(rad / 256)) = floor(2^21 / sqrt(rad))
        // Compute with 128-bit integer sqrt: sqrt(2^42 / rad) = floor(2^21 / sqrt(rad))
        let rad: u128 = if i < 256 {
            256 + i as u128
        } else {
            512 + ((i as u128 - 256) * 2)
        };
        let m = isqrt_u128((1u128 << 42) / rad);
        *entry = (m.saturating_sub(0x1_0000).min(0xFFFF)) as u16;
    }
    rom
}

#[inline]
pub(crate) fn compute_rcp(input: i32) -> u32 {
    let rcp_rom = RCP_ROM.get_or_init(init_rcp_rom);
    if input == 0 {
        return 0x7FFF_FFFF;
    }
    if input == -32768 {
        return 0xFFFF_0000;
    }
    if input == -1 {
        return 0x8000_0000;
    }
    let neg = input < 0;
    let data = if neg {
        input.wrapping_neg() as u32
    } else {
        input as u32
    };
    let msb = 31 - data.leading_zeros();
    let idx = if msb > 9 {
        (data >> (msb - 9)) & 0x1FF
    } else {
        (data << (9 - msb)) & 0x1FF
    } as usize;
    let mant = (0x1_0000u32 | u32::from(rcp_rom[idx])) << 14;
    let mut res = mant >> msb;
    if neg {
        res = !res;
    }
    res
}

#[inline]
pub(crate) fn compute_rsq(input: i32) -> u32 {
    let rsq_rom = RSQ_ROM.get_or_init(init_rsq_rom);
    if input == 0 {
        return 0x7FFF_FFFF;
    }
    if input == -1 {
        return 0x8000_0000;
    }
    let neg = input < 0;
    let data = if neg {
        input.wrapping_neg() as u32
    } else {
        input as u32
    };
    let msb = 31 - data.leading_zeros();
    let sub_idx = if msb > 8 {
        (data >> (msb - 8)) & 0xFF
    } else {
        (data << (8 - msb)) & 0xFF
    } as usize;
    let idx = if (msb & 1) == 0 {
        sub_idx
    } else {
        256 + sub_idx
    };
    let mant = (0x1_0000u32 | u32::from(rsq_rom[idx])) << 14;
    let mut res = mant >> (msb >> 1);
    if neg {
        res = !res;
    }
    res
}

/// Execute COP2 scalar/control transfers (`MFC2`, `CFC2`, `MTC2`, `CTC2`).
pub fn exec_cop2_move(rsp: &mut InterpRsp, pc: u16, word: u32) -> Result<(), Trap> {
    let rs = (word >> 21) & 0x1F;
    let rt = ((word >> 16) & 0x1F) as usize;
    let rd = ((word >> 11) & 0x1F) as usize;
    let e = ((word >> 7) & 0x0F) as usize;

    match rs {
        0x00 => {
            // MFC2 rt, rd[e] (wraps byte 15 -> 0)
            let hi = rsp.vu.get_byte(rd, e);
            let lo = rsp.vu.get_byte(rd, (e + 1) & 15);
            let val = (u16::from_be_bytes([hi, lo]) as i16) as i32 as u32;
            rsp.set_gpr(rt, val);
            Ok(())
        }
        0x02 => {
            // CFC2 rt, rd
            let val = match rd & 3 {
                0 => (rsp.vu.vco as i16) as i32 as u32,
                1 => (rsp.vu.vcc as i16) as i32 as u32,
                _ => u32::from(rsp.vu.vce),
            };
            rsp.set_gpr(rt, val);
            Ok(())
        }
        0x04 => {
            // MTC2 rt, rd[e] (hardware quirk: does NOT wrap byte 15 -> 0)
            let v = rsp.gpr(rt) as u16;
            rsp.vu.set_byte(rd, e, (v >> 8) as u8);
            if e < 15 {
                rsp.vu.set_byte(rd, e + 1, (v & 0xFF) as u8);
            }
            Ok(())
        }
        0x06 => {
            // CTC2 rt, rd
            let v = rsp.gpr(rt);
            match rd & 3 {
                0 => rsp.vu.vco = v as u16,
                1 => rsp.vu.vcc = v as u16,
                _ => rsp.vu.vce = (v & 0xFF) as u8,
            }
            Ok(())
        }
        _ => Err(Trap::UnimplementedOpcode {
            pc: u64::from(pc),
            word,
        }),
    }
}

/// Execute VU reciprocal and inverse-sqrt instructions (`VRCP`, `VRCPL`, `VRCPH`, `VRSQ`, `VRSQL`, `VRSQH`).
pub fn exec_vu_recip(rsp: &mut InterpRsp, pc: u16, word: u32) -> Result<(), Trap> {
    let e = ((word >> 21) & 0x0F) as usize;
    let vt = ((word >> 16) & 0x1F) as usize;
    let vs = ((word >> 11) & 0x1F) as usize;
    let vd = ((word >> 6) & 0x1F) as usize;
    let funct = word & 0x3F;

    let se = e & 7;
    let de = vs & 7;
    let vt_swiz = rsp.vu.get_vt_swizzled(vt, e);
    rsp.vu.acc_lo = vt_swiz;

    let src_lane = rsp.vu.vpr[vt][se];
    let input_16 = i32::from(src_lane as i16);

    match funct {
        0x30 => {
            // VRCP
            let div_out = compute_rcp(input_16);
            rsp.vu.div_out = div_out;
            rsp.vu.div_in_loaded = false;
            rsp.vu.vpr[vd][de] = (div_out & 0xFFFF) as u16;
            Ok(())
        }
        0x31 => {
            // VRCPL
            let full_in = if rsp.vu.div_in_loaded {
                ((i32::from(rsp.vu.div_in)) << 16) | i32::from(src_lane)
            } else {
                input_16
            };
            let div_out = compute_rcp(full_in);
            rsp.vu.div_out = div_out;
            rsp.vu.div_in_loaded = false;
            rsp.vu.vpr[vd][de] = (div_out & 0xFFFF) as u16;
            Ok(())
        }
        0x32 => {
            // VRCPH
            rsp.vu.div_in = src_lane as i16;
            rsp.vu.div_in_loaded = true;
            rsp.vu.vpr[vd][de] = (rsp.vu.div_out >> 16) as u16;
            Ok(())
        }
        0x34 => {
            // VRSQ
            let div_out = compute_rsq(input_16);
            rsp.vu.div_out = div_out;
            rsp.vu.div_in_loaded = false;
            rsp.vu.vpr[vd][de] = (div_out & 0xFFFF) as u16;
            Ok(())
        }
        0x35 => {
            // VRSQL
            let full_in = if rsp.vu.div_in_loaded {
                ((i32::from(rsp.vu.div_in)) << 16) | i32::from(src_lane)
            } else {
                input_16
            };
            let div_out = compute_rsq(full_in);
            rsp.vu.div_out = div_out;
            rsp.vu.div_in_loaded = false;
            rsp.vu.vpr[vd][de] = (div_out & 0xFFFF) as u16;
            Ok(())
        }
        0x36 => {
            // VRSQH
            rsp.vu.div_in = src_lane as i16;
            rsp.vu.div_in_loaded = true;
            rsp.vu.vpr[vd][de] = (rsp.vu.div_out >> 16) as u16;
            Ok(())
        }
        _ => Err(Trap::UnimplementedOpcode {
            pc: u64::from(pc),
            word,
        }),
    }
}
