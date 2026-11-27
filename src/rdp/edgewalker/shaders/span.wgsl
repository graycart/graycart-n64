// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// This is the span stage of that RDP: raster, TMEM, combiner, blender,
// 9th-bit coverage, and Z, writing the RDRAM mirrors.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor
//   https://n64brew.dev/wiki/Reality_Display_Processor
//
// Combiner equation, when implemented: (A - B) * C + D, per cycle.
// TMEM is 4 KiB. `coverage` is the RDRAM 9th-bit side array.
// Dither and noise use a deterministic per-span seed so workgroups match
// the serial CpuRdpLle hash bit-for-bit.
//
// M6 gate: Rust `Pipeline::dispatch_span` traps under `vulkan-rdp` until
// that hash matches. This entry writes nothing.

struct SpanParams {
  other_modes_lo: u32,
  other_modes_hi: u32,
  combine_lo: u32,
  combine_hi: u32,
}

@group(0) @binding(0) var<storage, read> params: array<SpanParams>;
@group(0) @binding(1) var<storage, read> tmem: array<u32>;
@group(0) @binding(2) var<storage, read_write> color: array<u32>;
@group(0) @binding(3) var<storage, read_write> coverage: array<u32>;
@group(0) @binding(4) var<storage, read_write> depth: array<u32>;

@compute @workgroup_size(64)
fn span_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let index = id.x;
  if index >= arrayLength(&params)
    || index >= arrayLength(&tmem)
    || index >= arrayLength(&color)
    || index >= arrayLength(&coverage)
    || index >= arrayLength(&depth)
  {
    return;
  }
}
