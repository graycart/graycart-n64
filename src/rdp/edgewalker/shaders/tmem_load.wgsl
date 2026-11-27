// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// TMEM load compute shader (Load Tile, Load Block, Load TLUT).
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor / Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//   Load Tile 0x34, Load Block 0x33, Load TLUT 0x30.
//
// Zero NVVM integer division or modulo is used. Row/col splits use
// u32(floor(f32(index) / f32(width))). Power-of-two alignments use bit shifts/masks.

struct TmemLoadParams {
  op: u32,
  img_addr: u32,
  img_width: u32,
  img_size: u32,
  tile_size: u32,
  tile_line: u32,
  tile_tmem: u32,
  s0: u32,
  t0: u32,
  width: u32,
  height: u32,
  dxt: u32,
  transfer: u32,
  pad0: u32,
  pad1: u32,
  pad2: u32,
}

@group(0) @binding(0) var<storage, read> lparams: array<TmemLoadParams>;
@group(0) @binding(1) var<storage, read> rdram: array<u32>;
@group(0) @binding(2) var<storage, read_write> tmem: array<atomic<u32>>;

fn read_rdram_byte(addr: u32) -> u32 {
  let word_idx = addr >> 2u;
  let byte_off = addr & 3u;
  if word_idx >= arrayLength(&rdram) {
    return 0u;
  }
  let w = rdram[word_idx];
  let shift = byte_off * 8u;
  return (w >> shift) & 0xFFu;
}

fn tmem_store_byte(addr: u32, byte_val: u32) {
  let masked_addr = addr & 0xFFFu;
  let word_idx = masked_addr >> 2u;
  let byte_shift = (masked_addr & 3u) * 8u;
  let mask = 0xFFu << byte_shift;
  let val = (byte_val & 0xFFu) << byte_shift;
  if word_idx >= 1024u {
    return;
  }
  loop {
    let old = atomicLoad(&tmem[word_idx]);
    let desired = (old & ~mask) | val;
    let res = atomicCompareExchangeWeak(&tmem[word_idx], old, desired);
    if res.exchanged {
      break;
    }
  }
}

fn tmem_store_nibble(addr: u32, high: bool, nib: u32) {
  let masked_addr = addr & 0xFFFu;
  let word_idx = masked_addr >> 2u;
  let byte_shift = (masked_addr & 3u) * 8u;
  let nib_shift = byte_shift + select(0u, 4u, high);
  let mask = 0xFu << nib_shift;
  let val = (nib & 0xFu) << nib_shift;
  if word_idx >= 1024u {
    return;
  }
  loop {
    let old = atomicLoad(&tmem[word_idx]);
    let desired = (old & ~mask) | val;
    let res = atomicCompareExchangeWeak(&tmem[word_idx], old, desired);
    if res.exchanged {
      break;
    }
  }
}

fn swizzled_addr(tile_tmem: u32, tile_line: u32, row: u32, byte_off: u32, high_half: bool) -> u32 {
  var addr = (tile_tmem + row * tile_line) << 3u;
  addr = addr + byte_off;
  if high_half {
    addr = addr + 0x800u;
  }
  if (row & 1u) != 0u {
    addr = addr ^ 4u;
  }
  return addr & 0xFFFu;
}

fn block_addr(dst_base: u32, byte_index: u32, dxt: u32) -> u32 {
  let word_index = byte_index >> 3u;
  let line = (word_index * dxt) >> 11u;
  var addr = dst_base + byte_index;
  if (line & 1u) != 0u {
    addr = addr ^ 4u;
  }
  return addr & 0xFFFu;
}

@compute @workgroup_size(64)
fn tmem_load_main(@builtin(global_invocation_id) id: vec3<u32>) {
  if arrayLength(&lparams) == 0u {
    return;
  }
  let p = lparams[0];
  let index = id.x;

  if p.op == 0u {
    // LoadTile
    if p.width == 0u || p.height == 0u {
      return;
    }
    let total = p.width * p.height;
    if index >= total {
      return;
    }
    let row = u32(floor(f32(index) / f32(p.width)));
    let col = index - row * p.width;

    if p.tile_size == 0u {
      // 4-bit (nibble)
      let texel = (p.t0 + row) * p.img_width + (p.s0 + col);
      let byte_val = read_rdram_byte(p.img_addr + (texel >> 1u));
      let nib = select(byte_val & 0xFu, byte_val >> 4u, (texel & 1u) == 0u);
      let dst = swizzled_addr(p.tile_tmem, p.tile_line, row, col >> 1u, false);
      tmem_store_nibble(dst, (col & 1u) == 0u, nib);
    } else if p.tile_size == 1u {
      // 8-bit
      let src = p.img_addr + (p.t0 + row) * p.img_width + (p.s0 + col);
      let byte_val = read_rdram_byte(src);
      let dst = swizzled_addr(p.tile_tmem, p.tile_line, row, col, false);
      tmem_store_byte(dst, byte_val);
    } else if p.tile_size == 2u {
      // 16-bit
      let src = p.img_addr + ((p.t0 + row) * p.img_width + (p.s0 + col)) * 2u;
      let hi = read_rdram_byte(src);
      let lo = read_rdram_byte(src + 1u);
      let dst0 = swizzled_addr(p.tile_tmem, p.tile_line, row, col * 2u, false);
      let dst1 = swizzled_addr(p.tile_tmem, p.tile_line, row, col * 2u + 1u, false);
      tmem_store_byte(dst0, hi);
      tmem_store_byte(dst1, lo);
    } else if p.tile_size == 3u {
      // 32-bit RGBA32
      let src = p.img_addr + ((p.t0 + row) * p.img_width + (p.s0 + col)) * 4u;
      let b0 = read_rdram_byte(src);
      let b1 = read_rdram_byte(src + 1u);
      let b2 = read_rdram_byte(src + 2u);
      let b3 = read_rdram_byte(src + 3u);
      let dst_low0 = swizzled_addr(p.tile_tmem, p.tile_line, row, col * 2u, false);
      let dst_low1 = swizzled_addr(p.tile_tmem, p.tile_line, row, col * 2u + 1u, false);
      let dst_high0 = swizzled_addr(p.tile_tmem, p.tile_line, row, col * 2u, true);
      let dst_high1 = swizzled_addr(p.tile_tmem, p.tile_line, row, col * 2u + 1u, true);
      tmem_store_byte(dst_low0, b0);
      tmem_store_byte(dst_low1, b1);
      tmem_store_byte(dst_high0, b2);
      tmem_store_byte(dst_high1, b3);
    }
  } else if p.op == 1u {
    // LoadBlock
    if index >= p.width {
      return;
    }
    let dst_base = p.tile_tmem << 3u;
    if p.transfer == 0u {
      // Nibble
      let start = p.t0 * p.img_width + p.s0;
      let texel = start + index;
      let byte_val = read_rdram_byte(p.img_addr + (texel >> 1u));
      let nib = select(byte_val & 0xFu, byte_val >> 4u, (texel & 1u) == 0u);
      let byte_off = index >> 1u;
      let addr = block_addr(dst_base, byte_off, p.dxt);
      tmem_store_nibble(addr, (index & 1u) == 0u, nib);
    } else if p.transfer == 1u {
      // 1-byte
      let src = p.img_addr + p.t0 * p.img_width + p.s0 + index;
      let byte_val = read_rdram_byte(src);
      let addr = block_addr(dst_base, index, p.dxt);
      tmem_store_byte(addr, byte_val);
    } else if p.transfer == 2u {
      // 2-byte
      let src = p.img_addr + (p.t0 * p.img_width + p.s0 + index) * 2u;
      let hi = read_rdram_byte(src);
      let lo = read_rdram_byte(src + 1u);
      let byte_off0 = index * 2u;
      let byte_off1 = index * 2u + 1u;
      let addr0 = block_addr(dst_base, byte_off0, p.dxt);
      let addr1 = block_addr(dst_base, byte_off1, p.dxt);
      tmem_store_byte(addr0, hi);
      tmem_store_byte(addr1, lo);
    }
  } else if p.op == 2u {
    // LoadTlut
    if p.width == 0u || p.height == 0u {
      return;
    }
    let total = p.width * p.height;
    if index >= total {
      return;
    }
    let row = u32(floor(f32(index) / f32(p.width)));
    let col = index - row * p.width;
    let src = p.img_addr + ((p.t0 + row) * p.img_width + (p.s0 + col)) * 2u;
    let hi = read_rdram_byte(src);
    let lo = read_rdram_byte(src + 1u);
    let word_addr = (p.tile_tmem + index) & 0x1FFu;
    let dst = word_addr << 3u;
    for (var copy = 0u; copy < 4u; copy = copy + 1u) {
      tmem_store_byte(dst + copy * 2u, hi);
      tmem_store_byte(dst + copy * 2u + 1u, lo);
    }
  }
}
