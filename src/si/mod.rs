//! Serial Interface (SI) PIF/Joybus DMA shell (`0x0480_0000..=0x048F_FFFF`).
//!
//! Cited: N64brew — Serial Interface
//!   https://n64brew.dev/wiki/Serial_Interface · docs/02-bus-memory-dma.md §7

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Si {
    pub dram_addr: u32,
    pub status: u32,
}

impl Si {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "SI",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "SI",
            offset,
            is_write: true,
        })
    }
}
