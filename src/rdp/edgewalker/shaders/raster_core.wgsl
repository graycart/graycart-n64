// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// Rasterizer core WGSL stage: FillRectangle, TextureRectangle/Flip, Triangles
// (Gouraud, textured, 18-bit Z-buffer, AA/non-AA coverage), Color Combiner,
// Blender, Bayer/Magic RGB dither, and atomic RDRAM + 9th-bit hidden-bit stores.
//
// Zero NVVM integer `/` or `%` operators anywhere in this file.
// Cited: N64brew — Reality Display Processor / Commands & Pipeline
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Pipeline

struct RasterParams {
    prim_kind: u32,
    prim_flags: u32,
    bbox_x0: i32,
    bbox_y0: i32,
    bbox_w: u32,
    bbox_h: u32,
    other_modes_lo: u32,
    other_modes_hi: u32,
    combine_lo: u32,
    combine_hi: u32,
    fill_color: u32,
    prim_color: u32,
    env_color: u32,
    fog_color: u32,
    blend_color: u32,
    prim_lod_frac: u32,
    prim_z: u32,
    prim_dz: u32,
    color_addr: u32,
    color_width_px: u32,
    color_size: u32,
    color_format: u32,
    depth_addr: u32,
    scissor_ulx: u32,
    scissor_uly: u32,
    scissor_lrx: u32,
    scissor_lry: u32,
    scissor_field: u32,
    scissor_odd: u32,
    convert_k4: u32,
    convert_k5: u32,
    key_center_rgb: u32,
    key_scale_rgb: u32,
    rect_ulx: i32,
    rect_uly: i32,
    rect_lrx: i32,
    rect_lry: i32,
    rect_s: i32,
    rect_t: i32,
    rect_dsdx: i32,
    rect_dtdy: i32,
    yh: i32,
    ym: i32,
    yl: i32,
    xh: i32,
    xm: i32,
    xl: i32,
    dxhdy: i32,
    dxmdy: i32,
    dxldy: i32,
    shade_r: i32,
    shade_g: i32,
    shade_b: i32,
    shade_a: i32,
    drdx_r: i32,
    drdx_g: i32,
    drdx_b: i32,
    drdx_a: i32,
    drde_r: i32,
    drde_g: i32,
    drde_b: i32,
    drde_a: i32,
    z_anchor: i32,
    dzdx: i32,
    dzde: i32,
    tex_s: i32,
    tex_t: i32,
    tex_dsdx: i32,
    tex_dtdx: i32,
    tex_dsde: i32,
    tex_dtde: i32,
    pad0: u32,
};

@group(0) @binding(0) var<storage, read> rparams: array<RasterParams>;
@group(0) @binding(3) var<storage, read_write> rdram: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> hidden: array<atomic<u32>>;

struct I64 {
    lo: u32,
    hi: i32,
};

fn i64_pixel(px: i32) -> I64 {
    return I64(u32(px) << 16u, px >> 16u);
}

fn i64_pixel_center(px: i32) -> I64 {
    return I64((u32(px) << 16u) | 0x8000u, px >> 16u);
}

fn i64_from_i32(v: i32) -> I64 {
    return I64(u32(v), select(0, -1, v < 0));
}

fn i64_add(a: I64, b: I64) -> I64 {
    let lo_sum = a.lo + b.lo;
    let carry = select(0u, 1u, lo_sum < a.lo);
    let hi_sum = a.hi + b.hi + i32(carry);
    return I64(lo_sum, hi_sum);
}

fn i64_sub(a: I64, b: I64) -> I64 {
    let lo_sub = a.lo - b.lo;
    let borrow = select(0u, 1u, a.lo < b.lo);
    let hi_sub = a.hi - b.hi - i32(borrow);
    return I64(lo_sub, hi_sub);
}

fn i64_lt(a: I64, b: I64) -> bool {
    if a.hi != b.hi {
        return a.hi < b.hi;
    }
    return a.lo < b.lo;
}

fn i64_ge(a: I64, b: I64) -> bool {
    return !i64_lt(a, b);
}

fn i64_max(a: I64, b: I64) -> I64 {
    if i64_ge(a, b) {
        return a;
    }
    return b;
}

fn i64_min(a: I64, b: I64) -> I64 {
    if i64_lt(a, b) {
        return a;
    }
    return b;
}

fn u32_abs_i32(v: i32) -> u32 {
    if v < 0 {
        return u32(-(v + 1)) + 1u;
    }
    return u32(v);
}

fn mul_i32_to_i64(a: i32, b: i32) -> I64 {
    let neg = (a < 0) != (b < 0);
    let ua = u32_abs_i32(a);
    let ub = u32_abs_i32(b);
    let a0 = ua & 0xFFFFu;
    let a1 = ua >> 16u;
    let b0 = ub & 0xFFFFu;
    let b1 = ub >> 16u;
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;
    let mid = p01 + p10;
    let mid_carry = select(0u, 1u, mid < p01);
    let lo = p00 + (mid << 16u);
    let lo_carry = select(0u, 1u, lo < p00);
    let hi_u = p11 + (mid >> 16u) + (mid_carry << 16u) + lo_carry;
    var res = I64(lo, i32(hi_u));
    if neg && (lo != 0u || hi_u != 0u) {
        res = i64_add(I64(~res.lo, ~res.hi), I64(1u, 0));
    }
    return res;
}

fn i64_asr16_to_i64(v: I64) -> I64 {
    return I64((u32(v.hi) << 16u) | (v.lo >> 16u), v.hi >> 16u);
}

fn i64_shr16(v: I64) -> i32 {
    return i32((u32(v.hi) << 16u) | (v.lo >> 16u));
}

fn i64_shr13(v: I64) -> i32 {
    return i32((u32(v.hi) << 19u) | (v.lo >> 13u));
}

fn i64_shr15(v: I64) -> i32 {
    return i32((u32(v.hi) << 17u) | (v.lo >> 15u));
}

fn bswap32(x: u32) -> u32 {
    return ((x & 0xFFu) << 24u)
        | ((x & 0xFF00u) << 8u)
        | ((x & 0xFF0000u) >> 8u)
        | ((x & 0xFF000000u) >> 24u);
}

fn rdram_read_be_u32(byte_addr: u32) -> u32 {
    let word_idx = byte_addr >> 2u;
    if word_idx >= arrayLength(&rdram) {
        return 0u;
    }
    return bswap32(atomicLoad(&rdram[word_idx]));
}

fn rdram_read_be_u16(byte_addr: u32) -> u32 {
    let w = rdram_read_be_u32(byte_addr & ~3u);
    if (byte_addr & 2u) != 0u {
        return w & 0xFFFFu;
    }
    return w >> 16u;
}

fn rdram_read_u8(byte_addr: u32) -> u32 {
    let half = rdram_read_be_u16(byte_addr & ~1u);
    if (byte_addr & 1u) != 0u {
        return half & 0xFFu;
    }
    return half >> 8u;
}

fn rdram_store_be_u32(byte_addr: u32, val: u32) {
    let word_idx = byte_addr >> 2u;
    if word_idx >= arrayLength(&rdram) {
        return;
    }
    atomicStore(&rdram[word_idx], bswap32(val));
}

fn rdram_store_be_u16(byte_addr: u32, val: u32) {
    let word_idx = byte_addr >> 2u;
    if word_idx >= arrayLength(&rdram) {
        return;
    }
    let is_low = (byte_addr & 2u) != 0u;
    let half = val & 0xFFFFu;
    var current = atomicLoad(&rdram[word_idx]);
    loop {
        let decoded = bswap32(current);
        var next_decoded = (decoded & 0xFFFFu) | (half << 16u);
        if is_low {
            next_decoded = (decoded & 0xFFFF0000u) | half;
        }
        let res = atomicCompareExchangeWeak(&rdram[word_idx], current, bswap32(next_decoded));
        if res.exchanged {
            break;
        }
        current = res.old_value;
    }
}

fn rdram_store_u8(byte_addr: u32, val: u32) {
    let word_idx = byte_addr >> 2u;
    if word_idx >= arrayLength(&rdram) {
        return;
    }
    let shift = (byte_addr & 3u) * 8u;
    let mask = ~(0xFFu << shift);
    let ins = (val & 0xFFu) << shift;
    var current = atomicLoad(&rdram[word_idx]);
    loop {
        let res = atomicCompareExchangeWeak(&rdram[word_idx], current, (current & mask) | ins);
        if res.exchanged {
            break;
        }
        current = res.old_value;
    }
}

fn hidden_read(byte_addr: u32) -> u32 {
    let word_idx = byte_addr >> 5u;
    if word_idx >= arrayLength(&hidden) {
        return 0u;
    }
    return (atomicLoad(&hidden[word_idx]) >> (byte_addr & 31u)) & 1u;
}

fn hidden_write(byte_addr: u32, bit_val: bool) {
    let word_idx = byte_addr >> 5u;
    if word_idx >= arrayLength(&hidden) {
        return;
    }
    let mask = 1u << (byte_addr & 31u);
    if bit_val {
        atomicOr(&hidden[word_idx], mask);
    } else {
        atomicAnd(&hidden[word_idx], ~mask);
    }
}

fn u32_mul_overflows(a: u32, b: u32) -> bool {
    let a_hi = a >> 16u;
    let a_lo = a & 0xFFFFu;
    let b_hi = b >> 16u;
    let b_lo = b & 0xFFFFu;
    if a_hi != 0u && b_hi != 0u {
        return true;
    }
    let mid = a_hi * b_lo + a_lo * b_hi;
    let low = a_lo * b_lo;
    let sum_mid = (low >> 16u) + (mid & 0xFFFFu);
    return (mid >> 16u) + (sum_mid >> 16u) != 0u;
}

fn allow_line(y: i32, field: u32, odd: u32) -> bool {
    if field == 0u {
        return true;
    }
    return ((y & 1) != 0) == (odd != 0u);
}

fn pixel_in_scissor(p: RasterParams, x: i32, y: i32, fill_like: bool) -> bool {
    if !allow_line(y, p.scissor_field, p.scissor_odd) {
        return false;
    }
    let ulx = i32(p.scissor_ulx);
    let uly = i32(p.scissor_uly);
    let lrx = i32(p.scissor_lrx);
    let lry = i32(p.scissor_lry);
    if fill_like {
        return x >= (ulx >> 2u) && x <= (lrx >> 2u) && y >= (uly >> 2u) && y < (lry >> 2u);
    }
    let sx = x * 4 + 2;
    let sy = y * 4 + 2;
    return sx >= ulx && sx < lrx && sy >= uly && sy < lry;
}

fn scissor_overlaps_pixel(p: RasterParams, x: i32, y: i32) -> bool {
    let px0 = x * 4;
    let py0 = y * 4;
    return px0 < i32(p.scissor_lrx)
        && px0 + 4 > i32(p.scissor_ulx)
        && py0 < i32(p.scissor_lry)
        && py0 + 4 > i32(p.scissor_uly);
}

fn i64_overlap(a0: I64, a1: I64, b0: I64, b1: I64) -> I64 {
    let lo = i64_max(a0, b0);
    let hi = i64_min(a1, b1);
    let diff = i64_sub(hi, lo);
    if diff.hi < 0 {
        return I64(0u, 0);
    }
    return diff;
}

fn raw_from_area(x_ov: I64, y_quarters: i32) -> u32 {
    if (x_ov.hi < 0 || (x_ov.hi == 0 && x_ov.lo == 0u)) || y_quarters <= 0 {
        return 0u;
    }
    // x_ov <= 0x10000 and y_quarters <= 4 -> product <= 262144, fits in i32.
    let area = i32(x_ov.lo) * y_quarters;
    let raw = area >> 15u;
    if raw <= 0 {
        return 1u;
    }
    if raw >= 8 {
        return 8u;
    }
    return u32(raw);
}

fn tri_coverage_raw(left: I64, right: I64, yh: i32, yl: i32, x: i32, y: i32) -> u32 {
    let x0 = i64_pixel(x);
    let x1 = i64_pixel(x + 1);
    let x_ov = i64_overlap(x0, x1, left, right);
    let top = y * 4;
    let y_lo = max(top, yh);
    let y_hi = min(top + 4, yl);
    let y_ov = max(y_hi - y_lo, 0);
    return raw_from_area(x_ov, y_ov);
}

fn rect_coverage_raw(p: RasterParams, ulx: i32, uly: i32, lrx: i32, lry: i32, x: i32, y: i32) -> u32 {
    let c_ulx = max(ulx, i32(p.scissor_ulx));
    let c_uly = max(uly, i32(p.scissor_uly));
    let c_lrx = min(lrx, i32(p.scissor_lrx));
    let c_lry = min(lry, i32(p.scissor_lry));
    if c_ulx >= c_lrx || c_uly >= c_lry {
        return 0u;
    }
    let px0 = i64_pixel(x);
    let px1 = i64_pixel(x + 1);
    let b0 = mul_i32_to_i64(c_ulx, 16384);
    let b1 = mul_i32_to_i64(c_lrx, 16384);
    let x_ov = i64_overlap(px0, px1, b0, b1);
    let top = y * 4;
    let y_lo = max(top, c_uly);
    let y_hi = min(top + 4, c_lry);
    let y_ov = max(y_hi - y_lo, 0);
    return raw_from_area(x_ov, y_ov);
}

fn div_ceil_4(v: i32) -> i32 {
    if v >= 0 {
        return (v + 3) >> 2u;
    }
    return -((-v) >> 2u);
}

fn ceil_10_2(v: i32) -> i32 {
    let i = v >> 2u;
    if (v & 3) == 0 {
        return i;
    }
    return i + 1;
}

fn unpack_rgba(c: u32) -> vec4<u32> {
    return vec4<u32>(
        (c >> 24u) & 0xFFu,
        (c >> 16u) & 0xFFu,
        (c >> 8u) & 0xFFu,
        c & 0xFFu
    );
}

fn magic_dither(row: u32, col: u32) -> i32 {
    let idx = (row << 2u) | col;
    switch (idx) {
        case 0u: { return 0; }
        case 1u: { return 6; }
        case 2u: { return 1; }
        case 3u: { return 7; }
        case 4u: { return 4; }
        case 5u: { return 2; }
        case 6u: { return 5; }
        case 7u: { return 3; }
        case 8u: { return 3; }
        case 9u: { return 5; }
        case 10u: { return 2; }
        case 11u: { return 4; }
        case 12u: { return 7; }
        case 13u: { return 1; }
        case 14u: { return 6; }
        default: { return 0; }
    }
}

fn bayer_dither(row: u32, col: u32) -> i32 {
    let idx = (row << 2u) | col;
    switch (idx) {
        case 0u: { return 0; }
        case 1u: { return 4; }
        case 2u: { return 1; }
        case 3u: { return 5; }
        case 4u: { return 4; }
        case 5u: { return 0; }
        case 6u: { return 5; }
        case 7u: { return 1; }
        case 8u: { return 3; }
        case 9u: { return 7; }
        case 10u: { return 2; }
        case 11u: { return 6; }
        case 12u: { return 7; }
        case 13u: { return 3; }
        case 14u: { return 6; }
        default: { return 2; }
    }
}

fn dither_of(p: RasterParams, x: i32, y: i32) -> i32 {
    let mode = (p.other_modes_hi >> 6u) & 3u;
    if mode == 3u {
        return 0;
    }
    let yi = select(u32(y) & 3u, u32(y >> 1u) & 3u, p.scissor_uly != 0u);
    let xi = u32(x) & 3u;
    if mode == 1u {
        return bayer_dither(yi, xi);
    }
    return magic_dither(yi, xi);
}

fn quantize5(chan: u32, d: i32) -> u32 {
    let v = clamp(i32(chan) + d, 0, 255);
    return u32(v) >> 3u;
}

fn read_coverage_gpu(byte_addr: u32, size: u32) -> u32 {
    if size == 4u {
        let a = rdram_read_u8(byte_addr + 3u);
        return (a >> 5u) & 7u;
    }
    if size == 2u {
        let px = rdram_read_be_u16(byte_addr);
        let h1 = hidden_read(byte_addr);
        let h0 = hidden_read(byte_addr + 1u);
        return ((px & 1u) << 2u) | (h1 << 1u) | h0;
    }
    return 0u;
}

fn combine_cvg_gpu(p: RasterParams, byte_addr: u32, size: u32, new_cvg: u32) -> u32 {
    let img_read = (p.other_modes_lo & (1u << 6u)) != 0u;
    var old = 0u;
    if img_read {
        old = read_coverage_gpu(byte_addr, size);
    }
    let sum = old + new_cvg;
    let dest = (p.other_modes_lo >> 8u) & 3u;
    if dest == 0u {
        return min(sum, 7u);
    }
    if dest == 1u {
        return sum & 7u;
    }
    if dest == 2u {
        return 7u;
    }
    if img_read {
        return old;
    }
    return 7u;
}

fn keep_memory_color_gpu(p: RasterParams, byte_addr: u32, size: u32, new_cvg: u32) -> bool {
    let color_on_cvg = (p.other_modes_lo & (1u << 7u)) != 0u;
    let img_read = (p.other_modes_lo & (1u << 6u)) != 0u;
    if !color_on_cvg || !img_read {
        return false;
    }
    return read_coverage_gpu(byte_addr, size) + new_cvg < 8u;
}

fn read_depth_gpu(byte_addr: u32) -> u32 {
    let half = rdram_read_be_u16(byte_addr);
    let h1 = hidden_read(byte_addr);
    let h0 = hidden_read(byte_addr + 1u);
    return (half << 2u) | (h1 << 1u) | h0;
}

fn write_depth_gpu(byte_addr: u32, z18: u32) {
    let masked = z18 & 0x3FFFFu;
    rdram_store_be_u16(byte_addr, masked >> 2u);
    hidden_write(byte_addr, (masked & 2u) != 0u);
    hidden_write(byte_addr + 1u, (masked & 1u) != 0u);
}

fn z18_from_i64(z: I64) -> u32 {
    if z.hi < 0 || (z.hi == 0 && z.lo == 0u) {
        return 0u;
    }
    return u32(i64_shr13(z)) & 0x3FFFFu;
}

fn z_passes_gpu(mode: u32, new_z: u32, old_z: u32, dz: u32) -> bool {
    if mode == 0u || mode == 2u {
        return new_z < old_z;
    }
    if mode == 1u {
        let sum = select(old_z + dz, 0xFFFFFFFFu, old_z > 0xFFFFFFFFu - dz);
        return new_z < sum;
    }
    let lo = select(0u, old_z - dz, old_z >= dz);
    let hi = select(old_z + dz, 0xFFFFFFFFu, old_z > 0xFFFFFFFFu - dz);
    return new_z >= lo && new_z <= hi;
}

@compute @workgroup_size(64)
fn raster_main(@builtin(global_invocation_id) id: vec3<u32>) {
    if arrayLength(&rparams) == 0u {
        return;
    }
    let p = rparams[0];
    if p.bbox_w == 0u || p.bbox_h == 0u || u32_mul_overflows(p.bbox_w, p.bbox_h) {
        return;
    }
    let total_threads = p.bbox_w * p.bbox_h;
    if id.x >= total_threads {
        return;
    }
    let ly = u32(floor(f32(id.x) / f32(p.bbox_w)));
    let lx = id.x - ly * p.bbox_w;
    let x = p.bbox_x0 + i32(lx);
    let y = p.bbox_y0 + i32(ly);
    if x < 0 || y < 0 {
        return;
    }

    let cycle = (p.other_modes_hi >> 20u) & 3u;
    let fill_like = (cycle == 3u);
    let aa = ((p.other_modes_lo & 8u) != 0u) && !fill_like;
    if !allow_line(y, p.scissor_field, p.scissor_odd) {
        return;
    }

    let tlut_ia16 = (p.other_modes_hi >> 14u) & 1u;
    var cvg = 7u;
    var texel = vec4<u32>(0u);
    var shade = unpack_rgba(p.prim_color);
    var edge_z = I64(0u, 0);
    var x_major = I64(0u, 0);
    var dzdx = p.dzdx;

    if p.prim_kind <= 2u {
        let ulx = p.rect_ulx;
        let uly = p.rect_uly;
        let lrx = p.rect_lrx;
        let lry = p.rect_lry;
        x_major = mul_i32_to_i64(ulx, 16384);
        edge_z = i64_pixel(i32(p.prim_z & 0x7FFFu));
        dzdx = 0;

        if fill_like {
            let x0 = ulx >> 2u;
            let y0 = uly >> 2u;
            let x1 = ceil_10_2(lrx);
            let y1 = ceil_10_2(lry);
            if x0 > x1 || y0 > y1 || x < x0 || x > x1 || y < y0 || y > y1 {
                return;
            }
            if !pixel_in_scissor(p, x, y, true) {
                return;
            }
            cvg = 7u;
        } else {
            if !pixel_in_scissor(p, x, y, false) && !aa {
                return;
            }
            if aa {
                if !scissor_overlaps_pixel(p, x, y) {
                    return;
                }
                let raw = rect_coverage_raw(p, ulx, uly, lrx, lry, x, y);
                if raw == 0u {
                    return;
                }
                cvg = raw - 1u;
            } else {
                let cx = x * 4 + 2;
                let cy = y * 4 + 2;
                if cx < ulx || cx >= lrx || cy < uly || cy >= lry {
                    return;
                }
                cvg = 7u;
            }
            if p.prim_kind >= 1u {
                let dx = x - (ulx >> 2u);
                let dy = y - (uly >> 2u);
                var s_step = 0;
                var t_step = 0;
                if p.prim_kind == 2u {
                    s_step = (p.rect_dsdx * dy) >> 5u;
                    t_step = (p.rect_dtdy * dx) >> 5u;
                } else {
                    s_step = (p.rect_dsdx * dx) >> 5u;
                    t_step = (p.rect_dtdy * dy) >> 5u;
                }
                let tile_idx = (p.prim_flags >> 4u) & 7u;
                texel = sample_texel(tile_idx, p.rect_s + s_step, p.rect_t + t_step, tlut_ia16);
            }
        }
    } else if p.prim_kind == 3u {
        let y_start = p.yh >> 2u;
        let y_mid = p.ym >> 2u;
        let y_end = div_ceil_4(p.yl);
        if y < y_start || y >= y_end {
            return;
        }
        let dy = y - y_start;
        x_major = i64_add(i64_from_i32(p.xh), mul_i32_to_i64(p.dxhdy, dy));

        let on_lower = y >= y_mid;
        let y_switch = max(y_start, y_mid);
        let dy_minor = select(dy, y - y_switch, on_lower);
        let x_minor_base = select(p.xm, p.xl, on_lower);
        let dx_minor_slope = select(p.dxmdy, p.dxldy, on_lower);
        let x_minor = i64_add(i64_from_i32(x_minor_base), mul_i32_to_i64(dx_minor_slope, dy_minor));

        let lmajor = (p.prim_flags & 1u) != 0u;
        var left = x_minor;
        var right = x_major;
        if lmajor {
            left = x_major;
            right = x_minor;
        }
        if !i64_lt(left, right) {
            return;
        }

        if aa {
            if !scissor_overlaps_pixel(p, x, y) {
                return;
            }
            let raw = tri_coverage_raw(left, right, p.yh, p.yl, x, y);
            if raw == 0u {
                return;
            }
            cvg = raw - 1u;
        } else {
            if !pixel_in_scissor(p, x, y, fill_like) {
                return;
            }
            let cx = i64_pixel_center(x);
            let cy = y * 4 + 2;
            if i64_lt(cx, left) || !i64_lt(cx, right) {
                return;
            }
            if !fill_like && (cy < p.yh || cy >= p.yl) {
                return;
            }
            cvg = 7u;
        }

        if !fill_like {
            let x_maj_px = i64_shr16(x_major);
            let dx_i64 = i64_sub(i64_pixel(x), x_major);
            let dx_16 = i64_shr16(dx_i64);
            let dx_step = i32((u32(dx_i64.lo) >> 16u) | (u32(dx_i64.hi) << 16u));
            let _unused_dx = dx_16;
            if (p.prim_flags & 2u) != 0u {
                // dx_fx is (x << 16) - x_major; shade + drde*dy + (drdx * dx_fx) >> 16, then >> 16.
                let edge_r = i64_add(i64_from_i32(p.shade_r), mul_i32_to_i64(p.drde_r, dy));
                let edge_g = i64_add(i64_from_i32(p.shade_g), mul_i32_to_i64(p.drde_g, dy));
                let edge_b = i64_add(i64_from_i32(p.shade_b), mul_i32_to_i64(p.drde_b, dy));
                let edge_a = i64_add(i64_from_i32(p.shade_a), mul_i32_to_i64(p.drde_a, dy));
                // Multiply in I64 via exact low*drdx (or i64_asr16 of drdx * (dx_fx >> 16) + frac part):
                // Since dx_fx fits in i32 for screen pixels ([-4096..4095]<<16 fits in i32):
                let dx_fx_i32 = i32(dx_i64.lo);
                let sr = i64_add(edge_r, i64_asr16_to_i64(mul_i32_to_i64(p.drdx_r, dx_fx_i32)));
                let sg = i64_add(edge_g, i64_asr16_to_i64(mul_i32_to_i64(p.drdx_g, dx_fx_i32)));
                let sb = i64_add(edge_b, i64_asr16_to_i64(mul_i32_to_i64(p.drdx_b, dx_fx_i32)));
                let sa = i64_add(edge_a, i64_asr16_to_i64(mul_i32_to_i64(p.drdx_a, dx_fx_i32)));
                shade = vec4<u32>(
                    u32(clamp(i64_shr16(sr), 0, 255)),
                    u32(clamp(i64_shr16(sg), 0, 255)),
                    u32(clamp(i64_shr16(sb), 0, 255)),
                    u32(clamp(i64_shr16(sa), 0, 255))
                );
            }
            if (p.prim_flags & 4u) != 0u {
                let edge_s = p.tex_s + p.tex_dsde * dy;
                let edge_t = p.tex_t + p.tex_dtde * dy;
                let dx_px = x - x_maj_px;
                let s = edge_s + p.tex_dsdx * dx_px;
                let t = edge_t + p.tex_dtdx * dx_px;
                let tile_idx = (p.prim_flags >> 4u) & 7u;
                texel = sample_texel(tile_idx, s, t, tlut_ia16);
            }
            if (p.prim_flags & 8u) != 0u {
                edge_z = i64_add(i64_from_i32(p.z_anchor), mul_i32_to_i64(p.dzde, dy));
            }
            let _unused_step = dx_step;
        }
    } else {
        return;
    }

    var final_color = vec4<u32>(0u);
    if !fill_like {
        let textured = (p.prim_flags & 4u) != 0u || p.prim_kind == 1u || p.prim_kind == 2u;
        if p.combine_lo == 0u && p.combine_hi == 0u {
            final_color = select(shade, texel, textured);
        } else {
            let st = CombineState(
                p.combine_lo,
                p.combine_hi,
                p.other_modes_lo,
                p.other_modes_hi,
                p.prim_color,
                p.env_color,
                p.fog_color,
                p.blend_color,
                p.prim_lod_frac,
                p.convert_k4,
                p.convert_k5,
                p.key_center_rgb,
                p.key_scale_rgb,
                0u,
                0u,
                0u
            );
            let combined = eval_combine_gpu(st, texel, texel, shade);
            let blend_blocked = (p.other_modes_lo & ((1u << 3u) | (1u << 7u) | (1u << 12u) | (1u << 13u))) != 0u
                || (p.other_modes_hi & (1u << 8u)) != 0u;
            if blend_blocked {
                final_color = combined;
            } else {
                final_color = eval_blend_gpu(st, combined, shade.w, vec4<u32>(0u));
            }
        }

        let z_cmp = (p.other_modes_lo & (1u << 4u)) != 0u;
        let z_upd = (p.other_modes_lo & (1u << 5u)) != 0u;
        if z_cmp || z_upd {
            let z_src_prim = (p.other_modes_lo & (1u << 2u)) != 0u;
            var z18 = 0u;
            var dz = 1u;
            if z_src_prim {
                z18 = (p.prim_z << 3u) & 0x3FFFFu;
                dz = max(p.prim_dz, 1u);
            } else {
                let dx_fx_i32 = i32(i64_sub(i64_pixel(x), x_major).lo);
                let z_fx = i64_add(edge_z, i64_asr16_to_i64(mul_i32_to_i64(dzdx, dx_fx_i32)));
                z18 = z18_from_i64(z_fx);
                let mag = u32_abs_i32(dzdx) >> 13u;
                dz = max(mag, 1u);
            }
            let z_pix = u32(y) * p.color_width_px + u32(x);
            let z_addr = p.depth_addr + z_pix * 2u;
            if z_cmp {
                let old_z = read_depth_gpu(z_addr);
                let z_mode = (p.other_modes_lo >> 10u) & 3u;
                if !z_passes_gpu(z_mode, z18, old_z, dz) {
                    return;
                }
            }
            if z_upd {
                write_depth_gpu(z_addr, z18);
            }
        }
    }

    let pix = u32(y) * p.color_width_px + u32(x);
    let size = p.color_size;
    if size == 1u {
        let byte_addr = p.color_addr + pix;
        var byte_val = final_color.x;
        if fill_like {
            let shift = u32(3 - (x & 3)) * 8u;
            byte_val = (p.fill_color >> shift) & 0xFFu;
        }
        rdram_store_u8(byte_addr, byte_val);
    } else if size == 2u {
        let byte_addr = p.color_addr + pix * 2u;
        if fill_like {
            let half = select(p.fill_color & 0xFFFFu, p.fill_color >> 16u, (x & 1) == 0);
            rdram_store_be_u16(byte_addr, half);
            return;
        }
        let out_cvg = combine_cvg_gpu(p, byte_addr, 2u, cvg);
        if keep_memory_color_gpu(p, byte_addr, 2u, cvg) {
            let old = rdram_read_be_u16(byte_addr);
            let px = (old & ~1u) | (out_cvg >> 2u);
            rdram_store_be_u16(byte_addr, px);
        } else {
            let d = dither_of(p, x, y);
            let r5 = quantize5(final_color.x, d);
            let g5 = quantize5(final_color.y, d);
            let b5 = quantize5(final_color.z, d);
            let px = (r5 << 11u) | (g5 << 6u) | (b5 << 1u) | (out_cvg >> 2u);
            rdram_store_be_u16(byte_addr, px);
        }
        hidden_write(byte_addr, (out_cvg & 2u) != 0u);
        hidden_write(byte_addr + 1u, (out_cvg & 1u) != 0u);
    } else if size == 3u {
        let byte_addr = p.color_addr + pix * 4u;
        if fill_like {
            rdram_store_be_u32(byte_addr, p.fill_color);
            return;
        }
        let out_cvg = combine_cvg_gpu(p, byte_addr, 4u, cvg);
        if keep_memory_color_gpu(p, byte_addr, 4u, cvg) {
            let old = rdram_read_be_u32(byte_addr);
            let px = (old & 0xFFFFFF1Fu) | (out_cvg << 5u);
            rdram_store_be_u32(byte_addr, px);
        } else {
            let a = (out_cvg << 5u) | (final_color.w >> 3u);
            let px = (final_color.x << 24u)
                | (final_color.y << 16u)
                | (final_color.z << 8u)
                | a;
            rdram_store_be_u32(byte_addr, px);
        }
        hidden_write(byte_addr, (out_cvg & 1u) != 0u);
        hidden_write(byte_addr + 1u, (out_cvg & 2u) != 0u);
        hidden_write(byte_addr + 2u, (out_cvg & 4u) != 0u);
        hidden_write(byte_addr + 3u, false);
    }
}
