// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Bespoke LLE RSP audio mixer microcodes and bit-exact DSP oracles.
//!
//! Cited: N64brew — Reality Signal Processor (Vector Unit Audio Mixer & Resampler)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Vector_Unit · docs/03-rsp.md · docs/05-audio.md · docs/DEVELOPMENT-PLAN.md §3.9

#[cfg(test)]
#[path = "rsp_mixer_tests.rs"]
mod tests;

pub const MIXER_GAINS_DMEM: u32 = 0x0000;
pub const MIXER_VOICE_A_DMEM: u32 = 0x0100;
pub const MIXER_VOICE_B_DMEM: u32 = 0x0500;
pub const MIXER_OUT_DMEM: u32 = 0x0900;

const RSP_BREAK: u32 = 0x0000_000D;

#[inline]
const fn enc_addiu(rt: u32, rs: u32, imm: i16) -> u32 {
    (0x09 << 26) | (rs << 21) | (rt << 16) | ((imm as u16) as u32)
}

#[inline]
const fn enc_ori(rt: u32, rs: u32, imm: u16) -> u32 {
    (0x0D << 26) | (rs << 21) | (rt << 16) | (imm as u32)
}

#[inline]
const fn enc_lui(rt: u32, imm: u16) -> u32 {
    (0x0F << 26) | (rt << 16) | (imm as u32)
}

#[inline]
const fn enc_bne(rs: u32, rt: u32, rel_off: i16) -> u32 {
    (0x05 << 26) | (rs << 21) | (rt << 16) | ((rel_off as u16) as u32)
}

#[inline]
const fn enc_mtc0(rt: u32, rd: u32) -> u32 {
    (0x10 << 26) | (0x04 << 21) | (rt << 16) | (rd << 11)
}

#[inline]
const fn enc_lqv(vt: u32, e: u32, off7: i8, base: u32) -> u32 {
    (0x32 << 26) | (base << 21) | (vt << 16) | (0x04 << 11) | (e << 7) | ((off7 as u32) & 0x7F)
}

#[inline]
const fn enc_sqv(vt: u32, e: u32, off7: i8, base: u32) -> u32 {
    (0x3A << 26) | (base << 21) | (vt << 16) | (0x04 << 11) | (e << 7) | ((off7 as u32) & 0x7F)
}

#[inline]
const fn enc_ssv(vt: u32, e: u32, off7: i8, base: u32) -> u32 {
    (0x3A << 26) | (base << 21) | (vt << 16) | (0x01 << 11) | (e << 7) | ((off7 as u32) & 0x7F)
}

#[inline]
const fn enc_vu(e: u32, vt: u32, vs: u32, vd: u32, funct: u32) -> u32 {
    (0x12 << 26) | (1 << 25) | (e << 21) | (vt << 16) | (vs << 11) | (vd << 6) | funct
}

/// Input vectors for the RSP 16.16 phase accumulator + 48-bit linear predictor kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhasePredictorInput {
    pub phase_lo: [u16; 8],
    pub step_lo: [u16; 8],
    pub phase_hi: [u16; 8],
    pub step_hi: [u16; 8],
    pub s0: [u16; 8],
    pub c0: [u16; 8],
    pub s1: [u16; 8],
    pub c1: [u16; 8],
}

/// Bit-exact oracle outputs matching [`build_rsp_phase_and_predictor_ucode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhasePredictorOutput {
    pub phase_lo: [u16; 8],
    pub phase_hi: [u16; 8],
    pub vco: u16,
    pub pred_lo: [u16; 8],
    pub acc: [i64; 8],
}

/// Deterministic integer triangle/saw waveform generator (zero floats).
#[must_use]
pub fn generate_pcm16_test_wave(num_samples: usize, step: i32, amp: i16) -> Vec<i16> {
    let mut wave = Vec::with_capacity(num_samples);
    let mut phase: i32 = 0;
    for _ in 0..num_samples {
        let tri = i32::from((phase >> 16) as i16);
        let scaled = (tri * i32::from(amp)) / 32_767;
        wave.push(scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16);
        phase = phase.wrapping_add(step);
    }
    wave
}

/// Write big-endian gain `[gA_L, gA_R, gB_L, gB_R]` and mono Voice A/B buffers to RSP DMEM.
pub fn load_stereo_mixer_inputs_to_dmem(
    sp_dmem: &mut [u8],
    voice_a: &[i16],
    voice_b: &[i16],
    gains: [i16; 4],
) {
    for (i, &g) in gains.iter().enumerate() {
        let addr = (MIXER_GAINS_DMEM as usize) + i * 2;
        if addr + 1 < sp_dmem.len() {
            let bytes = g.to_be_bytes();
            sp_dmem[addr] = bytes[0];
            sp_dmem[addr + 1] = bytes[1];
        }
    }

    for (base, voice) in [
        (MIXER_VOICE_A_DMEM as usize, voice_a),
        (MIXER_VOICE_B_DMEM as usize, voice_b),
    ] {
        for (i, &sample) in voice.iter().enumerate() {
            let addr = base + i * 2;
            if addr + 1 < sp_dmem.len() {
                let bytes = sample.to_be_bytes();
                sp_dmem[addr] = bytes[0];
                sp_dmem[addr + 1] = bytes[1];
            }
        }
    }
}

/// Upload big-endian 32-bit RSP instruction words into SP IMEM.
pub fn upload_rsp_ucode(sp_imem: &mut [u8], words: &[u32]) {
    for (i, &word) in words.iter().enumerate() {
        let addr = i * 4;
        if addr + 3 < sp_imem.len() {
            sp_imem[addr..addr + 4].copy_from_slice(&word.to_be_bytes());
        }
    }
}

/// Build Multi-Block 2-Voice Stereo Q15 Gain Mixer + SP DMA Writeback microcode.
#[must_use]
pub fn build_rsp_stereo_mixer_ucode(num_blocks: u16, dram_dst: u32) -> Vec<u32> {
    let mut ucode = Vec::new();
    let (r0, r1, r2, r3, r4, r8) = (0, 1, 2, 3, 4, 8);

    ucode.push(enc_addiu(r1, r0, MIXER_VOICE_A_DMEM as i16));
    ucode.push(enc_addiu(r2, r0, MIXER_VOICE_B_DMEM as i16));
    ucode.push(enc_addiu(r3, r0, MIXER_OUT_DMEM as i16));
    ucode.push(enc_ori(r4, r0, num_blocks.max(1)));
    ucode.push(enc_addiu(r8, r0, MIXER_GAINS_DMEM as i16));

    // Preload Q15 stereo gain vector [gA_L, gA_R, gB_L, gB_R, ...] into v3.
    ucode.push(enc_lqv(3, 0, 0, r8));

    let loop_pc_idx = ucode.len();
    ucode.push(enc_lqv(1, 0, 0, r1));
    ucode.push(enc_lqv(2, 0, 0, r2));

    // Left: v4 = VMULF(v1, v3[8]) + VMACF(v2, v3[10])
    ucode.push(enc_vu(8, 3, 1, 4, 0x00));
    ucode.push(enc_vu(10, 3, 2, 4, 0x08));

    // Right: v5 = VMULF(v1, v3[9]) + VMACF(v2, v3[11])
    ucode.push(enc_vu(9, 3, 1, 5, 0x00));
    ucode.push(enc_vu(11, 3, 2, 5, 0x08));

    for k in 0..8i8 {
        let off_l = k * 2;
        let off_r = k * 2 + 1;
        let elem = (k as u32) * 2;
        ucode.push(enc_ssv(4, elem, off_l, r3));
        ucode.push(enc_ssv(5, elem, off_r, r3));
    }

    ucode.push(enc_addiu(r1, r1, 16));
    ucode.push(enc_addiu(r2, r2, 16));
    ucode.push(enc_addiu(r4, r4, -1));

    let delay_slot_pc_idx = ucode.len() + 1;
    let rel_off = (loop_pc_idx as i32 - delay_slot_pc_idx as i32) as i16;
    ucode.push(enc_bne(r4, r0, rel_off));
    ucode.push(enc_addiu(r3, r3, 32)); // delay slot advances interleaved output ptr

    let (r5, r6, r7) = (5, 6, 7);
    ucode.push(enc_addiu(r5, r0, MIXER_OUT_DMEM as i16));
    ucode.push(enc_mtc0(r5, 0)); // SP_MEM_ADDR = MIXER_OUT_DMEM

    let dram_hi = (dram_dst >> 16) as u16;
    let dram_lo = (dram_dst & 0xFFFF) as u16;
    if dram_hi != 0 {
        ucode.push(enc_lui(r6, dram_hi));
        ucode.push(enc_ori(r6, r6, dram_lo));
    } else {
        ucode.push(enc_ori(r6, r0, dram_lo));
    }
    ucode.push(enc_mtc0(r6, 1)); // SP_DRAM_ADDR = dram_dst

    let total_bytes = u32::from(num_blocks.max(1)) * 32;
    ucode.push(enc_ori(r7, r0, (total_bytes - 1) as u16));
    ucode.push(enc_mtc0(r7, 3)); // SP_WR_LEN -> DMA DMEM to RDRAM
    ucode.push(RSP_BREAK);

    ucode
}

/// Bit-exact Q15 `VMULF`/`VMACF` oracle for stereo mix verification.
#[must_use]
pub fn oracle_stereo_mix(voice_a: &[i16], voice_b: &[i16], gains: [i16; 4]) -> Vec<i16> {
    let [g_a_l, g_a_r, g_b_l, g_b_r] = gains;
    let num_samples = voice_a.len().min(voice_b.len());
    let mut out = Vec::with_capacity(num_samples * 2);

    for i in 0..num_samples {
        let a = i64::from(voice_a[i]);
        let b = i64::from(voice_b[i]);

        let acc_l = ((a * i64::from(g_a_l)) << 1) + 0x8000 + ((b * i64::from(g_b_l)) << 1);
        let sample_l = (acc_l >> 16).clamp(-32_768, 32_767) as i16;

        let acc_r = ((a * i64::from(g_a_r)) << 1) + 0x8000 + ((b * i64::from(g_b_r)) << 1);
        let sample_r = (acc_r >> 16).clamp(-32_768, 32_767) as i16;

        out.push(sample_l);
        out.push(sample_r);
    }

    out
}

/// Build Microcode #2 — 32-bit `VADDC`/`VADD` Resampler Phase + 48-bit Linear Predictor Kernel.
#[must_use]
pub fn build_rsp_phase_and_predictor_ucode() -> Vec<u32> {
    let (r0, r1) = (0, 1);
    let mut ucode = Vec::new();
    ucode.push(enc_addiu(r1, r0, 0));

    for reg_off in 0..8i8 {
        ucode.push(enc_lqv((reg_off as u32) + 1, 0, reg_off, r1));
    }

    ucode.push(enc_vu(0, 2, 1, 9, 0x14)); // VADDC v9, v1, v2
    ucode.push(enc_vu(0, 4, 3, 10, 0x10)); // VADD v10, v3, v4
    ucode.push(enc_sqv(9, 0, 16, r1)); // DMEM 0x100
    ucode.push(enc_sqv(10, 0, 17, r1)); // DMEM 0x110

    ucode.push(enc_vu(0, 6, 5, 11, 0x04)); // VMUDL v11, v5, v6
    ucode.push(enc_vu(0, 8, 7, 11, 0x0D)); // VMACM v11, v7, v8
    ucode.push(enc_vu(10, 0, 0, 12, 0x1D)); // VSAW v12, acc_lo
    ucode.push(enc_sqv(12, 0, 18, r1)); // DMEM 0x120
    ucode.push(RSP_BREAK);

    ucode
}

/// Bit-exact oracle for 32-bit phase increment and 48-bit linear predictor kernel.
#[must_use]
pub fn oracle_phase_and_predictor(input: &PhasePredictorInput) -> PhasePredictorOutput {
    let mut new_phase_lo = [0u16; 8];
    let mut new_phase_hi = [0u16; 8];
    let mut vco = 0u16;

    for i in 0..8 {
        let sum_lo = u32::from(input.phase_lo[i]) + u32::from(input.step_lo[i]);
        new_phase_lo[i] = sum_lo as u16;
        let carry = u32::from(sum_lo > 0xFFFF);
        if carry != 0 {
            vco |= 1 << i;
        }
        let sum_hi = u32::from(input.phase_hi[i]) + u32::from(input.step_hi[i]) + carry;
        new_phase_hi[i] = sum_hi as u16;
    }

    let mut pred_lo = [0u16; 8];
    let mut acc_results = [0i64; 8];
    for i in 0..8 {
        let prod0 = (u64::from(input.s0[i]) * u64::from(input.c0[i])) >> 16;
        let prod1 = i64::from(input.s1[i] as i16) * i64::from(input.c1[i]);
        let acc = (prod0 as i64).wrapping_add(prod1);
        acc_results[i] = acc;
        pred_lo[i] = (acc & 0xFFFF) as u16;
    }

    PhasePredictorOutput {
        phase_lo: new_phase_lo,
        phase_hi: new_phase_hi,
        vco,
        pred_lo,
        acc: acc_results,
    }
}
