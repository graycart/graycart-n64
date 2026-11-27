// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// Command-ingestion stage: one barrier record per command word.
// Bespoke Graycart shader. No third-party RDP shader source.
//
// Cited: N64brew — Reality Display Processor/Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//   Sync Load 0x26, Sync Pipe 0x27, Sync Tile 0x28, Sync Full 0x29.
//   Opcode is bits [61:56] of the 64-bit word (low 6 bits of the top byte).
//
// Entry point: ingest_main
// Workgroup: 64. Dispatch ceil(command_count / 64) workgroups.
// One thread per command index. In-range threads always write `barriers[index]`.
//
// Host packing of one RDP command word (big-endian style, not GPU-endian):
//   hi = bits [63:32], lo = bits [31:0]
//   i.e. hi = (word >> 32) as u32, lo = word as u32.
//
// @group(0) binding table (all storage, std430-style, no extra padding):
//   binding 0  commands  read        array<CmdWord>   stride 8
//   binding 1  barriers  read_write  array<Barrier>   stride 8
//
// CmdWord  offset 0 hi:u32, offset 4 lo:u32
// Barrier  offset 0 kind:u32, offset 4 word_index:u32
//   kind 0 = not a sync
//   kind 1 = Sync Pipe  (opcode 0x27)
//   kind 2 = Sync Tile  (opcode 0x28)
//   kind 3 = Sync Load  (opcode 0x26)
//   kind 4 = Sync Full  (opcode 0x29)
// A later Full does not rewrite an earlier Pipe, Tile, or Load. Each index
// keeps the kind of that word alone. `word_index` is the command index.
//
// Layout vs the previous stub: same structs and the same two bindings.
// The stub returned before the store. This entry writes both fields.

struct CmdWord {
  hi: u32,
  lo: u32,
}

struct Barrier {
  // 0 = none, 1 = Pipe, 2 = Tile, 3 = Load, 4 = Full.
  kind: u32,
  word_index: u32,
}

@group(0) @binding(0) var<storage, read> commands: array<CmdWord>;
@group(0) @binding(1) var<storage, read_write> barriers: array<Barrier>;

fn sync_kind(hi: u32) -> u32 {
  let op = (hi >> 24u) & 0x3Fu;
  if op == 0x27u {
    return 1u;
  }
  if op == 0x28u {
    return 2u;
  }
  if op == 0x26u {
    return 3u;
  }
  if op == 0x29u {
    return 4u;
  }
  return 0u;
}

@compute @workgroup_size(64)
fn ingest_main(@builtin(global_invocation_id) id: vec3<u32>) {
  let index = id.x;
  if index >= arrayLength(&commands) || index >= arrayLength(&barriers) {
    return;
  }
  barriers[index].kind = sync_kind(commands[index].hi);
  barriers[index].word_index = index;
}
