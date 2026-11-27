//! RDRAM Interface (RI) shell (`0x0470_0000..=0x047F_FFFF`).
//!
//! Cited: N64brew — RDRAM Interface
//!   https://n64brew.dev/wiki/RDRAM_Interface · docs/02-bus-memory-dma.md §5

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ri {
    /// Non-zero `RI_SELECT` (0x14) signals RDRAM is initialized (systemtest IPL3 boot trap).
    pub select: u32,
}

impl Ri {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "RI",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "RI",
            offset,
            is_write: true,
        })
    }
}
