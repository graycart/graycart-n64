//! EdgeWalker compute pipeline.
//!
//! EdgeWalker is Graycart's bespoke Vulkan compute LLE RDP. Command-stream
//! ingestion, screen-tile binning, the span pass (TMEM, color combiner,
//! blender, 9th-bit coverage, and Z), and VI scan-out are the stages of that
//! RDP.
//!
//! **M6 gate:** this GPU path becomes the default backend only after its 1×
//! frame hash matches [`crate::rdp::CpuRdpLle`] bit-for-bit. Until that
//! receipt, [`super::EdgeWalker::submit`] and [`super::EdgeWalker::sync`]
//! stay on the CPU parity pipe. [`Pipeline::dispatch`] traps when the
//! `vulkan-rdp` feature is on. With the feature off it returns without
//! writing pixels, so default CI stays GPU-free.
//!
//! [`SnapshotRing`] stores `SyncPipe`, `SyncTile`, `SyncLoad`, and `SyncFull`
//! as four separate barriers, in arrival order. Span dither and noise, once
//! the pass exists, use a deterministic per-span seed so GPU workgroups
//! reproduce the serial [`crate::rdp::CpuRdpLle`] hash.
//!
//! Bespoke WGSL for each stage lives in `shaders/`. The `vulkan-rdp` feature
//! embeds those sources. It does not link `wgpu` yet, and it does not run
//! them. No third-party RDP shader is included.
//!
//! Cited: N64brew — Reality Display Processor / Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//! Cited: N64brew — Video Interface
//!   https://n64brew.dev/wiki/Video_Interface
//! Cross-check: docs/ORCHESTRATOR-HANDOFF.md (Milestone M6)

use crate::rdp::RdpSyncKind;
use crate::trap::Trap;

/// `Sync Load`. Command id bits [61:56]; the top two bits of the byte are ignored.
const CMD_SYNC_LOAD: u8 = 0x26;
/// `Sync Pipe`.
const CMD_SYNC_PIPE: u8 = 0x27;
/// `Sync Tile`.
const CMD_SYNC_TILE: u8 = 0x28;
/// `Sync Full`.
const CMD_SYNC_FULL: u8 = 0x29;

/// One EdgeWalker compute stage.
///
/// The discriminant is the [`Trap::UnimplementedRegister`] offset while the
/// stage is still a stub. It is not a DPC or DPS register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Stage {
    /// Command upload from the snapshot ring.
    Ingest = 0x10,
    /// Primitive bounds to screen-bin lists.
    TileBin = 0x11,
    /// Span raster, TMEM, combiner, blender, coverage, and Z.
    Span = 0x12,
    /// VI scan-out to RGBA8888.
    Vi = 0x13,
}

impl Stage {
    /// Trap offset for this stage. Not a hardware MMIO address.
    #[must_use]
    pub const fn trap_offset(self) -> u32 {
        self as u32
    }
}

/// WGSL source for `stage`. Present only when `vulkan-rdp` is enabled.
///
/// The text is the bespoke EdgeWalker shader for that stage. Dispatch still
/// traps; nothing in this module submits the source to a device.
#[cfg(feature = "vulkan-rdp")]
#[must_use]
pub fn shader_wgsl(stage: Stage) -> &'static str {
    match stage {
        Stage::Ingest => include_str!("shaders/ingest.wgsl"),
        Stage::TileBin => include_str!("shaders/tile_bin.wgsl"),
        Stage::Span => include_str!("shaders/span.wgsl"),
        Stage::Vi => include_str!("shaders/vi.wgsl"),
    }
}

/// Map one command word onto its sync barrier.
///
/// `Pipe`, `Tile`, `Load`, and `Full` each return their own variant.
#[must_use]
pub fn sync_kind(word: u64) -> Option<RdpSyncKind> {
    match ((word >> 56) & 0x3F) as u8 {
        CMD_SYNC_PIPE => Some(RdpSyncKind::Pipe),
        CMD_SYNC_TILE => Some(RdpSyncKind::Tile),
        CMD_SYNC_LOAD => Some(RdpSyncKind::Load),
        CMD_SYNC_FULL => Some(RdpSyncKind::Full),
        _ => None,
    }
}

/// Commands between two sync barriers.
///
/// `barrier` is the opcode that closed this snapshot. A later `SyncFull` does
/// not rewrite an earlier `Pipe`, `Tile`, or `Load` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Command words since the previous barrier, excluding the barrier word.
    pub words: Vec<u64>,
    /// Barrier that closed this snapshot.
    pub barrier: RdpSyncKind,
    /// Raw barrier command word (opcode plus payload).
    pub barrier_word: u64,
}

/// Host-side command ring for the EdgeWalker ingest stage.
///
/// Recording is bookkeeping for the compute upload. It does not rasterize and
/// it does not write RDRAM. Pixel output stays on the CPU parity pipe until
/// the GPU hash matches [`crate::rdp::CpuRdpLle`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnapshotRing {
    closed: Vec<Snapshot>,
    open: Vec<u64>,
}

impl SnapshotRing {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `cmds`, closing a snapshot at each sync barrier.
    ///
    /// Returns [`Ok`] after the words are stored. This does not execute the
    /// commands. Unimplemented opcodes still trap on
    /// [`super::EdgeWalker::submit`], which is the hash path.
    pub fn ingest(&mut self, cmds: &[u64]) -> Result<(), Trap> {
        for &word in cmds {
            if let Some(barrier) = sync_kind(word) {
                self.closed.push(Snapshot {
                    words: std::mem::take(&mut self.open),
                    barrier,
                    barrier_word: word,
                });
            } else {
                self.open.push(word);
            }
        }
        Ok(())
    }

    /// Snapshots closed by a sync barrier, in order.
    #[must_use]
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.closed
    }

    /// Words not yet closed by a barrier.
    #[must_use]
    pub fn open_words(&self) -> &[u64] {
        &self.open
    }
}

/// One screen-space bin of primitive indices.
///
/// An EdgeWalker compute bin. The eight RDP texture tiles (`Set Tile`) are a
/// separate structure and stay in the CPU parity state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScreenBin {
    pub x: u32,
    pub y: u32,
    pub primitive_ids: Vec<u32>,
}

/// Tile-binning pass output: primitive ids per screen bin.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileBinPass {
    pub bins: Vec<ScreenBin>,
}

/// GPU-side color, 9th-bit coverage, and depth image.
///
/// `coverage` matches the RDRAM hidden-bit side array: one byte per color
/// byte, `0` or `1`. The span pass fills these once it matches
/// [`crate::rdp::CpuRdpLle`]. The stub leaves them empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MirroredRdram {
    pub color: Vec<u8>,
    pub coverage: Vec<u8>,
    pub depth: Vec<u8>,
}

/// VI scan-out target. RGBA8888, matching the CPU oracle's scan-out hash.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViFramebuffer {
    pub width: u32,
    pub height: u32,
    pub rgba8888: Vec<u8>,
}

/// EdgeWalker compute pipeline.
///
/// Holds the snapshot ring. The four [`dispatch`](Pipeline::dispatch) stages
/// are the GPU entry points and do not replace [`super::EdgeWalker::submit`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipeline {
    ring: SnapshotRing,
}

impl Pipeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `cmds` into the snapshot ring.
    pub fn ingest(&mut self, cmds: &[u64]) -> Result<(), Trap> {
        self.ring.ingest(cmds)
    }

    /// Closed snapshots, oldest first.
    #[must_use]
    pub fn snapshots(&self) -> &[Snapshot] {
        self.ring.snapshots()
    }

    /// Words waiting for a closing barrier.
    #[must_use]
    pub fn open_words(&self) -> &[u64] {
        self.ring.open_words()
    }

    /// Run one compute stage.
    ///
    /// `vulkan-rdp`: traps. The WGSL stub is loaded and then refused so a
    /// partial shader cannot write a frame.
    /// Feature off: `TileBin` yields an empty [`TileBinPass`]; the other
    /// stages leave their buffers untouched.
    pub fn dispatch(&self, stage: Stage) -> Result<DispatchOutput, Trap> {
        let _ = self.snapshots().len() + self.open_words().len();
        gpu_unimplemented(stage)?;
        Ok(match stage {
            Stage::Ingest | Stage::Span | Stage::Vi => DispatchOutput::Done,
            Stage::TileBin => DispatchOutput::Bins(TileBinPass::default()),
        })
    }

    /// Span-pass entry. `image` is the RDRAM mirror the shader will own.
    ///
    /// The stub does not write `image`. The `&mut` borrow is the write
    /// interface the real pass will use.
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn dispatch_span(&self, image: &mut MirroredRdram) -> Result<(), Trap> {
        let _ = (
            self.snapshots().len(),
            image.color.len(),
            image.coverage.len(),
            image.depth.len(),
        );
        gpu_unimplemented(Stage::Span)
    }

    /// VI entry. `out` is the RGBA8888 scan-out buffer.
    ///
    /// The stub does not write `out`. The `&mut` borrow is the write
    /// interface the real pass will use. Hash parity with the CPU VI path
    /// is required before this pass feeds presentation.
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn dispatch_vi(&self, image: &MirroredRdram, out: &mut ViFramebuffer) -> Result<(), Trap> {
        let _ = (
            self.snapshots().len(),
            image.color.len(),
            image.coverage.len(),
            image.depth.len(),
            out.width,
            out.height,
            out.rgba8888.len(),
        );
        gpu_unimplemented(Stage::Vi)
    }
}

/// Result of a feature-off [`Pipeline::dispatch`].
///
/// `Bins` is empty. `Done` means the stage wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutput {
    /// Ingest, span, or VI stage accepted and wrote nothing.
    Done,
    /// Tile bin stage accepted and produced no bins.
    Bins(TileBinPass),
}

fn gpu_unimplemented(stage: Stage) -> Result<(), Trap> {
    #[cfg(feature = "vulkan-rdp")]
    {
        // Embed the bespoke source in the feature build, then trap.
        // A stub shader must not run.
        let _ = shader_wgsl(stage).len();
        Err(stage_trap(stage))
    }
    #[cfg(not(feature = "vulkan-rdp"))]
    {
        let _ = stage;
        Ok(())
    }
}

#[cfg(feature = "vulkan-rdp")]
fn stage_trap(stage: Stage) -> Trap {
    Trap::UnimplementedRegister {
        device: "EdgeWalker",
        offset: stage.trap_offset(),
        is_write: true,
    }
}
