// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
// Cited: N64brew — Reality Display Processor (RDP) Color Combiner & Blender
//   https://n64brew.dev/wiki/Reality_Display_Processor/Color_Combiner
//   https://n64brew.dev/wiki/Reality_Display_Processor/Blender

struct CombineState {
    combine_lo: u32,
    combine_hi: u32,
    other_modes_lo: u32,
    other_modes_hi: u32,
    prim_color: u32,
    env_color: u32,
    fog_color: u32,
    blend_color: u32,
    prim_lod_frac: u32,
    convert_k4: u32,
    convert_k5: u32,
    key_center_rgb: u32,
    key_scale_rgb: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

const CC_ONE: i32 = 256;

fn clamp9_cc(v: i32) -> u32 {
    let n = u32(v) & 0x1FFu;
    if (n & 0x100u) == 0u {
        return n;
    }
    if (n & 0x80u) == 0u {
        return 255u;
    }
    return 0u;
}

fn cc_mul(a: i32, b: i32, c: i32, d: i32) -> i32 {
    return (((a - b) * c) >> 8u) + d;
}

// 18-bit integer division clamping quotient to 255 with zero / or % operators.
fn div_u32_clamped255(num: u32, denom_in: u32) -> u32 {
    var denom = denom_in;
    if (denom == 0u) {
        denom = 1u;
    }
    var rem = 0u;
    var quot = 0u;
    for (var i = 17; i >= 0; i--) {
        quot = quot << 1u;
        rem = rem << 1u;
        if ((num & (1u << u32(i))) != 0u) {
            rem = rem | 1u;
        }
        if (rem >= denom) {
            rem = rem - denom;
            quot = quot | 1u;
        }
    }
    if (quot > 255u) {
        return 255u;
    }
    return quot;
}

fn decode_rgb_a(
    mux: u32,
    combined: vec4<i32>,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    prim: vec4<u32>,
    shade: vec4<u32>,
    env: vec4<u32>,
    st: CombineState
) -> i32 {
    switch (mux) {
        case 0u: { return combined.x; }
        case 1u: { return i32(texel0.x); }
        case 2u: { return i32(texel1.x); }
        case 3u: { return i32(prim.x); }
        case 4u: { return i32(shade.x); }
        case 5u: { return i32(env.x); }
        case 6u: { return 256; }
        case 7u: { return 0; } // noise (validated out if unsupported)
        default: { return 0; }
    }
}

fn decode_rgb_b(
    mux: u32,
    combined: vec4<i32>,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    prim: vec4<u32>,
    shade: vec4<u32>,
    env: vec4<u32>,
    st: CombineState
) -> i32 {
    switch (mux) {
        case 0u: { return combined.x; }
        case 1u: { return i32(texel0.x); }
        case 2u: { return i32(texel1.x); }
        case 3u: { return i32(prim.x); }
        case 4u: { return i32(shade.x); }
        case 5u: { return i32(env.x); }
        case 6u: { return i32(st.key_center_rgb & 0xFFu); }
        case 7u: { return i32(st.convert_k4 & 0x1FFu); }
        default: { return 0; }
    }
}

fn decode_rgb_c(
    mux: u32,
    combined: vec4<i32>,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    prim: vec4<u32>,
    shade: vec4<u32>,
    env: vec4<u32>,
    st: CombineState
) -> i32 {
    switch (mux) {
        case 0u: { return combined.x; }
        case 1u: { return i32(texel0.x); }
        case 2u: { return i32(texel1.x); }
        case 3u: { return i32(prim.x); }
        case 4u: { return i32(shade.x); }
        case 5u: { return i32(env.x); }
        case 6u: { return i32(st.key_scale_rgb & 0xFFu); }
        case 7u: { return combined.w; }
        case 8u: { return i32(texel0.w); }
        case 9u: { return i32(texel1.w); }
        case 10u: { return i32(prim.w); }
        case 11u: { return i32(shade.w); }
        case 12u: { return i32(env.w); }
        case 13u: { return 0; } // lod_fraction (validated out if unsupported)
        case 14u: { return i32(st.prim_lod_frac & 0xFFu); }
        case 15u: { return i32(st.convert_k5 & 0x1FFu); }
        default: { return 0; }
    }
}

fn decode_rgb_d(
    mux: u32,
    combined: vec4<i32>,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    prim: vec4<u32>,
    shade: vec4<u32>,
    env: vec4<u32>,
    st: CombineState
) -> i32 {
    switch (mux) {
        case 0u: { return combined.x; }
        case 1u: { return i32(texel0.x); }
        case 2u: { return i32(texel1.x); }
        case 3u: { return i32(prim.x); }
        case 4u: { return i32(shade.x); }
        case 5u: { return i32(env.x); }
        case 6u: { return 256; }
        case 7u: { return 0; }
        default: { return 0; }
    }
}

fn decode_alpha_a_b_d(
    mux: u32,
    combined: vec4<i32>,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    prim: vec4<u32>,
    shade: vec4<u32>,
    env: vec4<u32>,
    st: CombineState
) -> i32 {
    switch (mux) {
        case 0u: { return combined.w; }
        case 1u: { return i32(texel0.w); }
        case 2u: { return i32(texel1.w); }
        case 3u: { return i32(prim.w); }
        case 4u: { return i32(shade.w); }
        case 5u: { return i32(env.w); }
        case 6u: { return 256; }
        case 7u: { return 0; }
        default: { return 0; }
    }
}

fn decode_alpha_c(
    mux: u32,
    combined: vec4<i32>,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    prim: vec4<u32>,
    shade: vec4<u32>,
    env: vec4<u32>,
    st: CombineState
) -> i32 {
    switch (mux) {
        case 0u: { return 0; } // lod_fraction (validated out)
        case 1u: { return i32(texel0.w); }
        case 2u: { return i32(texel1.w); }
        case 3u: { return i32(prim.w); }
        case 4u: { return i32(shade.w); }
        case 5u: { return i32(env.w); }
        case 6u: { return i32(st.prim_lod_frac & 0xFFu); }
        case 7u: { return 0; }
        default: { return 0; }
    }
}

fn eval_combine_cycle(
    which: u32,
    st: CombineState,
    texel0: vec4<u32>,
    texel1: vec4<u32>,
    shade: vec4<u32>,
    prim: vec4<u32>,
    env: vec4<u32>,
    prev_combined: vec4<i32>
) -> vec4<i32> {
    var rgb_a_mux: u32 = 0u;
    var rgb_b_mux: u32 = 0u;
    var rgb_c_mux: u32 = 0u;
    var rgb_d_mux: u32 = 0u;
    var alpha_a_mux: u32 = 0u;
    var alpha_b_mux: u32 = 0u;
    var alpha_c_mux: u32 = 0u;
    var alpha_d_mux: u32 = 0u;

    if (which == 0u) {
        rgb_a_mux = (st.combine_hi >> 20u) & 0xFu;
        rgb_c_mux = (st.combine_hi >> 15u) & 0x1Fu;
        alpha_a_mux = (st.combine_hi >> 12u) & 0x7u;
        alpha_c_mux = (st.combine_hi >> 9u) & 0x7u;
        rgb_b_mux = (st.combine_lo >> 28u) & 0xFu;
        rgb_d_mux = (st.combine_lo >> 15u) & 0x7u;
        alpha_b_mux = (st.combine_lo >> 12u) & 0x7u;
        alpha_d_mux = (st.combine_lo >> 9u) & 0x7u;
    } else {
        rgb_a_mux = (st.combine_hi >> 5u) & 0xFu;
        rgb_c_mux = st.combine_hi & 0x1Fu;
        alpha_a_mux = (st.combine_lo >> 21u) & 0x7u;
        alpha_c_mux = (st.combine_lo >> 18u) & 0x7u;
        rgb_b_mux = (st.combine_lo >> 24u) & 0xFu;
        rgb_d_mux = (st.combine_lo >> 6u) & 0x7u;
        alpha_b_mux = (st.combine_lo >> 3u) & 0x7u;
        alpha_d_mux = st.combine_lo & 0x7u;
    }

    var r_res = cc_mul(
        decode_rgb_a(rgb_a_mux, prev_combined, texel0, texel1, prim, shade, env, st),
        decode_rgb_b(rgb_b_mux, prev_combined, texel0, texel1, prim, shade, env, st),
        decode_rgb_c(rgb_c_mux, prev_combined, texel0, texel1, prim, shade, env, st),
        decode_rgb_d(rgb_d_mux, prev_combined, texel0, texel1, prim, shade, env, st)
    );
    var g_res = cc_mul(
        decode_rgb_a(rgb_a_mux, vec4<i32>(prev_combined.y, 0, 0, prev_combined.w), vec4<u32>(texel0.y, 0u, 0u, texel0.w), vec4<u32>(texel1.y, 0u, 0u, texel1.w), vec4<u32>(prim.y, 0u, 0u, prim.w), vec4<u32>(shade.y, 0u, 0u, shade.w), vec4<u32>(env.y, 0u, 0u, env.w), st),
        decode_rgb_b(rgb_b_mux, vec4<i32>(prev_combined.y, 0, 0, prev_combined.w), vec4<u32>(texel0.y, 0u, 0u, texel0.w), vec4<u32>(texel1.y, 0u, 0u, texel1.w), vec4<u32>(prim.y, 0u, 0u, prim.w), vec4<u32>(shade.y, 0u, 0u, shade.w), vec4<u32>(env.y, 0u, 0u, env.w), st),
        decode_rgb_c(rgb_c_mux, vec4<i32>(prev_combined.y, 0, 0, prev_combined.w), vec4<u32>(texel0.y, 0u, 0u, texel0.w), vec4<u32>(texel1.y, 0u, 0u, texel1.w), vec4<u32>(prim.y, 0u, 0u, prim.w), vec4<u32>(shade.y, 0u, 0u, shade.w), vec4<u32>(env.y, 0u, 0u, env.w), st),
        decode_rgb_d(rgb_d_mux, vec4<i32>(prev_combined.y, 0, 0, prev_combined.w), vec4<u32>(texel0.y, 0u, 0u, texel0.w), vec4<u32>(texel1.y, 0u, 0u, texel1.w), vec4<u32>(prim.y, 0u, 0u, prim.w), vec4<u32>(shade.y, 0u, 0u, shade.w), vec4<u32>(env.y, 0u, 0u, env.w), st)
    );
    var b_res = cc_mul(
        decode_rgb_a(rgb_a_mux, vec4<i32>(prev_combined.z, 0, 0, prev_combined.w), vec4<u32>(texel0.z, 0u, 0u, texel0.w), vec4<u32>(texel1.z, 0u, 0u, texel1.w), vec4<u32>(prim.z, 0u, 0u, prim.w), vec4<u32>(shade.z, 0u, 0u, shade.w), vec4<u32>(env.z, 0u, 0u, env.w), st),
        decode_rgb_b(rgb_b_mux, vec4<i32>(prev_combined.z, 0, 0, prev_combined.w), vec4<u32>(texel0.z, 0u, 0u, texel0.w), vec4<u32>(texel1.z, 0u, 0u, texel1.w), vec4<u32>(prim.z, 0u, 0u, prim.w), vec4<u32>(shade.z, 0u, 0u, shade.w), vec4<u32>(env.z, 0u, 0u, env.w), st),
        decode_rgb_c(rgb_c_mux, vec4<i32>(prev_combined.z, 0, 0, prev_combined.w), vec4<u32>(texel0.z, 0u, 0u, texel0.w), vec4<u32>(texel1.z, 0u, 0u, texel1.w), vec4<u32>(prim.z, 0u, 0u, prim.w), vec4<u32>(shade.z, 0u, 0u, shade.w), vec4<u32>(env.z, 0u, 0u, env.w), st),
        decode_rgb_d(rgb_d_mux, vec4<i32>(prev_combined.z, 0, 0, prev_combined.w), vec4<u32>(texel0.z, 0u, 0u, texel0.w), vec4<u32>(texel1.z, 0u, 0u, texel1.w), vec4<u32>(prim.z, 0u, 0u, prim.w), vec4<u32>(shade.z, 0u, 0u, shade.w), vec4<u32>(env.z, 0u, 0u, env.w), st)
    );
    var a_res = cc_mul(
        decode_alpha_a_b_d(alpha_a_mux, prev_combined, texel0, texel1, prim, shade, env, st),
        decode_alpha_a_b_d(alpha_b_mux, prev_combined, texel0, texel1, prim, shade, env, st),
        decode_alpha_c(alpha_c_mux, prev_combined, texel0, texel1, prim, shade, env, st),
        decode_alpha_a_b_d(alpha_d_mux, prev_combined, texel0, texel1, prim, shade, env, st)
    );

    return vec4<i32>(r_res, g_res, b_res, a_res);
}

fn eval_combine_gpu(st: CombineState, texel0: vec4<u32>, texel1: vec4<u32>, shade: vec4<u32>) -> vec4<u32> {
    let prim = vec4<u32>(
        (st.prim_color >> 24u) & 0xFFu,
        (st.prim_color >> 16u) & 0xFFu,
        (st.prim_color >> 8u) & 0xFFu,
        st.prim_color & 0xFFu
    );
    let env = vec4<u32>(
        (st.env_color >> 24u) & 0xFFu,
        (st.env_color >> 16u) & 0xFFu,
        (st.env_color >> 8u) & 0xFFu,
        st.env_color & 0xFFu
    );

    let cycle_type = (st.other_modes_hi >> 20u) & 3u;
    var c0 = vec4<i32>(0, 0, 0, 0);
    if (cycle_type != 0u) {
        c0 = eval_combine_cycle(0u, st, texel0, texel1, shade, prim, env, vec4<i32>(0));
    }
    let c1 = eval_combine_cycle(1u, st, texel0, texel1, shade, prim, env, c0);

    return vec4<u32>(
        clamp9_cc(c1.x),
        clamp9_cc(c1.y),
        clamp9_cc(c1.z),
        clamp9_cc(c1.w)
    );
}

fn decode_blend_color_channel(sel: u32, in_c: u32, mem_c: u32, blend_c: u32, fog_c: u32) -> u32 {
    switch (sel) {
        case 0u: { return in_c; }
        case 1u: { return mem_c; }
        case 2u: { return blend_c; }
        case 3u: { return fog_c; }
        default: { return in_c; }
    }
}

fn decode_blend_alpha(sel: u32, combined_a: u32, fog_a: u32, shade_a: u32) -> u32 {
    switch (sel) {
        case 0u: { return combined_a; }
        case 1u: { return fog_a; }
        case 2u: { return shade_a; }
        case 3u: { return 0u; }
        default: { return 0u; }
    }
}

fn eval_blend_single_mux(
    p_sel: u32,
    a_sel: u32,
    m_sel: u32,
    b_sel: u32,
    in_col: vec4<u32>,
    combined_a: u32,
    shade_a: u32,
    mem_col: vec4<u32>,
    blend_col: vec4<u32>,
    fog_col: vec4<u32>,
    channel_idx: u32
) -> u32 {
    let p = decode_blend_color_channel(p_sel, in_col[channel_idx], mem_col[channel_idx], blend_col[channel_idx], fog_col[channel_idx]);
    let m = decode_blend_color_channel(m_sel, in_col[channel_idx], mem_col[channel_idx], blend_col[channel_idx], fog_col[channel_idx]);
    let a_raw = decode_blend_alpha(a_sel, combined_a, fog_col.w, shade_a);
    let factor_a = min(a_raw, 256u);

    var factor_b = 0u;
    switch (b_sel) {
        case 0u: { factor_b = 256u - factor_a; }
        case 1u: { factor_b = 0u; } // mem_cvg unsupported
        case 2u: { factor_b = 256u; }
        case 3u: { factor_b = 0u; }
        default: { factor_b = 0u; }
    }

    if (factor_b == 0u && factor_a != 0u) {
        return min(p, 255u);
    }
    if (b_sel == 0u) {
        // factor_a + factor_b == 256u
        let num = p * factor_a + m * factor_b;
        return min(num >> 8u, 255u);
    }
    if (b_sel == 2u) {
        // denom = factor_a + 256u
        let num = p * factor_a + m * 256u;
        let denom = factor_a + 256u;
        return div_u32_clamped255(num, denom);
    }
    return min(p, 255u);
}

fn eval_blend_gpu(st: CombineState, combined: vec4<u32>, shade_a: u32, memory: vec4<u32>) -> vec4<u32> {
    let blend_c = vec4<u32>(
        (st.blend_color >> 24u) & 0xFFu,
        (st.blend_color >> 16u) & 0xFFu,
        (st.blend_color >> 8u) & 0xFFu,
        st.blend_color & 0xFFu
    );
    let fog_c = vec4<u32>(
        (st.fog_color >> 24u) & 0xFFu,
        (st.fog_color >> 16u) & 0xFFu,
        (st.fog_color >> 8u) & 0xFFu,
        st.fog_color & 0xFFu
    );

    let cycle_type = (st.other_modes_hi >> 20u) & 3u;
    let om_lo = st.other_modes_lo;

    // Mux set 0
    let p0 = (om_lo >> 30u) & 3u;
    let a0 = (om_lo >> 26u) & 3u;
    let m0 = (om_lo >> 22u) & 3u;
    let b0 = (om_lo >> 18u) & 3u;

    var cur_col = vec4<u32>(
        eval_blend_single_mux(p0, a0, m0, b0, combined, combined.w, shade_a, memory, blend_c, fog_c, 0u),
        eval_blend_single_mux(p0, a0, m0, b0, combined, combined.w, shade_a, memory, blend_c, fog_c, 1u),
        eval_blend_single_mux(p0, a0, m0, b0, combined, combined.w, shade_a, memory, blend_c, fog_c, 2u),
        combined.w
    );

    if (cycle_type != 0u) {
        let p1 = (om_lo >> 28u) & 3u;
        let a1 = (om_lo >> 24u) & 3u;
        let m1 = (om_lo >> 20u) & 3u;
        let b1 = (om_lo >> 16u) & 3u;
        cur_col = vec4<u32>(
            eval_blend_single_mux(p1, a1, m1, b1, cur_col, combined.w, shade_a, memory, blend_c, fog_c, 0u),
            eval_blend_single_mux(p1, a1, m1, b1, cur_col, combined.w, shade_a, memory, blend_c, fog_c, 1u),
            eval_blend_single_mux(p1, a1, m1, b1, cur_col, combined.w, shade_a, memory, blend_c, fog_c, 2u),
            combined.w
        );
    }

    return cur_col;
}
