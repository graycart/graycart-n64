//! MIPS Interface (MI) interrupt aggregation and mode registers (`0x0430_0000..=0x043F_FFFF`).
//!
//! Cited: N64brew — MIPS Interface
//!   https://n64brew.dev/wiki/MIPS_Interface · docs/02-bus-memory-dma.md §9

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

pub const MI_INTR_SP: u32 = 1 << 0;
pub const MI_INTR_SI: u32 = 1 << 1;
pub const MI_INTR_AI: u32 = 1 << 2;
pub const MI_INTR_VI: u32 = 1 << 3;
pub const MI_INTR_PI: u32 = 1 << 4;
pub const MI_INTR_DP: u32 = 1 << 5;

pub const MI_VERSION_RETAIL: u32 = 0x0202_0102;

/// MI interrupt and mode registers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mi {
    pub mode: u32,
    pub version: u32,
    pub intr: u32,
    pub intr_mask: u32,
}

impl Default for Mi {
    fn default() -> Self {
        Self {
            mode: 0,
            version: MI_VERSION_RETAIL,
            intr: 0,
            intr_mask: 0,
        }
    }
}

impl Mi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_intr(&mut self, bit: u32) {
        self.intr |= bit;
    }

    pub fn clear_intr(&mut self, bit: u32) {
        self.intr &= !bit;
    }

    #[must_use]
    pub fn irq_pending(&self) -> bool {
        (self.intr & self.intr_mask) != 0
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 => Ok(self.mode & 0x3FF),
            0x04 => Ok(self.version),
            0x08 => Ok(self.intr & 0x3F),
            0x0C => Ok(self.intr_mask & 0x3F),
            _ => Err(Trap::UnimplementedRegister {
                device: "MI",
                offset,
                is_write: false,
            }),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        match offset {
            0x00 => {
                // MI_MODE
                self.mode = (self.mode & !0x7F) | (value & 0x7F);
                // bit 7 clears init_mode (bit 7 of mode)
                if (value & (1 << 7)) != 0 {
                    self.mode &= !(1 << 7);
                }
                // bit 8 sets init_mode (bit 7 of mode)
                if (value & (1 << 8)) != 0 {
                    self.mode |= 1 << 7;
                }
                // bit 9 clears ebus_test (bit 8 of mode)
                if (value & (1 << 9)) != 0 {
                    self.mode &= !(1 << 8);
                }
                // bit 10 sets ebus_test (bit 8 of mode)
                if (value & (1 << 10)) != 0 {
                    self.mode |= 1 << 8;
                }
                // bit 11 clears MI_INTR_DP
                if (value & (1 << 11)) != 0 {
                    self.intr &= !MI_INTR_DP;
                }
                // bit 12 clears rdram_reg_mode (bit 9 of mode)
                if (value & (1 << 12)) != 0 {
                    self.mode &= !(1 << 9);
                }
                // bit 13 sets rdram_reg_mode (bit 9 of mode)
                if (value & (1 << 13)) != 0 {
                    self.mode |= 1 << 9;
                }
                Ok(())
            }
            0x04 => {
                // MI_VERSION is read-only
                Ok(())
            }
            0x08 => {
                // MI_INTR is read-only
                Ok(())
            }
            0x0C => {
                // MI_INTR_MASK: paired clear/set bits for all 6 lines
                for i in 0..6 {
                    if (value & (1 << (i * 2))) != 0 {
                        self.intr_mask &= !(1 << i);
                    }
                    if (value & (1 << (i * 2 + 1))) != 0 {
                        self.intr_mask |= 1 << i;
                    }
                }
                Ok(())
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "MI",
                offset,
                is_write: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
