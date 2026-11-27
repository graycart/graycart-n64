//! Video Interface (VI) register file and register-driven scanline timing (`0x0440_0000..=0x044F_FFFF`).
//!
//! Cited: N64brew — Video Interface
//!   https://n64brew.dev/wiki/Video_Interface · docs/04-rdp.md

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViAction {
    None,
    ClearIntr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vi {
    pub ctrl: u32,
    pub origin: u32,
    pub width: u32,
    pub v_intr: u32,
    pub v_current: u32,
    pub burst: u32,
    pub v_sync: u32,
    pub h_sync: u32,
    pub h_sync_leap: u32,
    pub h_video: u32,
    pub v_video: u32,
    pub v_burst: u32,
    pub x_scale: u32,
    pub y_scale: u32,
    pub field: u32,
    pub frame_count: u64,
}

impl Default for Vi {
    fn default() -> Self {
        Self {
            ctrl: 0,
            origin: 0,
            width: 0,
            v_intr: 0x3FF,
            v_current: 0,
            burst: 0,
            v_sync: 525,
            h_sync: 0,
            h_sync_leap: 0,
            h_video: 0,
            v_video: 0,
            v_burst: 0,
            x_scale: 0,
            y_scale: 0,
            field: 0,
            frame_count: 0,
        }
    }
}

impl Vi {
    #[must_use]
    pub fn new() -> Self {
        Self {
            v_sync: 525,
            v_intr: 0x3FF,
            ..Default::default()
        }
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00 => Ok(self.ctrl & 0x0001_FFFF),
            0x04 => Ok(self.origin & 0x00FF_FFFF),
            0x08 => Ok(self.width & 0x0FFF),
            0x0C => Ok(self.v_intr & 0x03FF),
            0x10 => Ok(self.v_current & 0x03FF),
            0x14 => Ok(self.burst & 0x3FFF_FFFF),
            0x18 => Ok(self.v_sync & 0x03FF),
            0x1C => Ok(self.h_sync & 0x001F_0FFF),
            0x20 => Ok(self.h_sync_leap & 0x0FFF_0FFF),
            0x24 => Ok(self.h_video & 0x03FF_03FF),
            0x28 => Ok(self.v_video & 0x03FF_03FF),
            0x2C => Ok(self.v_burst & 0x03FF_03FF),
            0x30 => Ok(self.x_scale & 0x0FFF_0FFF),
            0x34 => Ok(self.y_scale & 0x0FFF_0FFF),
            _ => Err(Trap::UnimplementedRegister {
                device: "VI",
                offset,
                is_write: false,
            }),
        }
    }

    pub fn write_reg_action(&mut self, offset: u32, value: u32) -> Result<ViAction, Trap> {
        match offset {
            0x00 => {
                self.ctrl = value & 0x0001_FFFF;
                Ok(ViAction::None)
            }
            0x04 => {
                self.origin = value & 0x00FF_FFFF;
                Ok(ViAction::None)
            }
            0x08 => {
                self.width = value & 0x0FFF;
                Ok(ViAction::None)
            }
            0x0C => {
                self.v_intr = value & 0x03FF;
                Ok(ViAction::None)
            }
            0x10 => {
                // Writing v_current clears VI interrupt and does not overwrite v_current
                Ok(ViAction::ClearIntr)
            }
            0x14 => {
                self.burst = value & 0x3FFF_FFFF;
                Ok(ViAction::None)
            }
            0x18 => {
                self.v_sync = value & 0x03FF;
                Ok(ViAction::None)
            }
            0x1C => {
                self.h_sync = value & 0x001F_0FFF;
                Ok(ViAction::None)
            }
            0x20 => {
                self.h_sync_leap = value & 0x0FFF_0FFF;
                Ok(ViAction::None)
            }
            0x24 => {
                self.h_video = value & 0x03FF_03FF;
                Ok(ViAction::None)
            }
            0x28 => {
                self.v_video = value & 0x03FF_03FF;
                Ok(ViAction::None)
            }
            0x2C => {
                self.v_burst = value & 0x03FF_03FF;
                Ok(ViAction::None)
            }
            0x30 => {
                self.x_scale = value & 0x0FFF_0FFF;
                Ok(ViAction::None)
            }
            0x34 => {
                self.y_scale = value & 0x0FFF_0FFF;
                Ok(ViAction::None)
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "VI",
                offset,
                is_write: true,
            }),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_reg_action(offset, value).map(|_| ())
    }

    #[must_use]
    pub fn num_halflines(&self) -> u32 {
        let v = self.v_sync & 0x3FF;
        if v < 2 {
            525
        } else {
            v
        }
    }

    #[must_use]
    pub fn cycles_per_line(&self) -> u64 {
        let lines = u64::from((self.num_halflines() / 2).max(1));
        (1_562_500 / lines).max(100)
    }

    pub fn step_line(&mut self) -> (bool, bool) {
        let mut line = (self.v_current & !1) + 2;
        let max_hl = self.num_halflines() & !1;
        let mut frame_completed = false;
        if line >= max_hl {
            line = 0;
            self.frame_count += 1;
            if (self.ctrl & 0x40) != 0 {
                self.field ^= 1;
            } else {
                self.field = 0;
            }
            frame_completed = true;
        }
        self.v_current = line | (self.field & 1);
        let intr = (self.v_current & !1) == (self.v_intr & !1);
        (intr, frame_completed)
    }

    #[must_use]
    pub fn framebuffer_hash(&self, rdram: &Rdram) -> u64 {
        const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
        let mut hash = FNV_OFFSET;
        let base = (self.origin & 0x00FF_FFFF) as usize;
        let byte_len = if self.width == 0 {
            320 * 240 * 2
        } else {
            (self.width as usize).saturating_mul(240 * 2)
        };
        let end = base.saturating_add(byte_len).min(rdram.len());
        if base < end {
            for &b in &rdram.as_slice()[base..end] {
                hash ^= u64::from(b);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        hash
    }
}

#[cfg(test)]
mod tests;
