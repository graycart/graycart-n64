//! Peripheral Interface (PI) cartridge DMA implementation (`0x0460_0000..=0x046F_FFFF`).
//!
//! Cited: N64brew — Peripheral Interface
//!   https://n64brew.dev/wiki/Peripheral_Interface · docs/02-bus-memory-dma.md §6
//! Note: M2 implements immediate-data / delayed-MI-interrupt PI DMA per §3.3.

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiAction {
    None,
    StartCartToRdram {
        dram_addr: u32,
        cart_addr: u32,
        len: u32,
        delay_cycles: u64,
    },
    StartRdramToCart {
        dram_addr: u32,
        cart_addr: u32,
        len: u32,
        delay_cycles: u64,
    },
    ClearIntr,
    Reset,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pi {
    pub dram_addr: u32,
    pub cart_addr: u32,
    pub rd_len: u32,
    pub wr_len: u32,
    pub status: u32,
    pub bsd_dom1_lat: u32,
    pub bsd_dom1_pwd: u32,
    pub bsd_dom1_pgs: u32,
    pub bsd_dom1_rls: u32,
    pub bsd_dom2_lat: u32,
    pub bsd_dom2_pwd: u32,
    pub bsd_dom2_pgs: u32,
    pub bsd_dom2_rls: u32,
    pub dma_bytes_total: u64,
    pub dma_count: u64,
}

impl Pi {
    pub const DMA_BUSY: u32 = 1 << 0;
    pub const IO_BUSY: u32 = 1 << 1;
    pub const ERROR: u32 = 1 << 2;
    pub const INTR: u32 = 1 << 3;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 => Ok(self.dram_addr),
            0x04 => Ok(self.cart_addr),
            0x08 => Ok(self.rd_len),
            0x0C => Ok(self.wr_len),
            0x10 => Ok(self.status & 0x0F),
            0x14 => Ok(self.bsd_dom1_lat & 0xFF),
            0x18 => Ok(self.bsd_dom1_pwd & 0xFF),
            0x1C => Ok(self.bsd_dom1_pgs & 0x0F),
            0x20 => Ok(self.bsd_dom1_rls & 0x03),
            0x24 => Ok(self.bsd_dom2_lat & 0xFF),
            0x28 => Ok(self.bsd_dom2_pwd & 0xFF),
            0x2C => Ok(self.bsd_dom2_pgs & 0x0F),
            0x30 => Ok(self.bsd_dom2_rls & 0x03),
            _ => Err(Trap::UnimplementedRegister {
                device: "PI",
                offset,
                is_write: false,
            }),
        }
    }

    pub fn write_reg_action(&mut self, offset: u32, value: u32) -> Result<PiAction, Trap> {
        match offset {
            0x00 => {
                self.dram_addr = value & 0x00FF_FFFE;
                Ok(PiAction::None)
            }
            0x04 => {
                self.cart_addr = value & 0xFFFF_FFFE;
                Ok(PiAction::None)
            }
            0x08 => {
                let len = (value & 0x00FF_FFFF) + 1;
                self.rd_len = value & 0x00FF_FFFF;
                self.status |= Self::DMA_BUSY;
                let delay_cycles = (u64::from(len) * 9).max(64);
                self.dma_bytes_total = self.dma_bytes_total.saturating_add(u64::from(len));
                self.dma_count = self.dma_count.saturating_add(1);
                Ok(PiAction::StartRdramToCart {
                    dram_addr: self.dram_addr,
                    cart_addr: self.cart_addr,
                    len,
                    delay_cycles,
                })
            }
            0x0C => {
                let len = (value & 0x00FF_FFFF) + 1;
                self.wr_len = value & 0x00FF_FFFF;
                self.status |= Self::DMA_BUSY;
                let delay_cycles = (u64::from(len) * 9).max(64);
                self.dma_bytes_total = self.dma_bytes_total.saturating_add(u64::from(len));
                self.dma_count = self.dma_count.saturating_add(1);
                Ok(PiAction::StartCartToRdram {
                    dram_addr: self.dram_addr,
                    cart_addr: self.cart_addr,
                    len,
                    delay_cycles,
                })
            }
            0x10 => {
                if (value & 1) != 0 {
                    self.status &= !(Self::DMA_BUSY | Self::IO_BUSY | Self::ERROR);
                }
                if (value & 2) != 0 {
                    self.status &= !Self::INTR;
                }
                if (value & 2) != 0 {
                    Ok(PiAction::ClearIntr)
                } else if (value & 1) != 0 {
                    Ok(PiAction::Reset)
                } else {
                    Ok(PiAction::None)
                }
            }
            0x14 => {
                self.bsd_dom1_lat = value & 0xFF;
                Ok(PiAction::None)
            }
            0x18 => {
                self.bsd_dom1_pwd = value & 0xFF;
                Ok(PiAction::None)
            }
            0x1C => {
                self.bsd_dom1_pgs = value & 0x0F;
                Ok(PiAction::None)
            }
            0x20 => {
                self.bsd_dom1_rls = value & 0x03;
                Ok(PiAction::None)
            }
            0x24 => {
                self.bsd_dom2_lat = value & 0xFF;
                Ok(PiAction::None)
            }
            0x28 => {
                self.bsd_dom2_pwd = value & 0xFF;
                Ok(PiAction::None)
            }
            0x2C => {
                self.bsd_dom2_pgs = value & 0x0F;
                Ok(PiAction::None)
            }
            0x30 => {
                self.bsd_dom2_rls = value & 0x03;
                Ok(PiAction::None)
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "PI",
                offset,
                is_write: true,
            }),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_reg_action(offset, value).map(|_| ())
    }

    pub fn complete_dma(&mut self, bytes_copied: u32) {
        self.status &= !Self::DMA_BUSY;
        self.status |= Self::INTR;
        let advanced = (bytes_copied + 1) & !1;
        self.dram_addr = (self.dram_addr.wrapping_add(advanced)) & 0x00FF_FFFE;
        self.cart_addr = (self.cart_addr.wrapping_add(advanced)) & 0xFFFF_FFFE;
    }
}
