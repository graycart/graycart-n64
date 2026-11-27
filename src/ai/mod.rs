//! Audio Interface (AI) DMA shell (`0x0450_0000..=0x045F_FFFF`).
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md
//! Note: Register/DMA event skeleton wires into the scheduler in M2; full
//! double-buffered DMA and hardware quirks land in M7.

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ai {
    pub dacrate: u32,
    pub bitrate: u32,
}

impl Ai {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "AI",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "AI",
            offset,
            is_write: true,
        })
    }
}
