// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Audio Interface (AI) subsystem composition (`0x0450_0000..=0x045F_FFFF`).
//!
//! Combines double-buffered AI DMA (`dma`), deterministic PCM ring buffer +
//! zero-float 32.32 fixed-point resampler (`pcm`), and bespoke LLE RSP audio
//! mixer microcode builders/oracles (`rsp_mixer`).
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md

pub mod dma;
pub mod pcm;
pub mod rsp_mixer;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

pub use dma::{
    AiAction, AiBurstOutcome, AiDma, AiDmaSlot, AiTvStandard, VI_CLOCK_MPAL, VI_CLOCK_NTSC,
    VI_CLOCK_PAL,
};
pub use pcm::{
    encode_wav_pcm16_stereo, fnv1a_pcm_i16, AudioRingBuffer, AudioTelemetry, PcmResampler,
    DEFAULT_PCM_RING_CAPACITY,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ai {
    pub dma: AiDma,
    pub pcm: AudioRingBuffer,
    pub resampler: PcmResampler,
}

impl Ai {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.dma.reset();
        self.pcm.reset();
        self.resampler.reset();
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        self.dma.read_reg(offset)
    }

    pub fn write_reg_action(&mut self, offset: u32, value: u32) -> Result<AiAction, Trap> {
        self.dma.write_reg_action(offset, value)
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_reg_action(offset, value).map(|_| ())
    }

    pub fn step_burst(&mut self, rdram: &Rdram) -> AiBurstOutcome {
        let out = self.dma.step_burst(rdram);
        if let Some([(l0, r0), (l1, r1)]) = out.frames {
            self.pcm.push_frame(l0, r0);
            self.pcm.push_frame(l1, r1);
        }
        out
    }

    #[must_use]
    pub fn sample_rate_hz(&self) -> u32 {
        self.dma.sample_rate_hz()
    }

    #[must_use]
    pub const fn pcm_hash(&self) -> u64 {
        self.pcm.pcm_hash()
    }

    #[must_use]
    pub const fn telemetry(&self) -> AudioTelemetry {
        self.pcm.telemetry()
    }

    pub fn drain_pcm_i16(&mut self, out: &mut Vec<i16>) -> usize {
        self.pcm.drain_interleaved_i16(out)
    }

    pub fn drain_pcm_resampled_i16(&mut self, host_rate_hz: u32, out: &mut Vec<i16>) -> usize {
        let mut frames = Vec::with_capacity(self.pcm.len());
        self.pcm.drain_frames(&mut frames);
        self.resampler
            .resample_interleaved(&frames, self.dma.sample_rate_hz(), host_rate_hz, out)
    }
}
