// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Audio Interface (AI) double-buffered DMA controller (`0x0450_0000..=0x045F_FFFF`).
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md · docs/DEVELOPMENT-PLAN.md §3.9

#[cfg(test)]
#[path = "dma_tests.rs"]
mod tests;

use crate::trap::Trap;
use serde::{Deserialize, Serialize};

pub const VI_CLOCK_NTSC: u64 = 48_681_812;
pub const VI_CLOCK_PAL: u64 = 49_656_530;
pub const VI_CLOCK_MPAL: u64 = 48_628_316;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AiTvStandard {
    #[default]
    Ntsc,
    Pal,
    Mpal,
}

impl AiTvStandard {
    #[must_use]
    pub const fn vi_clock_hz(self) -> u64 {
        match self {
            Self::Ntsc => VI_CLOCK_NTSC,
            Self::Pal => VI_CLOCK_PAL,
            Self::Mpal => VI_CLOCK_MPAL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AiDmaSlot {
    pub addr: u32,
    pub len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAction {
    None,
    StartDma {
        raise_intr: bool,
        delay_cycles: Option<u64>,
    },
    ScheduleDma {
        delay_cycles: u64,
    },
    ClearIntr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiBurstOutcome {
    pub frames: Option<[(i16, i16); 2]>,
    pub raise_intr: bool,
    pub next_delay_cycles: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiDma {
    pub dram_addr: u32,
    pub len: u32,
    pub control: u32,
    pub dacrate: u32,
    pub bitrate: u32,
    pub fifo: [AiDmaSlot; 2],
    pub fifo_count: u8,
    pub dma_count: u64,
    pub tv_standard: AiTvStandard,
    pub cycle_rem: u64,
    pub delayed_carry: bool,
}

impl AiDma {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub fn sample_rate_hz(&self) -> u32 {
        (self.tv_standard.vi_clock_hz() / u64::from(self.dacrate.saturating_add(1))) as u32
    }

    pub fn next_burst_cycles(&mut self) -> u64 {
        let num = 2u128
            * u128::from(crate::rcp::CPU_CLOCK_HZ_NTSC)
            * u128::from(self.dacrate.saturating_add(1))
            + u128::from(self.cycle_rem);
        let den = u128::from(self.tv_standard.vi_clock_hz());
        let cycles = (num / den) as u64;
        self.cycle_rem = (num % den) as u64;
        cycles.max(1)
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 | 0x04 | 0x08 | 0x10 | 0x14 => Ok(if self.fifo_count > 0 {
                self.fifo[0].len
            } else {
                0
            }),
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
                if len == 0 || self.fifo_count >= 2 {
                    Ok(AiAction::None)
                } else if self.fifo_count == 0 {
                    let mut addr = self.dram_addr;
                    if self.delayed_carry {
                        addr = addr.wrapping_add(0x2000) & 0x00FF_FFF8;
                        self.delayed_carry = false;
                    }
                    self.fifo[0] = AiDmaSlot { addr, len };
                    self.fifo_count = 1;
                    self.dma_count += 1;
                    let delay_cycles = ((self.control & 1) != 0).then(|| self.next_burst_cycles());
                    Ok(AiAction::StartDma {
                        raise_intr: true,
                        delay_cycles,
                    })
                } else {
                    self.fifo[1] = AiDmaSlot {
                        addr: self.dram_addr,
                        len,
                    };
                    self.fifo_count = 2;
                    self.dma_count += 1;
                    Ok(AiAction::None)
                }
            }
            0x08 => {
                let was_enabled = (self.control & 1) != 0;
                self.control = value & 1;
                let now_enabled = (self.control & 1) != 0;
                if !was_enabled && now_enabled && self.fifo_count >= 1 {
                    Ok(AiAction::ScheduleDma {
                        delay_cycles: self.next_burst_cycles(),
                    })
                } else {
                    Ok(AiAction::None)
                }
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

    pub fn step_burst(&mut self, rdram: &crate::bus::Rdram) -> AiBurstOutcome {
        if self.fifo_count == 0 || (self.control & 1) == 0 {
            return AiBurstOutcome {
                frames: None,
                raise_intr: false,
                next_delay_cycles: None,
            };
        }
        let addr = self.fifo[0].addr;
        let mut bytes = [0u8; 8];
        for (i, slot) in bytes.iter_mut().enumerate() {
            *slot = rdram.read_u8(addr.wrapping_add(i as u32));
        }

        let l0 = i16::from_be_bytes([bytes[0], bytes[1]]);
        let r0 = i16::from_be_bytes([bytes[2], bytes[3]]);
        let l1 = i16::from_be_bytes([bytes[4], bytes[5]]);
        let r1 = i16::from_be_bytes([bytes[6], bytes[7]]);

        let next_low = (addr & 0x1FFF) + 8;
        self.fifo[0].len = self.fifo[0].len.saturating_sub(8);

        if next_low == 0x2000 {
            if self.fifo[0].len > 0 {
                self.fifo[0].addr = addr.wrapping_add(8) & 0x00FF_FFF8;
            } else {
                self.fifo[0].addr = addr & 0x00FF_E000;
                self.delayed_carry = true;
            }
        } else {
            self.fifo[0].addr = (addr & 0x00FF_E000) | (next_low & 0x1FFF);
        }

        if self.fifo[0].len == 0 {
            if self.fifo_count == 2 {
                let mut next = self.fifo[1];
                if self.delayed_carry {
                    next.addr = next.addr.wrapping_add(0x2000) & 0x00FF_FFF8;
                    self.delayed_carry = false;
                }
                self.fifo[0] = next;
                self.fifo[1] = AiDmaSlot::default();
                self.fifo_count = 1;
                AiBurstOutcome {
                    frames: Some([(l0, r0), (l1, r1)]),
                    raise_intr: true,
                    next_delay_cycles: Some(self.next_burst_cycles()),
                }
            } else {
                self.fifo[0] = AiDmaSlot::default();
                self.fifo_count = 0;
                AiBurstOutcome {
                    frames: Some([(l0, r0), (l1, r1)]),
                    raise_intr: false,
                    next_delay_cycles: None,
                }
            }
        } else {
            AiBurstOutcome {
                frames: Some([(l0, r0), (l1, r1)]),
                raise_intr: false,
                next_delay_cycles: Some(self.next_burst_cycles()),
            }
        }
    }
}
