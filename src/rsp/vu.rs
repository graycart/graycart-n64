//! Reality Signal Processor (RSP) Vector Unit (VU / COP2) register file and helpers.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md §2, §5

use serde::{Deserialize, Serialize};

/// RSP Vector Unit state (32×128-bit VPR, 8×48-bit accumulator, VCO/VCC/VCE, RCP/RSQ latches).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vu {
    /// 32 vector registers, each 8 big-endian 16-bit lanes (`lane 0` = MSB halfword).
    pub vpr: [[u16; 8]; 32],
    /// 48-bit accumulator high 16 bits (`bits 47..32`) per lane.
    pub acc_hi: [u16; 8],
    /// 48-bit accumulator middle 16 bits (`bits 31..16`) per lane.
    pub acc_md: [u16; 8],
    /// 48-bit accumulator low 16 bits (`bits 15..0`) per lane.
    pub acc_lo: [u16; 8],
    /// Vector Carry Out (`bits 0..7` = carry, `bits 8..15` = not-equal).
    pub vco: u16,
    /// Vector Compare Code (`bits 0..7` = LE / compare, `bits 8..15` = GE / clip-high).
    pub vcc: u16,
    /// Vector Clip Extension (`bits 0..7`).
    pub vce: u8,
    /// High 16-bit operand latch written by `VRCPH` / `VRSQH`.
    pub div_in: i16,
    /// True if `VRCPH` / `VRSQH` loaded `div_in` for a 32-bit `VRCPL` / `VRSQL` pair.
    pub div_in_loaded: bool,
    /// 32-bit reciprocal / inverse-sqrt result latch.
    pub div_out: u32,
}

impl Vu {
    #[must_use]
    pub fn new() -> Self {
        Self {
            vpr: [[0; 8]; 32],
            acc_hi: [0; 8],
            acc_md: [0; 8],
            acc_lo: [0; 8],
            vco: 0,
            vcc: 0,
            vce: 0,
            div_in: 0,
            div_in_loaded: false,
            div_out: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Map RSP 4-bit element broadcast specifier `e` (`0..16`) and lane `i` (`0..8`)
    /// to the source lane index (`0..8`).
    #[must_use]
    #[inline]
    pub const fn lane_elem(e: usize, i: usize) -> usize {
        match e & 0xF {
            0 | 1 => i & 7,
            2 | 3 => (i & 6) | (e & 1),
            4..=7 => (i & 4) | (e & 3),
            _ => e & 7,
        }
    }

    /// Read vector register `vt` with 4-bit element swizzle `e` applied across all 8 lanes.
    #[must_use]
    #[inline]
    pub fn get_vt_swizzled(&self, vt: usize, e: usize) -> [u16; 8] {
        let src = self.vpr[vt & 31];
        let mut out = [0u16; 8];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = src[Self::lane_elem(e, i)];
        }
        out
    }

    /// Read a single big-endian byte (`0..16`, wrapping mod 16) from vector register `vr`.
    #[must_use]
    #[inline]
    pub fn get_byte(&self, vr: usize, byte_idx: usize) -> u8 {
        let idx = byte_idx & 15;
        let half = self.vpr[vr & 31][idx >> 1];
        if (idx & 1) == 0 {
            (half >> 8) as u8
        } else {
            (half & 0xFF) as u8
        }
    }

    /// Write a single big-endian byte (`0..16`, wrapping mod 16) into vector register `vr`.
    #[inline]
    pub fn set_byte(&mut self, vr: usize, byte_idx: usize, val: u8) {
        let idx = byte_idx & 15;
        let lane = &mut self.vpr[vr & 31][idx >> 1];
        if (idx & 1) == 0 {
            *lane = (*lane & 0x00FF) | (u16::from(val) << 8);
        } else {
            *lane = (*lane & 0xFF00) | u16::from(val);
        }
    }

    /// Read the signed 48-bit accumulator value for `lane`, sign-extended to `i64`.
    #[must_use]
    #[inline]
    pub fn acc48(&self, lane: usize) -> i64 {
        let hi = i64::from(self.acc_hi[lane] as i16);
        let md = i64::from(self.acc_md[lane]);
        let lo = i64::from(self.acc_lo[lane]);
        (hi << 32) | (md << 16) | lo
    }

    /// Write the low 48 bits of `val` into `acc_hi/md/lo` for `lane`.
    #[inline]
    pub fn set_acc48(&mut self, lane: usize, val: i64) {
        let u = val as u64;
        self.acc_hi[lane] = ((u >> 32) & 0xFFFF) as u16;
        self.acc_md[lane] = ((u >> 16) & 0xFFFF) as u16;
        self.acc_lo[lane] = (u & 0xFFFF) as u16;
    }

    /// Clamp a signed `i64` to signed 16-bit `[-32768, 32767]`.
    #[must_use]
    #[inline]
    pub fn clamp_signed(val: i64) -> i16 {
        val.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
    }

    /// Clamp accumulator `(acc_hi, acc_md)` to signed 16-bit (`VMULF`, `VMACF`, `VMUDM`, `VMACM`, `VMUDH`, `VMACH`).
    #[must_use]
    #[inline]
    pub fn clamp_acc_signed(&self, lane: usize) -> u16 {
        let hi_md = (i32::from(self.acc_hi[lane] as i16) << 16) | i32::from(self.acc_md[lane]);
        hi_md.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16 as u16
    }

    /// Clamp accumulator `(acc_hi, acc_md)` to unsigned 16-bit (`VMULU`, `VMACU`).
    #[must_use]
    #[inline]
    pub fn clamp_acc_unsigned(&self, lane: usize) -> u16 {
        let hi = self.acc_hi[lane] as i16;
        if hi < 0 {
            0x0000
        } else if hi > 0 || (self.acc_md[lane] as i16) < 0 {
            0xFFFF
        } else {
            self.acc_md[lane]
        }
    }

    /// Clamp accumulator low 16 bits based on signed `(acc_hi, acc_md)` overflow (`VMUDL`, `VMACL`, `VMUDN`, `VMACN`).
    #[must_use]
    #[inline]
    pub fn clamp_acc_low(&self, lane: usize) -> u16 {
        let hi_md = (i32::from(self.acc_hi[lane] as i16) << 16) | i32::from(self.acc_md[lane]);
        if hi_md < i32::from(i16::MIN) {
            0x0000
        } else if hi_md > i32::from(i16::MAX) {
            0xFFFF
        } else {
            self.acc_lo[lane]
        }
    }
}

impl Default for Vu {
    fn default() -> Self {
        Self::new()
    }
}
