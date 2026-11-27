//! Video Interface (VI) register file, scanline timing, and RGBA8888 scanout
//! (`0x0440_0000..=0x044F_FFFF`).
//!
//! Cited: N64brew — Video Interface
//!   https://n64brew.dev/wiki/Video_Interface · docs/04-rdp.md
//!
//! `VI_CTRL` TYPE[1:0] selects the framebuffer format consumed by
//! [`Vi::scanout_rgba8888`]:
//! - `00` — VI off. The scanout is a 0×0 frame (no pixels fetched).
//! - `01` — reserved. Scanout traps.
//! - `10` — 16-bit. RDRAM stores a big-endian 5/5/5/1 halfword
//!   (`RRRR RGGG GGBB BBBA`). n64brew calls the mode 5/5/5/3 because the
//!   analog path also folds in two RDRAM 9th bits; this unfiltered scanout
//!   does not. 5-bit channels expand by replication `(v << 3) | (v >> 2)`.
//!   The alpha bit expands to `0x00` or `0xFF`.
//! - `11` — 32-bit big-endian `R,G,B,A`, copied through.
//!
//! Width is `VI_WIDTH`. Height is the inclusive source-row span of the
//! vertical accumulator: it starts at `Y_OFFSET` (0.10) and adds `Y_SCALE`
//! (2.10, the reciprocal of the scale-up) once per active output line.
//! Active lines are `(V_END - V_START) / 2` half-lines from `VI_V_VIDEO`.
//! `Y_SCALE = 0x400` is 1:1 (one source row per output line).
//!
//! Gamma, gamma dither, divot, dedither, and any `AA_MODE` other than
//! REPLICATE (3) trap. They are not applied, so a host cannot treat this
//! buffer as a filtered television picture. `SERRATE` does not weave fields;
//! field selection on hardware is an `VI_ORIGIN` change, and this reads the
//! bytes at the current origin in storage order.

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViAction {
    None,
    ClearIntr,
}

/// Framebuffer format selected by `VI_CTRL` TYPE[1:0].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ViColorType {
    /// TYPE `00`. VI is off.
    Off = 0,
    /// TYPE `10`. Big-endian 5/5/5/1 in RDRAM.
    Rgba16 = 2,
    /// TYPE `11`. Big-endian 8/8/8/8 in RDRAM.
    Rgba32 = 3,
}

/// Unfiltered RGBA8888 picture produced by [`Vi::scanout_rgba8888`].
///
/// `rgba` is row-major, 4 bytes per pixel, order R, G, B, A.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8888Frame {
    pub width: u32,
    pub height: u32,
    pub color_type: ViColorType,
    pub rgba: Vec<u8>,
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

    /// `VI_CTRL` TYPE[1:0]. Reserved TYPE `01` traps.
    pub fn color_type(&self) -> Result<ViColorType, Trap> {
        match self.ctrl & 0b11 {
            0 => Ok(ViColorType::Off),
            2 => Ok(ViColorType::Rgba16),
            3 => Ok(ViColorType::Rgba32),
            _ => Err(Trap::UnimplementedRegister {
                device: "VI_CTRL.TYPE",
                offset: 1,
                is_write: false,
            }),
        }
    }

    /// Active output lines: `(V_END - V_START) / 2` half-lines from `VI_V_VIDEO`.
    #[must_use]
    pub fn active_output_lines(&self) -> u32 {
        let start = (self.v_video >> 16) & 0x3FF;
        let end = self.v_video & 0x3FF;
        end.saturating_sub(start) / 2
    }

    /// Source rows covered by the vertical scaler, including the last row landed on.
    ///
    /// The accumulator starts at `Y_OFFSET` (bits 25:16, 0.10) and adds
    /// `Y_SCALE` (bits 11:0, 2.10) once per active output line. The integer
    /// part (`>> 10`) is the framebuffer row.
    #[must_use]
    pub fn scanout_height(&self) -> u32 {
        let lines = self.active_output_lines();
        if lines == 0 {
            return 0;
        }
        let y_offset = (self.y_scale >> 16) & 0x3FF;
        let y_step = u64::from(self.y_scale & 0xFFF);
        let last = u64::from(y_offset) + u64::from(lines - 1) * y_step;
        u32::try_from(last / 1024)
            .unwrap_or(u32::MAX)
            .saturating_add(1)
    }

    /// Expand the RDRAM framebuffer to tightly packed RGBA8888.
    ///
    /// See the module docs for TYPE selection, the row count, and which
    /// `VI_CTRL` filter bits trap. `VI_ORIGIN` must be 8-byte aligned when
    /// any pixel is read, and the row stride must be a multiple of 8 bytes
    /// (16-bit width multiple of 4, 32-bit width multiple of 2). Bytes past
    /// installed RDRAM read as 0.
    pub fn scanout_rgba8888(&self, rdram: &Rdram) -> Result<Rgba8888Frame, Trap> {
        let color_type = self.color_type()?;
        if color_type == ViColorType::Off {
            return Ok(Rgba8888Frame {
                width: 0,
                height: 0,
                color_type,
                rgba: Vec::new(),
            });
        }
        if unsupported_vi_filter(self.ctrl) {
            return Err(Trap::UnimplementedRegister {
                device: "VI_SCANOUT",
                offset: self.ctrl & 0x0001_FFFF,
                is_write: false,
            });
        }
        let width = self.width & 0x0FFF;
        let height = self.scanout_height();
        let bpp: u32 = if color_type == ViColorType::Rgba32 {
            4
        } else {
            2
        };
        let mut rgba = Vec::new();
        if width == 0 || height == 0 {
            return Ok(Rgba8888Frame {
                width,
                height,
                color_type,
                rgba,
            });
        }
        let origin = self.origin & 0x00FF_FFFF;
        let stride = width.saturating_mul(bpp);
        if origin & 7 != 0 || stride & 7 != 0 {
            return Err(Trap::MisalignedAccess {
                pa: origin,
                size: 8,
            });
        }
        let bytes = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        rgba.reserve(bytes);
        for y in 0..height {
            let row = origin.saturating_add(y.saturating_mul(stride));
            for x in 0..width {
                let addr = row.saturating_add(x.saturating_mul(bpp));
                if color_type == ViColorType::Rgba32 {
                    let pix = rdram.read_u32(addr).unwrap_or(0);
                    rgba.extend_from_slice(&pix.to_be_bytes());
                } else {
                    let pix = rdram.read_u16(addr).unwrap_or(0);
                    rgba.extend_from_slice(&expand_5551(pix));
                }
            }
        }
        Ok(Rgba8888Frame {
            width,
            height,
            color_type,
            rgba,
        })
    }

    /// FNV-1a 64 of [`Self::scanout_rgba8888`], prefixed with width, height, and TYPE.
    ///
    /// Distinct from [`Self::framebuffer_hash`], which hashes raw RDRAM bytes.
    pub fn scanout_frame_hash(&self, rdram: &Rdram) -> Result<u64, Trap> {
        Ok(hash_rgba8888_frame(&self.scanout_rgba8888(rdram)?))
    }
}

/// Gamma, gamma-dither, divot, dedither, or an `AA_MODE` other than REPLICATE.
fn unsupported_vi_filter(ctrl: u32) -> bool {
    const GAMMA_DITHER: u32 = 1 << 2;
    const GAMMA: u32 = 1 << 3;
    const DIVOT: u32 = 1 << 4;
    const DEDITHER: u32 = 1 << 16;
    let filters = ctrl & (GAMMA_DITHER | GAMMA | DIVOT | DEDITHER);
    let aa_mode = (ctrl >> 8) & 0b11;
    filters != 0 || aa_mode != 0b11
}

fn expand_5551(pix: u16) -> [u8; 4] {
    let expand5 = |v: u16| -> u8 {
        let v = (v & 0x1F) as u8;
        (v << 3) | (v >> 2)
    };
    let a = if pix & 1 == 0 { 0 } else { 0xFF };
    [expand5(pix >> 11), expand5(pix >> 6), expand5(pix >> 1), a]
}

/// FNV-1a 64 over `b"GCVI"`, little-endian width, little-endian height, TYPE, then pixels.
#[must_use]
pub fn hash_rgba8888_frame(frame: &Rgba8888Frame) -> u64 {
    const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut hash = OFFSET;
    let mut mix = |b: u8| {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(PRIME);
    };
    for b in b"GCVI" {
        mix(*b);
    }
    for b in frame.width.to_le_bytes() {
        mix(b);
    }
    for b in frame.height.to_le_bytes() {
        mix(b);
    }
    mix(frame.color_type as u8);
    for &b in &frame.rgba {
        mix(b);
    }
    hash
}

#[cfg(test)]
mod tests;
