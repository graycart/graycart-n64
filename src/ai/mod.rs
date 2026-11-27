//! Audio Interface (AI) register file and 2-slot DMA event skeleton (`0x0450_0000..=0x045F_FFFF`).
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAction {
    None,
    StartDma { delay_cycles: u64 },
    ClearIntr,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ai {
    pub dram_addr: u32,
    pub len: u32,
    pub control: u32,
    pub dacrate: u32,
    pub bitrate: u32,
    pub fifo_addr: [u32; 2],
    pub fifo_len: [u32; 2],
    pub fifo_count: u8,
    pub dma_count: u64,
}

impl Ai {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 | 0x04 | 0x08 | 0x10 | 0x14 => Ok(self.fifo_len[0]),
            0x0C => {
                let full = u32::from(self.fifo_count >= 2);
                let busy = u32::from(self.fifo_count >= 1);
                let enable = u32::from((self.control & 1) != 0);
                Ok((full << 31) | (busy << 30) | (enable << 25) | (1 << 20) | full)
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "AI",
                offset,
                is_write: false,
            }),
        }
    }

    pub fn write_reg_action(&mut self, offset: u32, value: u32) -> Result<AiAction, Trap> {
        match offset {
            0x00 => {
                self.dram_addr = value & 0x00FF_FFF8;
                Ok(AiAction::None)
            }
            0x04 => {
                let len = value & 0x0003_FFF8;
                self.len = len;
                if len > 0 && self.fifo_count < 2 {
                    let idx = self.fifo_count as usize;
                    self.fifo_addr[idx] = self.dram_addr;
                    self.fifo_len[idx] = len;
                    self.fifo_count += 1;
                    self.dma_count += 1;
                    if self.fifo_count == 1 {
                        Ok(AiAction::StartDma {
                            delay_cycles: self.dma_cycles(len),
                        })
                    } else {
                        Ok(AiAction::None)
                    }
                } else {
                    Ok(AiAction::None)
                }
            }
            0x08 => {
                self.control = value & 1;
                Ok(AiAction::None)
            }
            0x0C => Ok(AiAction::ClearIntr),
            0x10 => {
                self.dacrate = value & 0x3FFF;
                Ok(AiAction::None)
            }
            0x14 => {
                self.bitrate = value & 0x0F;
                Ok(AiAction::None)
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "AI",
                offset,
                is_write: true,
            }),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_reg_action(offset, value).map(|_| ())
    }

    #[must_use]
    pub fn dma_cycles(&self, len: u32) -> u64 {
        let samples = (u64::from(len) / 4).max(1);
        let rate = 48_681_812u64 / u64::from(self.dacrate.saturating_add(1));
        ((samples * 93_750_000) / rate.max(1)).max(1000)
    }

    pub fn on_dma_complete(&mut self) -> Option<u64> {
        if self.fifo_count == 2 {
            self.fifo_addr[0] = self.fifo_addr[1];
            self.fifo_len[0] = self.fifo_len[1];
            self.fifo_count = 1;
            Some(self.dma_cycles(self.fifo_len[0]))
        } else if self.fifo_count == 1 {
            self.fifo_count = 0;
            None
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
