//! Video Interface (VI) scan-out shell (`0x0440_0000..=0x044F_FFFF`).
//!
//! Cited: N64brew — Video Interface
//!   https://n64brew.dev/wiki/Video_Interface · docs/04-rdp.md

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

/// Video Interface state shell (register-driven field period and pre/post-filter
/// scan-out land in M2/M5).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vi {
    pub origin: u32,
    pub width: u32,
    pub v_sync: u32,
    pub v_intr: u32,
    pub v_current: u32,
}

impl Vi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "VI",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "VI",
            offset,
            is_write: true,
        })
    }

    /// Deterministic FNV-1a 64-bit hash over the configured VI origin window (M0 stub).
    #[must_use]
    pub fn framebuffer_hash(&self, rdram: &Rdram) -> u64 {
        const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
        let mut hash = FNV_OFFSET;
        let base = (self.origin & 0x00FF_FFFF) as usize;
        let byte_len = if self.width == 0 {
            320 * 240 * 2
        } else {
            (self.width as usize).saturating_mul(240 * 2)
        };
        let end = base.saturating_add(byte_len).min(rdram.len());
        if base < end {
            for &b in &rdram.as_slice()[base..end] {
                hash ^= u64::from(b);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        hash
    }
}
