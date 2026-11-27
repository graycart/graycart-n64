//! MIPS Interface (MI) interrupt aggregation shell (`0x0430_0000..=0x043F_FFFF`).
//!
//! Cited: N64brew — MIPS Interface
//!   https://n64brew.dev/wiki/MIPS_Interface · docs/02-bus-memory-dma.md §9

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

/// MI interrupt and mode register shell (full mask/status wiring lands in M2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mi {
    pub intr: u32,
    pub intr_mask: u32,
}

impl Mi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "MI",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "MI",
            offset,
            is_write: true,
        })
    }
}
