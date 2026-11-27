// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Unit and live GPU tests for EdgeWalker Color Combiner & Blender.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use super::*;

#[test]
fn validate_combine_accepts_valid_modes_and_traps_unsupported() {
    // 1-cycle primitive * shade with alpha_c=7 (0) and blend mux 0 = P: combined, A:0, M:mem, B:1-A
    let combine = (3u64 << 37) | (4u64 << 32) | (7u64 << 24) | (7u64 << 18);
    assert!(validate_combine_and_blend(combine, 0, 0xFFFF_FFFF, 0).is_ok());

    // key_en traps
    assert!(validate_combine_and_blend(combine, 1 << 40, 0xFFFF_FFFF, 0).is_err());

    // NOISE (rgb_a = 7 in cycle 1) traps
    let noise_combine = (7u64 << 37) | (4u64 << 32);
    assert!(validate_combine_and_blend(noise_combine, 0, 0, 0).is_err());

    // Blender B == 1 traps when blend is active
    assert!(validate_combine_and_blend(combine, 1 << 18, 0, 0).is_err());

    // Blender B == 3 with A == 3 (zero denominator) traps
    assert!(validate_combine_and_blend(combine, (3 << 26) | (3 << 18), 0, 0).is_err());
}

#[cfg(feature = "vulkan-rdp")]
fn run_combine_blend_harness(
    st: CombineStatePack,
    texel0: [u32; 4],
    shade: [u32; 4],
    memory: [u32; 4],
) -> [u32; 8] {
    use super::super::ComputeDevice;

    let device = ComputeDevice::open().expect("EdgeWalker compute device");
    let wgsl = format!(
        "{COMBINE_BLEND_WGSL}\n
struct HarnessInput {{
    st: CombineState,
    texel0: vec4<u32>,
    shade: vec4<u32>,
    memory: vec4<u32>,
}}
@group(0) @binding(0) var<storage, read> inp: array<HarnessInput>;
@group(0) @binding(1) var<storage, read_write> out_words: array<u32>;

@compute @workgroup_size(1)
fn harness_main() {{
    let x = inp[0];
    let combined = eval_combine_gpu(x.st, x.texel0, x.texel0, x.shade);
    let blended = eval_blend_gpu(x.st, combined, x.shade.w, x.memory);
    out_words[0] = combined.x;
    out_words[1] = combined.y;
    out_words[2] = combined.z;
    out_words[3] = combined.w;
    out_words[4] = blended.x;
    out_words[5] = blended.y;
    out_words[6] = blended.z;
    out_words[7] = blended.w;
}}
"
    );
    let pipeline = device
        .create_compute_pipeline(&wgsl, "harness_main")
        .expect("compile combine_blend harness");

    let mut in_bytes = Vec::with_capacity(64 + 48);
    in_bytes.extend_from_slice(&st.to_bytes());
    for word in texel0.iter().chain(shade.iter()).chain(memory.iter()) {
        in_bytes.extend_from_slice(&word.to_le_bytes());
    }

    let in_buf = device
        .create_storage_buffer(in_bytes.len() as u64)
        .expect("in_buf");
    device
        .write_buffer(&in_buf, 0, &in_bytes)
        .expect("write in");
    let out_buf = device.create_storage_buffer(32).expect("out_buf");
    let group = device
        .create_storage_bind_group(&pipeline, 0, &[(0, &in_buf), (1, &out_buf)])
        .expect("bind group");
    device
        .dispatch_workgroups(&pipeline, Some(&group), 1, 1, 1)
        .expect("dispatch combine harness");
    let raw = device.readback(&out_buf).expect("readback");
    let mut out = [0u32; 8];
    for (i, dst) in out.iter_mut().enumerate() {
        let s = i * 4;
        *dst = u32::from_le_bytes([raw[s], raw[s + 1], raw[s + 2], raw[s + 3]]);
    }
    out
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn gpu_combine_1cycle_2cycle_and_blend_match_n64_math() {
    // 1-cycle: RGB = (TEXEL0 - 0) * PRIM + 0, A = TEXEL0.a.
    // Muxes in cycle 1: a1=1, c1=3, aa1=7, ac1=7, b1=15(0), d1=7(0), ab1=7(0), ad1=1(texel0.a)
    let combine = (1u64 << 37)
        | (3u64 << 32)
        | (15u64 << 24)
        | (7u64 << 21)
        | (7u64 << 18)
        | (7u64 << 6)
        | (7u64 << 3)
        | 1u64;
    // 1-cycle blender: P = combined (0), A = fog.a (1), M = fog (3), B = 1 - A (0)
    // fog = (0, 0, 200, 128) -> (200 * 128 + 200 * 128) >> 8 for blue etc.
    let other_modes = (1u64 << 26) | (3u64 << 22);
    let st = combine_state_pack(
        combine,
        other_modes,
        0x8040_FF00, // prim = (128, 64, 255, 0)
        0,
        0x0000_C880, // fog = (0, 0, 200, 128)
        0,
        0,
        0,
        0,
        [0; 3],
        [0; 3],
    );
    let res = run_combine_blend_harness(st, [200, 200, 100, 180], [0; 4], [0; 4]);
    // Combined RGB: (200*128)>>8 = 100, (200*64)>>8 = 50, (100*255)>>8 = 99, A = 180
    assert_eq!(&res[0..4], &[100, 50, 99, 180]);
    // Blended RGB with A=128, B=128: R=(100*128 + 0)>>8=50, G=25, B=(99*128 + 200*128)>>8=149, A=180
    assert_eq!(&res[4..8], &[50, 25, 149, 180]);

    // Test 2-cycle divider path (b == 2: denom = factor_a + 256 -> factor_a=128 => denom=384)
    // Cycle 0 passes texel0 through unchanged; Cycle 1 adds prim (clamp9),
    // Blend mux0 uses B=2 (256), A=0 (combined.a = 128), M=blend_color(2) (0,0,0,0), P=in(0).
    let combine2 = (1u64 << 52) // c0 a=texel0
        | (31u64 << 47) // c0 c=0
        | (15u64 << 28) // c0 b=0
        | (1u64 << 15) // c0 d=texel0 -> c0 = texel0
        | (6u64 << 44) // c0 aa=0
        | (7u64 << 12) // c0 ab=0
        | (1u64 << 9) // c0 ad=texel0
        // c1 a=combined (0)
        | (10u64 << 32) // c1 c=PRIM_ALPHA (128)
        | (15u64 << 24) // c1 b=0
        | (3u64 << 6) // c1 d=prim -> (combined * prim.a)>>8 + prim
        | (7u64 << 21)
        | (7u64 << 18)
        | (7u64 << 3);
    let om2 = (1u64 << 52) // 2-cycle
        | (2u64 << 22) // m0 = blend_color (0)
        | (2u64 << 18) // b0 = 256 (denom = 128 + 256 = 384)
        | (3u64 << 24) // a1 = 0
        | (3u64 << 16); // b1 = 0 -> factor_a=0, factor_b=0? No, use a1=0, b1=0 -> factor_b=256-128 -> p*(128/256)+... wait: set a1=1(fog.a=256? max 255), wait set b1=3, a1=1 (fog.a=255) so cycle 1 passes cycle 0 P!
    let om2 = om2 | (1u64 << 24) | (3u64 << 16);
    let st2 = combine_state_pack(
        combine2,
        om2,
        0x1818_1880, // prim RGB = 24, A = 128
        0,
        0x0000_00FF, // fog.a = 255
        0,
        0,
        0,
        0,
        [0; 3],
        [0; 3],
    );
    let res2 = run_combine_blend_harness(st2, [120, 120, 120, 128], [0; 4], [0; 4]);
    // Combined: (120 * 128) >> 8 + 24 = 60 + 24 = 84, A = 128
    assert_eq!(&res2[0..4], &[84, 84, 84, 128]);
    // Blended: (84 * 128) / (128 + 256) = 10752 / 384 = 28!
    assert_eq!(&res2[4..8], &[28, 28, 28, 128]);
}
