// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Deterministic PCM ring buffer, zero-float 32.32 fixed-point resampler, and WAV encoder.
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md · docs/13-debugger-and-ui-architecture.md

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const DEFAULT_PCM_RING_CAPACITY: usize = 16_384;
pub const FNV1A_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
pub const FNV1A_PRIME: u64 = 0x0000_0100_0000_01b3;

#[must_use]
pub fn fnv1a_pcm_i16(samples: &[i16]) -> u64 {
    let mut hash = FNV1A_OFFSET_BASIS;
    for &s in samples {
        let bytes = s.to_le_bytes();
        for &b in &bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(FNV1A_PRIME);
        }
    }
    hash
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTelemetry {
    pub peak_left: u16,
    pub peak_right: u16,
    pub clipped_samples: u64,
    pub total_frames_produced: u64,
    pub dropped_frames: u64,
    pub underrun_count: u64,
    pub pcm_hash: u64,
}

impl Default for AudioTelemetry {
    fn default() -> Self {
        Self {
            peak_left: 0,
            peak_right: 0,
            clipped_samples: 0,
            total_frames_produced: 0,
            dropped_frames: 0,
            underrun_count: 0,
            pcm_hash: FNV1A_OFFSET_BASIS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRingBuffer {
    buffer: VecDeque<(i16, i16)>,
    capacity: usize,
    telemetry: AudioTelemetry,
}

impl Default for AudioRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRingBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_PCM_RING_CAPACITY)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            buffer: VecDeque::with_capacity(cap),
            capacity: cap,
            telemetry: AudioTelemetry::default(),
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.telemetry = AudioTelemetry::default();
    }

    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }

    pub fn reset_peaks(&mut self) {
        self.telemetry.peak_left = 0;
        self.telemetry.peak_right = 0;
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub const fn telemetry(&self) -> AudioTelemetry {
        self.telemetry
    }

    #[must_use]
    pub const fn pcm_hash(&self) -> u64 {
        self.telemetry.pcm_hash
    }

    pub fn record_underrun(&mut self) {
        self.telemetry.underrun_count = self.telemetry.underrun_count.wrapping_add(1);
    }

    pub fn push_frame(&mut self, left: i16, right: i16) {
        // Fold bytes into pcm_hash before capacity check
        let lb = left.to_le_bytes();
        let rb = right.to_le_bytes();
        let mut h = self.telemetry.pcm_hash;
        for &b in &lb {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV1A_PRIME);
        }
        for &b in &rb {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV1A_PRIME);
        }
        self.telemetry.pcm_hash = h;

        // Update peaks
        let pl = i32::from(left).unsigned_abs() as u16;
        let pr = i32::from(right).unsigned_abs() as u16;
        if pl > self.telemetry.peak_left {
            self.telemetry.peak_left = pl;
        }
        if pr > self.telemetry.peak_right {
            self.telemetry.peak_right = pr;
        }

        // Clipped samples
        if left == i16::MIN || left == i16::MAX {
            self.telemetry.clipped_samples = self.telemetry.clipped_samples.wrapping_add(1);
        }
        if right == i16::MIN || right == i16::MAX {
            self.telemetry.clipped_samples = self.telemetry.clipped_samples.wrapping_add(1);
        }

        self.telemetry.total_frames_produced = self.telemetry.total_frames_produced.wrapping_add(1);

        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
            self.telemetry.dropped_frames = self.telemetry.dropped_frames.wrapping_add(1);
        }

        self.buffer.push_back((left, right));
    }

    pub fn drain_frames(&mut self, out: &mut Vec<(i16, i16)>) -> usize {
        let count = self.buffer.len();
        out.extend(self.buffer.drain(..));
        count
    }

    pub fn drain_interleaved_i16(&mut self, out: &mut Vec<i16>) -> usize {
        let count = self.buffer.len();
        out.reserve(count * 2);
        for (l, r) in self.buffer.drain(..) {
            out.push(l);
            out.push(r);
        }
        count
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PcmResampler {
    pub prev_frame: (i16, i16),
    pub has_prev: bool,
    pub phase: u64,
}

impl PcmResampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.prev_frame = (0, 0);
        self.has_prev = false;
        self.phase = 0;
    }

    pub fn resample(
        &mut self,
        src_frames: &[(i16, i16)],
        src_rate_hz: u32,
        dst_rate_hz: u32,
        out: &mut Vec<(i16, i16)>,
    ) -> usize {
        self.resample_with(src_frames, src_rate_hz, dst_rate_hz, |l, r| {
            out.push((l, r))
        })
    }

    pub fn resample_interleaved(
        &mut self,
        src_frames: &[(i16, i16)],
        src_rate_hz: u32,
        dst_rate_hz: u32,
        out: &mut Vec<i16>,
    ) -> usize {
        self.resample_with(src_frames, src_rate_hz, dst_rate_hz, |l, r| {
            out.push(l);
            out.push(r);
        })
    }

    fn resample_with(
        &mut self,
        src_frames: &[(i16, i16)],
        src_rate_hz: u32,
        dst_rate_hz: u32,
        mut emit: impl FnMut(i16, i16),
    ) -> usize {
        if src_frames.is_empty() || src_rate_hz == 0 || dst_rate_hz == 0 {
            return 0;
        }

        if src_rate_hz == dst_rate_hz {
            for &(l, r) in src_frames {
                emit(l, r);
            }
            self.prev_frame = *src_frames.last().expect("non-empty src_frames");
            self.has_prev = true;
            return src_frames.len();
        }

        // step = (src_rate << 32) / dst_rate using u128 (zero float)
        let step = ((u128::from(src_rate_hz) << 32) / u128::from(dst_rate_hz)) as u64;
        let mut produced = 0;

        for &cur in src_frames {
            if !self.has_prev {
                self.prev_frame = cur;
                self.has_prev = true;
                self.phase = 0;
                continue;
            }

            let (p_l, p_r) = self.prev_frame;
            let (c_l, c_r) = cur;

            while self.phase < (1_u64 << 32) {
                let l = i64::from(p_l)
                    + (((i64::from(c_l) - i64::from(p_l)) * (self.phase as i64)) >> 32);
                let r = i64::from(p_r)
                    + (((i64::from(c_r) - i64::from(p_r)) * (self.phase as i64)) >> 32);

                let clamped_l = l.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
                let clamped_r = r.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;

                emit(clamped_l, clamped_r);
                produced += 1;

                self.phase = self.phase.wrapping_add(step);
            }

            self.phase = self.phase.wrapping_sub(1_u64 << 32);
            self.prev_frame = cur;
        }

        produced
    }
}

#[must_use]
pub fn encode_wav_pcm16_stereo(samples: &[i16], sample_rate_hz: u32) -> Option<Vec<u8>> {
    if !samples.len().is_multiple_of(2) || sample_rate_hz == 0 {
        return None;
    }

    let num_samples = samples.len() as u32;
    let data_bytes = num_samples.checked_mul(2)?; // 2 bytes per i16
    let riff_chunk_size = 36u32.checked_add(data_bytes)?;

    let mut wav = Vec::with_capacity(44 + data_bytes as usize);

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt subchunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
    wav.extend_from_slice(&2u16.to_le_bytes()); // NumChannels (2 = Stereo)
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    let byte_rate = sample_rate_hz.checked_mul(4)?;
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes()); // BlockAlign (4 bytes per frame)
    wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample (16)

    // data subchunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());

    for &s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }

    Some(wav)
}

#[cfg(test)]
#[path = "pcm_tests.rs"]
mod tests;
