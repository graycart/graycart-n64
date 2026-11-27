// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
//! Unit tests for Audio PCM subsystem (ring buffer, zero-float resampler, WAV encoder).
//!
//! Cited: N64brew — Audio Interface · https://n64brew.dev/wiki/Audio_Interface · docs/05-audio.md

use super::*;

#[test]
fn test_ring_fifo_push_drain_and_telemetry() {
    let mut ring = AudioRingBuffer::with_capacity(4);
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    assert_eq!(ring.capacity(), 4);

    ring.push_frame(100, -100);
    ring.push_frame(200, -200);
    assert_eq!(ring.len(), 2);
    assert_eq!(ring.telemetry().total_frames_produced, 2);
    assert_eq!(ring.telemetry().peak_left, 200);
    assert_eq!(ring.telemetry().peak_right, 200);

    ring.reset_peaks();
    assert_eq!(ring.telemetry().peak_left, 0);
    assert_eq!(ring.telemetry().peak_right, 0);

    let mut out = Vec::new();
    let drained = ring.drain_frames(&mut out);
    assert_eq!(drained, 2);
    assert_eq!(out, vec![(100, -100), (200, -200)]);
    assert!(ring.is_empty());

    ring.push_frame(300, 400);
    ring.clear_buffer();
    assert!(ring.is_empty());
}

#[test]
fn test_peaks_and_clipped_samples() {
    let mut ring = AudioRingBuffer::with_capacity(10);
    ring.push_frame(i16::MIN, i16::MAX);
    let tel = ring.telemetry();
    assert_eq!(tel.peak_left, 32768);
    assert_eq!(tel.peak_right, 32767);
    assert_eq!(tel.clipped_samples, 2);
}

#[test]
fn test_capacity_overflow_and_hash_invariance() {
    let mut ring = AudioRingBuffer::with_capacity(2);
    ring.push_frame(1, 2);
    ring.push_frame(3, 4);
    ring.push_frame(5, 6); // should pop (1, 2) and increment dropped_frames

    assert_eq!(ring.len(), 2);
    assert_eq!(ring.telemetry().dropped_frames, 1);
    assert_eq!(ring.telemetry().total_frames_produced, 3);

    let all_pushed = vec![(1, 2), (3, 4), (5, 6)];
    let mut flat = Vec::new();
    for (l, r) in all_pushed {
        flat.push(l);
        flat.push(r);
    }
    let expected_hash = fnv1a_pcm_i16(&flat);
    assert_eq!(ring.pcm_hash(), expected_hash);
}

#[test]
fn test_pcm_resampler_varieties() {
    // 1:1 passthrough
    let mut resampler = PcmResampler::new();
    let src = vec![(100, 200), (300, 400)];
    let mut out = Vec::new();
    let count = resampler.resample(&src, 44100, 44100, &mut out);
    assert_eq!(count, 2);
    assert_eq!(out, src);

    // Upsample 22050 -> 44100
    resampler.reset();
    out.clear();
    let src_up = vec![(0, 0), (1000, 1000), (2000, 2000)];
    let count_up = resampler.resample(&src_up, 22050, 44100, &mut out);
    assert!(count_up > 0);

    // Downsample 48000 -> 22050 & NTSC 44135 -> 48000
    resampler.reset();
    out.clear();
    let src_ntsc = vec![(100, -100), (200, -200), (300, -300), (400, -400)];
    let count_ntsc = resampler.resample(&src_ntsc, 44135, 48000, &mut out);
    assert!(count_ntsc > 0);

    // Chunked vs Monolithic bit-exact check
    let mut res_mono = PcmResampler::new();
    let mut out_mono = Vec::new();
    let stream = vec![
        (10, -10),
        (20, -20),
        (30, -30),
        (40, -40),
        (50, -50),
        (60, -60),
    ];
    res_mono.resample(&stream, 22050, 44100, &mut out_mono);

    let mut res_chunk = PcmResampler::new();
    let mut out_chunk = Vec::new();
    res_chunk.resample(&stream[..3], 22050, 44100, &mut out_chunk);
    res_chunk.resample(&stream[3..], 22050, 44100, &mut out_chunk);

    assert_eq!(out_mono, out_chunk);

    // Zero rate safety
    let mut res_zero = PcmResampler::new();
    assert_eq!(res_zero.resample(&stream, 0, 44100, &mut out), 0);
    assert_eq!(res_zero.resample(&stream, 44100, 0, &mut out), 0);
}

#[test]
fn test_encode_wav_pcm16_stereo_and_serde() {
    let samples = vec![100, -100, 200, -200];
    let wav = encode_wav_pcm16_stereo(&samples, 44100);
    assert!(wav.is_some());
    let bytes = wav.unwrap();
    assert!(bytes.len() >= 44);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(&bytes[36..40], b"data");

    // Odd sample count -> None
    let odd_samples = vec![100, -100, 200];
    assert!(encode_wav_pcm16_stereo(&odd_samples, 44100).is_none());

    // Zero rate -> None
    assert!(encode_wav_pcm16_stereo(&samples, 0).is_none());

    // Serde round-trip
    let ring = AudioRingBuffer::new();
    let serialized = serde_json::to_string(&ring).unwrap();
    let deserialized: AudioRingBuffer = serde_json::from_str(&serialized).unwrap();
    assert_eq!(ring.capacity(), deserialized.capacity());
    assert_eq!(ring.pcm_hash(), deserialized.pcm_hash());
}
