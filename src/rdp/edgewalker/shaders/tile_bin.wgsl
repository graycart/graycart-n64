// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// Screen-tile binning stage. A bin is a workgroup screen tile of primitive
// ids. It is not one of the eight RDP `Set Tile` descriptors.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor/Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//
// Entry point: tile_bin_main
// Workgroup: 64. Dispatch ceil(prim_count / 64) workgroups.
// One thread per primitive. Thread `i` writes `prim_bin_counts[i]` and every
// slot of its pair slice. Threads outside the primitive prefix return.
//
// Pixel AABB is half-open and already in integer pixels: [x0, x1) × [y0, y1).
// The host converts an RDP rectangle (s10.2) to that box before upload.
// FILL's inclusive pixel range is x = (ulx >> 2) ..= ceil_10_2(lrx), so the
// uploaded exclusive edge is that inclusive max plus one (same for y).
// This stage does not interpret s10.2 itself.
//
// Screen grid: `bin_size` pixels on a side (EdgeWalker default 32),
// `bins_x` by `bins_y` bins. Bin id = by * bins_x + bx, row-major.
// A primitive overlaps bin (bx, by) when its AABB hits
// [bx * bin_size, (bx + 1) * bin_size) × [by * bin_size, (by + 1) * bin_size).
// Coordinates below 0 are clipped to the origin. An empty box
// (x1 <= x0 or y1 <= y0), a non-positive bin size, or a zero grid overlaps
// nothing.
//
// Flat pair list (deterministic, one writer per slot, no atomics):
//   pairs[prim_id * max_bins_per_prim + slot]
// Slots are filled in row-major bin order (by, then bx).
//   overlap <= max_bins_per_prim:
//     the first `overlap` slots are live; the rest of the slice are sentinels.
//   overlap > max_bins_per_prim:
//     the first `max_bins_per_prim` bins in that order are stored and the
//     rest are dropped. `prim_bin_counts[prim_id]` is still the full overlap,
//     so the host detects truncation when count > max_bins_per_prim.
// Live BinPair: bin_id as above, prim_id = thread index.
// Sentinel (unused slot): bin_id = 0xFFFFFFFF, prim_id = 0xFFFFFFFF.
// Bins with no primitive are absent from the pair list. The host builds an
// empty ScreenBin for them from `bins_x` and `bins_y` (bin x = id % bins_x,
// bin y = id / bins_x). Pair order inside one primitive is sorted; a bin's
// primitive list, after the host gathers pairs, follows increasing prim_id
// if the host scans primitives in order.
//
// @group(0) binding table (all storage):
//   binding 0  prims            read        array<PrimBounds>  stride 16
//   binding 1  pairs            read_write  array<BinPair>     stride 8
//   binding 2  grid             read        array<BinGrid>     stride 32
//                               only element 0 is read
//   binding 3  prim_bin_counts  read_write  array<u32>         stride 4
//
// PrimBounds (half-open pixel AABB):
//   0 x0:i32  4 y0:i32  8 x1:i32  12 y1:i32
// BinPair:
//   0 bin_id:u32  4 prim_id:u32
// BinGrid (element 0):
//   0 bins_x:u32
//   4 bins_y:u32
//   8 bin_size:u32
//   12 prim_count:u32
//   16 max_bins_per_prim:u32
//   20 _pad0:u32
//   24 _pad1:u32
//   28 _pad2:u32
// prim_bin_counts[i]: u32 full overlap count (not clamped).
//
// Required sizes:
//   len(prims) >= prim_count
//   len(prim_bin_counts) >= prim_count
//   len(pairs) >= prim_count * max_bins_per_prim
//   len(grid) >= 1
// Host reads grid[0].prim_count. The shader also clamps it to the bound arrays.
//
// Layout vs the previous stub:
//   binding 0 PrimBounds is unchanged (x0,y0,x1,y1 as i32) but is now a
//   half-open pixel box, not an unread placeholder.
//   binding 1 was `bins: array<BinList>` {x,y,count}. It is now `pairs`.
//   bindings 2 and 3 are new.

struct PrimBounds {
  x0: i32,
  y0: i32,
  x1: i32,
  y1: i32,
}

struct BinPair {
  bin_id: u32,
  prim_id: u32,
}

struct BinGrid {
  bins_x: u32,
  bins_y: u32,
  bin_size: u32,
  prim_count: u32,
  max_bins_per_prim: u32,
  _pad0: u32,
  _pad1: u32,
  _pad2: u32,
}

@group(0) @binding(0) var<storage, read> prims: array<PrimBounds>;
@group(0) @binding(1) var<storage, read_write> pairs: array<BinPair>;
@group(0) @binding(2) var<storage, read> grid: array<BinGrid>;
@group(0) @binding(3) var<storage, read_write> prim_bin_counts: array<u32>;

const SENTINEL_ID: u32 = 0xFFFFFFFFu;

// True when `a * b` does not fit in u32. No integer quotient: NVIDIA's
// compiler faults the device if this stage's SPIR-V contains one.
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

fn clamp_bin(v: i32, limit: i32) -> i32 {
  if v < 0 {
    return 0;
  }
  if v > limit {
    return limit;
  }
  return v;
}

@compute @workgroup_size(64)
fn tile_bin_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let index = id.x;
  if arrayLength(&grid) == 0u {
    return;
  }
  let g = grid[0];
  var prim_count = g.prim_count;
  if prim_count > arrayLength(&prims) {
    prim_count = arrayLength(&prims);
  }
  if prim_count > arrayLength(&prim_bin_counts) {
    prim_count = arrayLength(&prim_bin_counts);
  }
  if index >= prim_count {
    return;
  }

  let p = prims[index];
  let bins_x = i32(g.bins_x);
  let bins_y = i32(g.bins_y);
  let bin_size = i32(g.bin_size);
  var bx0 = 0;
  var by0 = 0;
  var bx1 = 0;
  var by1 = 0;
  let live = p.x1 > p.x0 && p.y1 > p.y0 && bin_size > 0 && bins_x > 0 && bins_y > 0 && p.x1 > 0 && p.y1 > 0;
  if live {
    let x_start = max(p.x0, 0);
    let y_start = max(p.y0, 0);
    bx0 = clamp_bin(i32(floor(f32(x_start) / f32(bin_size))), bins_x);
    by0 = clamp_bin(i32(floor(f32(y_start) / f32(bin_size))), bins_y);
    bx1 = clamp_bin(i32(floor(f32(p.x1 + bin_size - 1) / f32(bin_size))), bins_x);
    by1 = clamp_bin(i32(floor(f32(p.y1 + bin_size - 1) / f32(bin_size))), bins_y);
  }

  var overlap = 0u;
  if bx1 > bx0 && by1 > by0 {
    let span_x = u32(bx1 - bx0);
    let span_y = u32(by1 - by0);
    if span_y != 0u && u32_mul_overflows(span_x, span_y) {
      overlap = 0xFFFFFFFFu;
    } else {
      overlap = span_x * span_y;
    }
  }
  prim_bin_counts[index] = overlap;

  let max_bins = g.max_bins_per_prim;
  if max_bins == 0u {
    return;
  }
  if u32_mul_overflows(index, max_bins) {
    return;
  }
  let base = index * max_bins;
  var slot = 0u;
  if live && bx1 > bx0 && by1 > by0 {
    for (var by = by0; by < by1; by = by + 1) {
      for (var bx = bx0; bx < bx1; bx = bx + 1) {
        if slot < max_bins && (base + slot) < arrayLength(&pairs) {
          let bin_id = u32(by) * g.bins_x + u32(bx);
          pairs[base + slot].bin_id = bin_id;
          pairs[base + slot].prim_id = index;
          slot = slot + 1u;
        }
      }
    }
  }
  while slot < max_bins && (base + slot) < arrayLength(&pairs) {
    pairs[base + slot].bin_id = SENTINEL_ID;
    pairs[base + slot].prim_id = SENTINEL_ID;
    slot = slot + 1u;
  }
}
