//! Peripheral Interface (PI) cartridge DMA shell (`0x0460_0000..=0x046F_FFFF`).
//!
//! Cited: N64brew — Peripheral Interface
//!   https://n64brew.dev/wiki/Peripheral_Interface · docs/02-bus-memory-dma.md §6
//! Note: M2 implements immediate-data / delayed-MI-interrupt PI DMA per §3.3.

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pi {
    pub dram_addr: u32,
    pub cart_addr: u32,
    pub status: u32,
}

impl Pi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "PI",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "PI",
            offset,
            is_write: true,
        })
    }
}
