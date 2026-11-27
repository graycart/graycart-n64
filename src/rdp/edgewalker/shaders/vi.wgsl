// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// This is the VI scan-out stage of that RDP.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Video Interface
//   https://n64brew.dev/wiki/Video_Interface
//
// Output is RGBA8888. The 1× frame hash must match CpuRdpLle (pre-filter and
// post-filter, once the CPU oracle's VI path is the reference) before this
// pass feeds presentation.
//
// M6 gate: Rust `Pipeline::dispatch_vi` traps under `vulkan-rdp` until that
// hash matches. This entry writes nothing.

@group(0) @binding(0) var<storage, read> color: array<u32>;
@group(0) @binding(1) var<storage, read> coverage: array<u32>;
@group(0) @binding(2) var<storage, read_write> rgba8888: array<u32>;

@compute @workgroup_size(64)
fn vi_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let index = id.x;
  if index >= arrayLength(&color)
    || index >= arrayLength(&coverage)
    || index >= arrayLength(&rgba8888)
  {
    return;
  }
}
