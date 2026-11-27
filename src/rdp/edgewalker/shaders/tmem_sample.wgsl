// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// Texel sampler WGSL library module (no compute entry point).
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor / Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//   Set Tile, Set Tile Size, Texture Sampling formats and attributes.

struct GpuTileDesc {
  format: u32,
  size: u32,
  line: u32,
  tmem_addr: u32,
  flags: u32,
  sl_tl: u32,
  sh_th: u32,
  pad: u32,
}

@group(0) @binding(1) var<storage, read> tiles: array<GpuTileDesc>;
@group(0) @binding(2) var<storage, read> tmem: array<u32>;

fn tmem_byte_wgsl(addr: u32) -> u32 {
  let masked = addr & 0xFFFu;
  let word_idx = masked >> 2u;
  let byte_off = masked & 3u;
  if word_idx >= arrayLength(&tmem) {
    return 0u;
  }
  let w = tmem[word_idx];
  let shift = byte_off * 8u;
  return (w >> shift) & 0xFFu;
}

fn swizzled_addr_wgsl(tmem_addr: u32, line: u32, row: u32, byte_off: u32, high_half: bool) -> u32 {
  var addr = (tmem_addr + row * line) << 3u;
  addr = addr + byte_off;
  if high_half {
    addr = addr + 0x800u;
  }
  if (row & 1u) != 0u {
    addr = addr ^ 4u;
  }
  return addr & 0xFFFu;
}

fn read_u16_wgsl(tmem_addr: u32, line: u32, row: u32, s: u32) -> u32 {
  let addr = swizzled_addr_wgsl(tmem_addr, line, row, s * 2u, false);
  let hi = tmem_byte_wgsl(addr);
  let lo = tmem_byte_wgsl(addr + 1u);
  return (hi << 8u) | lo;
}

fn read_nibble_wgsl(tmem_addr: u32, line: u32, row: u32, s: u32) -> u32 {
  let addr = swizzled_addr_wgsl(tmem_addr, line, row, s >> 1u, false);
  let byte_val = tmem_byte_wgsl(addr);
  return select(byte_val & 0xFu, byte_val >> 4u, (s & 1u) == 0u);
}

fn replicate5(v: u32) -> u32 {
  let sub = v & 0x1Fu;
  return (sub << 3u) | (sub >> 2u);
}

fn replicate4(v: u32) -> u32 {
  let sub = v & 0xFu;
  return (sub << 4u) | sub;
}

fn replicate3(v: u32) -> u32 {
  let sub = v & 7u;
  return (sub << 5u) | (sub << 2u) | (sub >> 1u);
}

fn expand_rgba16(c: u32) -> vec4<u32> {
  let r = replicate5(c >> 11u);
  let g = replicate5(c >> 6u);
  let b = replicate5(c >> 1u);
  let a = select(0u, 255u, (c & 1u) != 0u);
  return vec4<u32>(r, g, b, a);
}

fn expand_ia16(c: u32) -> vec4<u32> {
  let i = c >> 8u;
  let a = c & 0xFFu;
  return vec4<u32>(i, i, i, a);
}

fn expand_ia8(c: u32) -> vec4<u32> {
  let i = replicate4(c >> 4u);
  let a = replicate4(c);
  return vec4<u32>(i, i, i, a);
}

fn expand_ia4(n: u32) -> vec4<u32> {
  let i = replicate3(n >> 1u);
  let a = select(0u, 255u, (n & 1u) != 0u);
  return vec4<u32>(i, i, i, a);
}

fn expand_i8(c: u32) -> vec4<u32> {
  return vec4<u32>(c, c, c, c);
}

fn expand_i4(n: u32) -> vec4<u32> {
  let i = replicate4(n);
  return vec4<u32>(i, i, i, i);
}

fn apply_shift_wgsl(coord: i32, shift: u32) -> i32 {
  let s_val = shift & 0xFu;
  let c = (coord << 16u) >> 16u;
  if s_val == 0u {
    return c;
  }
  if s_val <= 10u {
    return c >> s_val;
  }
  return c << (16u - s_val);
}

fn axis_index_wgsl(coord: i32, shift: u32, mask: u32, mirror: u32, clamp_en: u32, lo_raw: u32, hi_raw: u32) -> i32 {
  let shifted = apply_shift_wgsl(coord, shift);
  let sl = i32((lo_raw & 0xFFFFu) >> 2u);
  let sh = i32((hi_raw & 0xFFFFu) >> 2u);
  let s = shifted >> 5u;
  let m = mask & 0xFu;
  if m == 0u {
    let idx = select(s, clamp(s, sl, sh), clamp_en != 0u);
    return idx - sl;
  }
  let period = 1i << m;
  var idx = 0i;
  if mirror != 0u {
    let doubled = s & ((period << 1u) - 1i);
    var i = doubled & (period - 1i);
    if (doubled & period) != 0i {
      i = (period - 1i) - i;
    }
    idx = i;
  } else {
    idx = s & (period - 1i);
  }
  if clamp_en != 0u && idx > sh {
    idx = sh;
  }
  return idx;
}

fn sample_texel(tile_idx: u32, s_in: i32, t_in: i32, tlut_ia16: u32) -> vec4<u32> {
  if arrayLength(&tiles) == 0u {
    return vec4<u32>(0u);
  }
  let t = tiles[tile_idx & 7u];
  let flags = t.flags;
  let palette = flags & 0xFu;
  let clamp_t = (flags >> 4u) & 1u;
  let mirror_t = (flags >> 5u) & 1u;
  let mask_t = (flags >> 6u) & 0xFu;
  let shift_t = (flags >> 10u) & 0xFu;
  let clamp_s = (flags >> 14u) & 1u;
  let mirror_s = (flags >> 15u) & 1u;
  let mask_s = (flags >> 16u) & 0xFu;
  let shift_s = (flags >> 20u) & 0xFu;

  let sl_raw = t.sl_tl & 0xFFFFu;
  let tl_raw = t.sl_tl >> 16u;
  let sh_raw = t.sh_th & 0xFFFFu;
  let th_raw = t.sh_th >> 16u;

  let s = u32(max(axis_index_wgsl(s_in, shift_s, mask_s, mirror_s, clamp_s, sl_raw, sh_raw), 0));
  let row = u32(max(axis_index_wgsl(t_in, shift_t, mask_t, mirror_t, clamp_t, tl_raw, th_raw), 0));

  let fmt = t.format;
  let size = t.size;

  if fmt == 0u && size == 2u {
    // RGBA16
    let val = read_u16_wgsl(t.tmem_addr, t.line, row, s);
    return expand_rgba16(val);
  } else if fmt == 0u && size == 3u {
    // RGBA32
    let low_addr = swizzled_addr_wgsl(t.tmem_addr, t.line, row, s * 2u, false);
    let high_addr = swizzled_addr_wgsl(t.tmem_addr, t.line, row, s * 2u, true);
    let r = tmem_byte_wgsl(low_addr);
    let g = tmem_byte_wgsl(low_addr + 1u);
    let b = tmem_byte_wgsl(high_addr);
    let a = tmem_byte_wgsl(high_addr + 1u);
    return vec4<u32>(r, g, b, a);
  } else if fmt == 2u && size == 0u {
    // CI4
    let nib = read_nibble_wgsl(t.tmem_addr, t.line, row, s);
    let tlut_word = 0x100u | (palette << 4u) | nib;
    let tlut_addr = tlut_word << 3u;
    let hi = tmem_byte_wgsl(tlut_addr);
    let lo = tmem_byte_wgsl(tlut_addr + 1u);
    let raw = (hi << 8u) | lo;
    if tlut_ia16 != 0u {
      return expand_ia16(raw);
    } else {
      return expand_rgba16(raw);
    }
  } else if fmt == 2u && size == 1u {
    // CI8
    let idx_byte = tmem_byte_wgsl(swizzled_addr_wgsl(t.tmem_addr, t.line, row, s, false));
    let tlut_word = 0x100u | idx_byte;
    let tlut_addr = tlut_word << 3u;
    let hi = tmem_byte_wgsl(tlut_addr);
    let lo = tmem_byte_wgsl(tlut_addr + 1u);
    let raw = (hi << 8u) | lo;
    if tlut_ia16 != 0u {
      return expand_ia16(raw);
    } else {
      return expand_rgba16(raw);
    }
  } else if fmt == 3u && size == 0u {
    // IA4
    let n = read_nibble_wgsl(t.tmem_addr, t.line, row, s);
    return expand_ia4(n);
  } else if fmt == 3u && size == 1u {
    // IA8
    let byte_val = tmem_byte_wgsl(swizzled_addr_wgsl(t.tmem_addr, t.line, row, s, false));
    return expand_ia8(byte_val);
  } else if fmt == 3u && size == 2u {
    // IA16
    let val = read_u16_wgsl(t.tmem_addr, t.line, row, s);
    return expand_ia16(val);
  } else if fmt >= 4u && size == 0u {
    // I4
    let n = read_nibble_wgsl(t.tmem_addr, t.line, row, s);
    return expand_i4(n);
  } else if fmt >= 4u && size == 1u {
    // I8
    let byte_val = tmem_byte_wgsl(swizzled_addr_wgsl(t.tmem_addr, t.line, row, s, false));
    return expand_i8(byte_val);
  }
  return vec4<u32>(0u, 0u, 0u, 0u);
}
