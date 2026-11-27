// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// This is the command-ingestion stage of that RDP.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor/Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//
// M6 gate: Rust `Pipeline::dispatch(Stage::Ingest)` traps under `vulkan-rdp`
// until the 1× frame hash matches CpuRdpLle. This entry writes nothing.
//
// SyncPipe (0x27), SyncTile (0x28), SyncLoad (0x26), and SyncFull (0x29)
// each keep their own barrier kind. A later Full does not replace an earlier
// Pipe, Tile, or Load.

struct CmdWord {
  hi: u32,
  lo: u32,
}

struct Barrier {
  // 1 = Pipe, 2 = Tile, 3 = Load, 4 = Full.
  kind: u32,
  word_index: u32,
}

@group(0) @binding(0) var<storage, read> commands: array<CmdWord>;
@group(0) @binding(1) var<storage, read_write> barriers: array<Barrier>;

@compute @workgroup_size(64)
fn ingest_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let index = id.x;
  if index >= arrayLength(&commands) || index >= arrayLength(&barriers) {
    return;
  }
}
