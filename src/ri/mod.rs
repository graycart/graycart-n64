//! RDRAM Interface (RI) registers (`0x0470_0000..=0x047F_FFFF`).
//!
//! Cited: N64brew — RDRAM Interface
//!   https://n64brew.dev/wiki/RDRAM_Interface · docs/02-bus-memory-dma.md §5

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ri {
    pub mode: u32,
    pub config: u32,
    pub current_load: u32,
    pub select: u32,
    pub refresh: u32,
    pub latency: u32,
    pub rerror: u32,
    pub werror: u32,
}

impl Default for Ri {
    fn default() -> Self {
        Self {
            mode: 0x0E,
            config: 0x40,
            current_load: 0,
            select: 0x14, // CRITICAL n64-systemtest / libdragon IPL3 boot trap: must default to non-zero!
            refresh: 0x0006_3634,
            latency: 0x10,
            rerror: 0,
            werror: 0,
        }
    }
}

impl Ri {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 => Ok(self.mode),
            0x04 => Ok(self.config),
            0x08 => Ok(self.current_load),
            0x0C => Ok(self.select),
            0x10 => Ok(self.refresh),
            0x14 => Ok(self.latency),
            0x18 => Ok(self.rerror),
            0x1C => Ok(self.werror),
            _ => Err(Trap::UnimplementedRegister {
                device: "RI",
                offset,
                is_write: false,
            }),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        match offset {
            0x00 => {
                self.mode = value & 0xF;
                Ok(())
            }
            0x04 => {
                self.config = value & 0x7F;
                Ok(())
            }
            0x08 => {
                self.current_load = 0; // writing sets 0 or value? Spec says: "current_load (sets 0)"
                Ok(())
            }
            0x0C => {
                self.select = value & 0x1F;
                Ok(())
            }
            0x10 => {
                self.refresh = value & 0x0007_FFFF;
                Ok(())
            }
            0x14 => {
                self.latency = value & 0xF;
                Ok(())
            }
            0x18 => {
                // rerror is read-only
                Ok(())
            }
            0x1C => {
                // werror write clears rerror and werror
                self.rerror = 0;
                self.werror = 0;
                Ok(())
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "RI",
                offset,
                is_write: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
