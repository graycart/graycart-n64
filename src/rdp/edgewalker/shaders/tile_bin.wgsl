// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// This is the screen-tile binning stage of that RDP.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// A bin is an EdgeWalker workgroup tile (primitive id lists). It is not one
// of the eight RDP `Set Tile` descriptors.
//
// Cited: N64brew — Reality Display Processor/Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//
// M6 gate: Rust `Pipeline::dispatch(Stage::TileBin)` traps under `vulkan-rdp`
// until the 1× frame hash matches CpuRdpLle. This entry writes nothing.
// Workgroup size 64 is a placeholder.

struct PrimBounds {
  x0: i32,
  y0: i32,
  x1: i32,
  y1: i32,
}

struct BinList {
  x: u32,
  y: u32,
  count: u32,
}

@group(0) @binding(0) var<storage, read> prims: array<PrimBounds>;
@group(0) @binding(1) var<storage, read_write> bins: array<BinList>;

@compute @workgroup_size(64)
fn tile_bin_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let index = id.x;
  if index >= arrayLength(&prims) || index >= arrayLength(&bins) {
    return;
  }
}
