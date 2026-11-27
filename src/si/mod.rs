//! Serial Interface (SI) PIF/Joybus DMA shell (`0x0480_0000..=0x048F_FFFF`).
//!
//! Cited: N64brew — Serial Interface
//!   https://n64brew.dev/wiki/Serial_Interface · docs/02-bus-memory-dma.md §7

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

/// Actions returned by SI register writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiAction {
    None,
    DmaPifToRdram { dram_addr: u32, delay_cycles: u64 },
    DmaRdramToPif { dram_addr: u32, delay_cycles: u64 },
    ClearIntr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Si {
    pub dram_addr: u32,
    pub status: u32,
    pub dma_count: u64,
}

impl Si {
    pub const DMA_BUSY: u32 = 1 << 0;
    pub const IO_BUSY: u32 = 1 << 1;
    pub const INTR: u32 = 1 << 12;

    #[must_use]
    pub fn new() -> Self {
        Self {
            dram_addr: 0,
            status: 0,
            dma_count: 0,
        }
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 => Ok(self.dram_addr),
            0x18 => Ok(self.status),
            _ => Err(Trap::UnimplementedRegister {
                device: "SI",
                offset,
                is_write: false,
            }),
        }
    }

    pub fn write_reg_action(&mut self, offset: u32, value: u32) -> Result<SiAction, Trap> {
        match offset {
            0x00 => {
                self.dram_addr = value & 0x00FF_FFF8;
                Ok(SiAction::None)
            }
            0x04 => {
                self.status |= Self::DMA_BUSY;
                self.dma_count += 1;
                Ok(SiAction::DmaPifToRdram {
                    dram_addr: self.dram_addr,
                    delay_cycles: 4096,
                })
            }
            0x10 => {
                self.status |= Self::DMA_BUSY;
                self.dma_count += 1;
                Ok(SiAction::DmaRdramToPif {
                    dram_addr: self.dram_addr,
                    delay_cycles: 4096,
                })
            }
            0x18 => {
                self.status &= !Self::INTR;
                Ok(SiAction::ClearIntr)
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "SI",
                offset,
                is_write: true,
            }),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_reg_action(offset, value).map(|_| ())
    }

    pub fn complete_dma(&mut self) {
        self.status &= !Self::DMA_BUSY;
        self.status |= Self::INTR;
    }
}

impl Default for Si {
    fn default() -> Self {
        Self::new()
    }
}
