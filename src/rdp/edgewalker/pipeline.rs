//! EdgeWalker compute pipeline.
//!
//! EdgeWalker is Graycart's bespoke Vulkan compute LLE RDP. Command-stream
//! ingestion, screen-tile binning, the span pass (TMEM, color combiner,
//! blender, 9th-bit coverage, and Z), and VI scan-out are the stages of that
//! RDP.
//!
//! EdgeWalker is Graycart's standalone bespoke Vulkan compute LLE RDP,
//! written directly from N64 RDP hardware specs with zero CPU fallback.
//! Without the `vulkan-rdp` feature, all dispatch paths trap cleanly.
//! **M6 gate:** 1× frame hash matches [`crate::rdp::CpuRdpLle`] bit-for-bit.
//!
//! With the `vulkan-rdp` feature, `dispatch_ingest`, `dispatch_tile_bin`,
//! `dispatch_span`, and `dispatch_vi` upload storage buffers, run the bespoke
//! WGSL entry point on `ComputeDevice`, and read the results back. Without
//! that feature those methods trap and do not write pixels.
//! [`SnapshotRing`] remains a host recorder. `dispatch_ingest` appends the
//! shader's barrier list onto it after a successful readback.
//!
//! Span dither and noise, once the pass exists, use a deterministic per-span
//! seed so GPU workgroups reproduce the serial [`crate::rdp::CpuRdpLle`] hash.
//! This wave's span shader is FILL-cycle RGBA16 only. Any other cycle or
//! color-image size traps here, before a dispatch.
//!
//! Bespoke WGSL for each stage lives in `shaders/`. The `vulkan-rdp` feature
//! embeds those sources and submits them. No third-party RDP shader is included.
//!
//! Cited: N64brew — Reality Display Processor / Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//! Cited: N64brew — Video Interface
//!   https://n64brew.dev/wiki/Video_Interface
//! Cross-check: docs/ORCHESTRATOR-HANDOFF.md (Milestone M6)
//! Cross-check: `shaders/span.wgsl` `SpanParams` (stride 80)
//! Cross-check: `src/rdp/raster.rs` FILL rectangle range (not copied)

use crate::rdp::RdpSyncKind;
use crate::trap::Trap;

#[cfg(feature = "vulkan-rdp")]
use super::ComputeDevice;

/// `Sync Load`. Command id bits [61:56]; the top two bits of the byte are ignored.
const CMD_SYNC_LOAD: u8 = 0x26;
/// `Sync Pipe`.
const CMD_SYNC_PIPE: u8 = 0x27;
/// `Sync Tile`.
const CMD_SYNC_TILE: u8 = 0x28;
/// `Sync Full`.
const CMD_SYNC_FULL: u8 = 0x29;

/// Compute workgroup size of every EdgeWalker stage (`@workgroup_size(64)`).
pub const WORKGROUP_SIZE: u32 = 64;

/// Host and GPU stride of one ingested command word (`CmdWord`: `hi`, `lo`).
pub const INGEST_WORD_STRIDE: usize = 8;
/// Host and GPU stride of one ingest barrier (`kind`, `word_index`).
pub const INGEST_BARRIER_STRIDE: usize = 8;

/// `Barrier.kind` when the word is not a sync.
pub const INGEST_KIND_NONE: u32 = 0;
/// `Barrier.kind` for Sync Pipe (opcode `0x27`).
pub const INGEST_KIND_PIPE: u32 = 1;
/// `Barrier.kind` for Sync Tile (opcode `0x28`).
pub const INGEST_KIND_TILE: u32 = 2;
/// `Barrier.kind` for Sync Load (opcode `0x26`).
pub const INGEST_KIND_LOAD: u32 = 3;
/// `Barrier.kind` for Sync Full (opcode `0x29`).
pub const INGEST_KIND_FULL: u32 = 4;

/// GPU `PrimBounds` stride. Half-open pixel box `x0,y0,x1,y1` as little-endian `i32`.
pub const PRIM_BOUNDS_STRIDE: usize = 16;
/// GPU `BinPair` stride. `bin_id`, `prim_id` as little-endian `u32`.
pub const BIN_PAIR_STRIDE: usize = 8;
/// GPU `BinGrid` stride. Eight little-endian `u32`s; the last three are padding.
pub const BIN_GRID_STRIDE: usize = 32;

/// Set Other Modes cycle type FILL. Bits [53:52] of `other_modes`, stored in
/// `other_modes_hi` bits [21:20].
pub const CYCLE_TYPE_FILL: u32 = 3;
/// Set Color Image size 2 (RGBA16 / 16-bit).
pub const COLOR_IMAGE_SIZE_16: u32 = 2;
/// VI color type 2 (RGBA16). Any other type traps in `dispatch_vi`.
pub const VI_COLOR_RGBA16: u32 = 2;

/// Byte size of one `SpanParams` element. See [`SpanParamsPack`].
pub const SPAN_PARAMS_STRIDE: usize = 80;
/// Byte size of one `ViParams` element. See [`ViParamsPack`].
pub const VI_PARAMS_STRIDE: usize = 16;

#[cfg(feature = "vulkan-rdp")]
const TMEM_BYTES: usize = 4096;
#[cfg(feature = "vulkan-rdp")]
const BIN_SENTINEL: u32 = 0xFFFF_FFFF;

const _: () = assert!(SPAN_PARAMS_STRIDE == 80);
const _: () = assert!(VI_PARAMS_STRIDE == 16);
const _: () = assert!(BIN_GRID_STRIDE == 32);
const _: () = assert!(PRIM_BOUNDS_STRIDE == 16);
const _: () = assert!(INGEST_WORD_STRIDE == 8);
const _: () = assert!(INGEST_BARRIER_STRIDE == 8);

/// One EdgeWalker compute stage.
///
/// The discriminant is the [`Trap::UnimplementedRegister`] offset when this
/// stage refuses a dispatch. It is not a DPC or DPS register. A device
/// failure inside `ComputeDevice` uses offset `0x0001` instead.
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

    #[cfg(feature = "vulkan-rdp")]
    const fn entry_point(self) -> &'static str {
        match self {
            Self::Ingest => "ingest_main",
            Self::TileBin => "tile_bin_main",
            Self::Span => "span_main",
            Self::Vi => "vi_main",
        }
    }
}

/// WGSL source for `stage`. Present only when `vulkan-rdp` is enabled.
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
/// [`Pipeline::ingest`] records words on the host. [`Pipeline::dispatch_ingest`]
/// records the same shape from the GPU barrier buffer. Neither path rasterizes.
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
///
/// The host builds one [`ScreenBin`] for every bin in the grid, including bins
/// with no primitives. `x = id % bins_x`, `y = id / bins_x`. Inside a bin,
/// primitive ids follow increasing primitive order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileBinPass {
    pub bins: Vec<ScreenBin>,
}

/// Half-open pixel AABB uploaded as `PrimBounds` (stride [`PRIM_BOUNDS_STRIDE`]).
///
/// The host converts an RDP rectangle to this box before upload. Coordinates
/// below 0 are clipped by the shader. An empty box (`x1 <= x0` or `y1 <= y0`)
/// overlaps no bins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimBounds {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl PrimBounds {
    /// Little-endian `x0, y0, x1, y1`.
    #[must_use]
    pub fn to_bytes(self) -> [u8; PRIM_BOUNDS_STRIDE] {
        let mut out = [0u8; PRIM_BOUNDS_STRIDE];
        write_i32s(&mut out, &[self.x0, self.y0, self.x1, self.y1]);
        out
    }
}

/// Screen grid uploaded as `BinGrid` element 0 (stride [`BIN_GRID_STRIDE`]).
///
/// Byte layout:
/// `0 bins_x`, `4 bins_y`, `8 bin_size`, `12 prim_count`,
/// `16 max_bins_per_prim`, `20 _pad0`, `24 _pad1`, `28 _pad2`.
/// `prim_count` is the primitive slice length, not a field of this struct.
/// `bin_size` is the bin edge in pixels (EdgeWalker default 32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinGridParams {
    pub bins_x: u32,
    pub bins_y: u32,
    pub bin_size: u32,
    pub max_bins_per_prim: u32,
}

impl BinGridParams {
    /// Little-endian grid words. Pads at bytes 20, 24, and 28 are zero.
    #[must_use]
    pub fn to_bytes(self, prim_count: u32) -> [u8; BIN_GRID_STRIDE] {
        let mut out = [0u8; BIN_GRID_STRIDE];
        write_u32s(
            &mut out,
            &[
                self.bins_x,
                self.bins_y,
                self.bin_size,
                prim_count,
                self.max_bins_per_prim,
                0,
                0,
                0,
            ],
        );
        out
    }
}

/// GPU-side color, 9th-bit coverage, and depth image.
///
/// `coverage` matches the RDRAM hidden-bit side array: one byte per color
/// byte, `0` or `1`. The span binding expands each byte to a `u32` whose low
/// 8 bits hold that value. FILL does not update coverage or depth.
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

/// `ViParams` element 0. Stride [`VI_PARAMS_STRIDE`].
///
/// Byte layout: `0 origin`, `4 width`, `8 height`, `12 color_type`.
/// `origin` is a byte offset into the color mirror. `width` is `VI_WIDTH`.
/// `height` is the scan-out row count. `color_type` 2 is RGBA16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViParamsPack {
    pub origin: u32,
    pub width: u32,
    pub height: u32,
    pub color_type: u32,
}

impl ViParamsPack {
    /// Little-endian `origin, width, height, color_type`.
    #[must_use]
    pub fn to_bytes(self) -> [u8; VI_PARAMS_STRIDE] {
        let mut out = [0u8; VI_PARAMS_STRIDE];
        write_u32s(
            &mut out,
            &[self.origin, self.width, self.height, self.color_type],
        );
        out
    }
}

/// `SpanParams` element 0. Stride [`SPAN_PARAMS_STRIDE`] (80).
///
/// Little-endian `u32` words, matching `shaders/span.wgsl`:
///
/// | Bytes | Field | Notes |
/// | --- | --- | --- |
/// | 0 | `other_modes_lo` | latched other-modes bits [31:0] |
/// | 4 | `other_modes_hi` | bits [63:32]; cycle type is bits [21:20] |
/// | 8 | `combine_lo` | unused on FILL |
/// | 12 | `combine_hi` | unused on FILL |
/// | 16 | `fill_color` | even x uses [31:16], odd x uses [15:0] |
/// | 20 | `color_dram_addr` | byte offset into the color mirror; must be even |
/// | 24 | `color_width_px` | pixels per row, already `(width field + 1)` |
/// | 28 | `color_size` | 2 = 16-bit; any other size traps before dispatch |
/// | 32 | `rect_ulx` | s10.2; the shader masks to 12 bits |
/// | 36 | `rect_uly` | s10.2 |
/// | 40 | `rect_lrx` | s10.2 |
/// | 44 | `rect_lry` | s10.2 |
/// | 48 | `scissor_ulx` | latched scissor, low 16 bits used |
/// | 52 | `scissor_uly` | |
/// | 56 | `scissor_lrx` | right edge inclusive after `>> 2` |
/// | 60 | `scissor_lry` | bottom edge exclusive after `>> 2` |
/// | 64 | `scissor_field` | 0 = all lines; nonzero = interlace |
/// | 68 | `scissor_odd` | nonzero = keep odd y when field is set |
/// | 72 | `_pad0` | zero |
/// | 76 | `_pad1` | zero |
///
/// FILL pixel range, same rule as `paint_rect`: inclusive
/// `x = (ulx >> 2) ..= ceil_10_2(lrx)` and the same for y. `ceil_10_2(v)` is
/// `v >> 2`, plus one when the low 2 bits are not 0. 12-bit fields are not
/// sign-extended. One dispatch is one rectangle (`params[0]`).
///
/// Cycle type other than FILL (3), or a color-image size other than 2, traps
/// in [`Pipeline::dispatch_span`] before any upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanParamsPack {
    pub other_modes_lo: u32,
    pub other_modes_hi: u32,
    pub combine_lo: u32,
    pub combine_hi: u32,
    pub fill_color: u32,
    pub color_dram_addr: u32,
    pub color_width_px: u32,
    pub color_size: u32,
    pub rect_ulx: u32,
    pub rect_uly: u32,
    pub rect_lrx: u32,
    pub rect_lry: u32,
    pub scissor_ulx: u32,
    pub scissor_uly: u32,
    pub scissor_lrx: u32,
    pub scissor_lry: u32,
    pub scissor_field: u32,
    pub scissor_odd: u32,
}

impl SpanParamsPack {
    /// Pack the 80-byte `SpanParams` element. Padding words are zero.
    #[must_use]
    pub fn to_bytes(self) -> [u8; SPAN_PARAMS_STRIDE] {
        let mut out = [0u8; SPAN_PARAMS_STRIDE];
        write_u32s(
            &mut out,
            &[
                self.other_modes_lo,
                self.other_modes_hi,
                self.combine_lo,
                self.combine_hi,
                self.fill_color,
                self.color_dram_addr,
                self.color_width_px,
                self.color_size,
                self.rect_ulx,
                self.rect_uly,
                self.rect_lrx,
                self.rect_lry,
                self.scissor_ulx,
                self.scissor_uly,
                self.scissor_lrx,
                self.scissor_lry,
                self.scissor_field,
                self.scissor_odd,
                0,
                0,
            ],
        );
        out
    }

    /// Decode one 80-byte element. A different length traps as [`Stage::Span`].
    ///
    /// Padding bytes are ignored. [`Self::to_bytes`] writes them as zero.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Trap> {
        if bytes.len() != SPAN_PARAMS_STRIDE {
            return Err(stage_trap(Stage::Span));
        }
        let w = |offset| read_u32(bytes, offset, Stage::Span);
        Ok(Self {
            other_modes_lo: w(0)?,
            other_modes_hi: w(4)?,
            combine_lo: w(8)?,
            combine_hi: w(12)?,
            fill_color: w(16)?,
            color_dram_addr: w(20)?,
            color_width_px: w(24)?,
            color_size: w(28)?,
            rect_ulx: w(32)?,
            rect_uly: w(36)?,
            rect_lrx: w(40)?,
            rect_lry: w(44)?,
            scissor_ulx: w(48)?,
            scissor_uly: w(52)?,
            scissor_lrx: w(56)?,
            scissor_lry: w(60)?,
            scissor_field: w(64)?,
            scissor_odd: w(68)?,
        })
    }

    /// Cycle type from `other_modes` bits [53:52].
    #[must_use]
    pub const fn cycle_type(self) -> u32 {
        (self.other_modes_hi >> 20) & 3
    }

    /// FILL cycle and 16-bit color image. Other combinations trap before dispatch.
    #[must_use]
    pub const fn is_fill_rgba16(self) -> bool {
        self.cycle_type() == CYCLE_TYPE_FILL && self.color_size == COLOR_IMAGE_SIZE_16
    }

    /// Inclusive FILL pixel count for the workgroup dispatch.
    ///
    /// `0` means the rectangle is empty (`x0 > x1` or `y0 > y1`) and the
    /// shader stores nothing. The count is `rect_w * rect_h` with
    /// `rect_w = x1 - x0 + 1` after the s10.2 conversion above.
    /// Overflow of that product traps as [`Stage::Span`].
    ///
    /// Smoke (`lrx = 4`, other rect fields 0): `ceil_10_2(4) = 1`, so the
    /// inclusive range is `(0,0)` and `(1,0)` — two pixels.
    pub fn fill_pixel_count(self) -> Result<u32, Trap> {
        let x0 = (self.rect_ulx & 0xFFF) >> 2;
        let y0 = (self.rect_uly & 0xFFF) >> 2;
        let x1 = ceil_10_2(self.rect_lrx & 0xFFF);
        let y1 = ceil_10_2(self.rect_lry & 0xFFF);
        if x0 > x1 || y0 > y1 {
            return Ok(0);
        }
        let rect_w = x1 - x0 + 1;
        let rect_h = y1 - y0 + 1;
        if rect_w == 0 || rect_h > u32::MAX / rect_w {
            return Err(stage_trap(Stage::Span));
        }
        Ok(rect_w * rect_h)
    }
}

/// EdgeWalker compute pipeline.
///
/// Holds the snapshot ring. The `dispatch_*` methods are the GPU entry points
/// and do not replace [`super::EdgeWalker::submit`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipeline {
    ring: SnapshotRing,
}

impl Pipeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `cmds` into the snapshot ring on the host.
    ///
    /// This does not submit a compute pass. Use [`Self::dispatch_ingest`] to
    /// classify the same words on the GPU.
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

    /// Run `ingest_main` and append its barriers to the snapshot ring.
    ///
    /// Each returned pair is `(kind, word_index)` in command order.
    /// `kind` is [`INGEST_KIND_NONE`], [`INGEST_KIND_PIPE`], [`INGEST_KIND_TILE`],
    /// [`INGEST_KIND_LOAD`], or [`INGEST_KIND_FULL`].
    ///
    /// Bindings: 0 command words (`hi = bits[63:32]`, `lo = bits[31:0]`),
    /// 1 barrier records. An empty slice returns an empty list and does not
    /// touch the ring. A kind or index the shader contract does not allow
    /// traps, and the ring is left unchanged.
    #[cfg(feature = "vulkan-rdp")]
    pub fn dispatch_ingest(
        &mut self,
        device: &ComputeDevice,
        commands: &[u64],
    ) -> Result<Vec<(u32, u32)>, Trap> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }
        let pipeline = pipeline_for(device, Stage::Ingest)?;
        self.dispatch_ingest_with_pipeline(device, &pipeline, commands)
    }

    /// Run `ingest_main` using a caller-supplied cached [`wgpu::ComputePipeline`]
    /// and append its barriers to the snapshot ring.
    #[cfg(feature = "vulkan-rdp")]
    pub fn dispatch_ingest_with_pipeline(
        &mut self,
        device: &ComputeDevice,
        pipeline: &wgpu::ComputePipeline,
        commands: &[u64],
    ) -> Result<Vec<(u32, u32)>, Trap> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }
        let count = u32::try_from(commands.len()).map_err(|_| stage_trap(Stage::Ingest))?;
        let packed = pack_commands(commands)?;
        let command_buf = upload(device, Stage::Ingest, &packed)?;
        let barrier_buf = upload_zeros(device, Stage::Ingest, packed.len())?;
        let group = device.create_storage_bind_group(
            pipeline,
            0,
            &[(0, &command_buf), (1, &barrier_buf)],
        )?;
        dispatch_threads(device, Stage::Ingest, pipeline, &group, count)?;
        let raw = read_exact(device, Stage::Ingest, &barrier_buf, packed.len())?;
        let pairs = unpack_pairs(&raw, Stage::Ingest)?;
        let classified = classify_barriers(commands, &pairs)?;
        append_classified(&mut self.ring, classified);
        Ok(pairs)
    }

    /// GPU ingest is unavailable. The ring is not modified.
    #[cfg(not(feature = "vulkan-rdp"))]
    pub fn dispatch_ingest(&mut self, commands: &[u64]) -> Result<Vec<(u32, u32)>, Trap> {
        let _ = (self, commands);
        Err(stage_trap(Stage::Ingest))
    }

    /// Run `tile_bin_main` and gather a [`TileBinPass`].
    ///
    /// Bindings: 0 `PrimBounds`, 1 `BinPair`, 2 `BinGrid`, 3 overlap counts.
    /// A primitive whose overlap is greater than `max_bins_per_prim` traps
    /// (the shader would drop the extra bins). A zero-area grid returns an
    /// empty pass after the dispatch.
    #[cfg(feature = "vulkan-rdp")]
    pub fn dispatch_tile_bin(
        &self,
        device: &ComputeDevice,
        prims: &[PrimBounds],
        grid: &BinGridParams,
    ) -> Result<TileBinPass, Trap> {
        let prim_count = u32::try_from(prims.len()).map_err(|_| stage_trap(Stage::TileBin))?;
        if prim_count > 1 && grid.max_bins_per_prim > 0 {
            let last = prim_count - 1;
            if last > u32::MAX / grid.max_bins_per_prim {
                return Err(stage_trap(Stage::TileBin));
            }
        }
        let prim_bytes = if prims.is_empty() {
            vec![0u8; PRIM_BOUNDS_STRIDE]
        } else {
            pack_prims(prims)?
        };
        let count_bytes = if prim_count == 0 {
            4
        } else {
            prims
                .len()
                .checked_mul(4)
                .ok_or_else(|| stage_trap(Stage::TileBin))?
        };
        let pair_elems = if prim_count == 0 || grid.max_bins_per_prim == 0 {
            1
        } else {
            prims
                .len()
                .checked_mul(
                    usize::try_from(grid.max_bins_per_prim)
                        .map_err(|_| stage_trap(Stage::TileBin))?,
                )
                .ok_or_else(|| stage_trap(Stage::TileBin))?
        };
        let pair_bytes = pair_elems
            .checked_mul(BIN_PAIR_STRIDE)
            .ok_or_else(|| stage_trap(Stage::TileBin))?;
        let grid_bytes = grid.to_bytes(prim_count);

        let prim_buf = upload(device, Stage::TileBin, &prim_bytes)?;
        let pair_buf = upload_zeros(device, Stage::TileBin, pair_bytes)?;
        let grid_buf = upload(device, Stage::TileBin, &grid_bytes)?;
        let count_buf = upload_zeros(device, Stage::TileBin, count_bytes)?;
        let pipeline = pipeline_for(device, Stage::TileBin)?;
        let group = device.create_storage_bind_group(
            &pipeline,
            0,
            &[
                (0, &prim_buf),
                (1, &pair_buf),
                (2, &grid_buf),
                (3, &count_buf),
            ],
        )?;
        dispatch_threads(device, Stage::TileBin, &pipeline, &group, prim_count)?;

        let counts = if prim_count == 0 {
            Vec::new()
        } else {
            let raw = read_exact(device, Stage::TileBin, &count_buf, count_bytes)?;
            unpack_u32s(&raw, Stage::TileBin)?
        };
        let pairs = if prim_count == 0 || grid.max_bins_per_prim == 0 {
            Vec::new()
        } else {
            let raw = read_exact(device, Stage::TileBin, &pair_buf, pair_bytes)?;
            unpack_pairs(&raw, Stage::TileBin)?
        };
        gather_bins(grid, &counts, &pairs)
    }

    /// Tile binning is unavailable. No bins are invented.
    #[cfg(not(feature = "vulkan-rdp"))]
    pub fn dispatch_tile_bin(
        &self,
        prims: &[PrimBounds],
        grid: &BinGridParams,
    ) -> Result<TileBinPass, Trap> {
        let _ = (self, prims, grid);
        Err(stage_trap(Stage::TileBin))
    }

    /// Run `span_main` for one FILL rectangle.
    ///
    /// `params_bytes` is one [`SpanParamsPack`] (80 bytes). `color_mirror` is
    /// the RDRAM byte mirror (little-endian `u32` words on the device). The
    /// pass uploads it, dispatches `ceil(fill_pixels / 64)` workgroups, and
    /// copies the color buffer back. Non-empty `coverage` and `depth` are
    /// round-tripped the same way. FILL does not change them.
    ///
    /// Bindings: 0 params, 1 TMEM (4 KiB, zero on this FILL pass), 2 color,
    /// 3 coverage (`u32` per host byte, low 8 bits), 4 depth (raw words).
    ///
    /// A cycle other than FILL, or a color size other than 16-bit, traps
    /// before upload. A storing rectangle requires a non-empty color mirror
    /// whose length is a multiple of 4.
    #[cfg(feature = "vulkan-rdp")]
    pub fn dispatch_span(
        &self,
        device: &ComputeDevice,
        params_bytes: &[u8],
        color_mirror: &mut [u8],
        coverage: &mut [u8],
        depth: &mut [u8],
    ) -> Result<(), Trap> {
        let params = SpanParamsPack::from_bytes(params_bytes)?;
        if !params.is_fill_rgba16() {
            return Err(stage_trap(Stage::Span));
        }
        require_word_len(color_mirror, Stage::Span)?;
        require_word_len(depth, Stage::Span)?;
        let pixels = params.fill_pixel_count()?;
        let stores = params.color_width_px != 0 && (params.color_dram_addr & 1) == 0 && pixels > 0;
        if stores && color_mirror.is_empty() {
            return Err(stage_trap(Stage::Span));
        }

        let params_buf = upload(device, Stage::Span, params_bytes)?;
        let tmem_buf = upload_zeros(device, Stage::Span, TMEM_BYTES)?;
        let color_buf = if color_mirror.is_empty() {
            upload_zeros(device, Stage::Span, 4)?
        } else {
            upload(device, Stage::Span, color_mirror)?
        };
        let coverage_packed = pack_coverage(coverage)?;
        let coverage_buf = if coverage_packed.is_empty() {
            upload_zeros(device, Stage::Span, 4)?
        } else {
            upload(device, Stage::Span, &coverage_packed)?
        };
        let depth_buf = if depth.is_empty() {
            upload_zeros(device, Stage::Span, 4)?
        } else {
            upload(device, Stage::Span, depth)?
        };
        let pipeline = pipeline_for(device, Stage::Span)?;
        let group = device.create_storage_bind_group(
            &pipeline,
            0,
            &[
                (0, &params_buf),
                (1, &tmem_buf),
                (2, &color_buf),
                (3, &coverage_buf),
                (4, &depth_buf),
            ],
        )?;
        let threads = if stores { pixels } else { 0 };
        dispatch_threads(device, Stage::Span, &pipeline, &group, threads)?;

        if !color_mirror.is_empty() {
            let raw = read_exact(device, Stage::Span, &color_buf, color_mirror.len())?;
            color_mirror.copy_from_slice(&raw);
        }
        if !coverage.is_empty() {
            let raw = read_exact(device, Stage::Span, &coverage_buf, coverage_packed.len())?;
            unpack_coverage(&raw, coverage)?;
        }
        if !depth.is_empty() {
            let raw = read_exact(device, Stage::Span, &depth_buf, depth.len())?;
            depth.copy_from_slice(&raw);
        }
        Ok(())
    }

    /// Span dispatch is unavailable. `color_mirror` is left untouched.
    #[cfg(not(feature = "vulkan-rdp"))]
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn dispatch_span(
        &self,
        params_bytes: &[u8],
        color_mirror: &mut [u8],
        coverage: &mut [u8],
        depth: &mut [u8],
    ) -> Result<(), Trap> {
        let _ = (self, params_bytes, color_mirror, coverage, depth);
        Err(stage_trap(Stage::Span))
    }

    /// Run `vi_main` and fill `out.rgba8888`.
    ///
    /// Bindings: 0 color byte mirror, 1 coverage (one `u32` per host byte,
    /// not read on the REPLICATE path), 2 RGBA8888 words, 3 [`ViParamsPack`].
    /// Output bytes are R, G, B, A. `color_type` other than
    /// [`VI_COLOR_RGBA16`] traps and leaves `out` unchanged. Width or height
    /// 0 clears `out.rgba8888` and does not submit.
    ///
    /// A non-empty color mirror must be a multiple of 4 bytes so each GPU
    /// `u32` is four RDRAM bytes. An empty color mirror expands every pixel
    /// as a short read of 0.
    #[cfg(feature = "vulkan-rdp")]
    pub fn dispatch_vi(
        &self,
        device: &ComputeDevice,
        color: &[u8],
        coverage: &[u8],
        params: &ViParamsPack,
        out: &mut ViFramebuffer,
    ) -> Result<(), Trap> {
        if params.color_type != VI_COLOR_RGBA16 {
            return Err(stage_trap(Stage::Vi));
        }
        if params.width == 0 || params.height == 0 {
            out.width = params.width;
            out.height = params.height;
            out.rgba8888.clear();
            return Ok(());
        }
        require_word_len(color, Stage::Vi)?;
        let pixels = params
            .width
            .checked_mul(params.height)
            .ok_or_else(|| stage_trap(Stage::Vi))?;
        let byte_len = usize::try_from(u64::from(pixels) * 4).map_err(|_| stage_trap(Stage::Vi))?;

        let color_buf = if color.is_empty() {
            upload_zeros(device, Stage::Vi, 4)?
        } else {
            upload(device, Stage::Vi, color)?
        };
        let coverage_packed = pack_coverage(coverage).map_err(|_| stage_trap(Stage::Vi))?;
        let coverage_buf = if coverage_packed.is_empty() {
            upload_zeros(device, Stage::Vi, 4)?
        } else {
            upload(device, Stage::Vi, &coverage_packed)?
        };
        let rgba_buf = upload_zeros(device, Stage::Vi, byte_len)?;
        let vi_bytes = params.to_bytes();
        let vi_buf = upload(device, Stage::Vi, &vi_bytes)?;
        let pipeline = pipeline_for(device, Stage::Vi)?;
        let group = device.create_storage_bind_group(
            &pipeline,
            0,
            &[
                (0, &color_buf),
                (1, &coverage_buf),
                (2, &rgba_buf),
                (3, &vi_buf),
            ],
        )?;
        dispatch_threads(device, Stage::Vi, &pipeline, &group, pixels)?;
        let rgba = read_exact(device, Stage::Vi, &rgba_buf, byte_len)?;
        out.width = params.width;
        out.height = params.height;
        out.rgba8888 = rgba;
        Ok(())
    }

    /// VI dispatch is unavailable. `out` is left untouched.
    #[cfg(not(feature = "vulkan-rdp"))]
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn dispatch_vi(
        &self,
        color: &[u8],
        coverage: &[u8],
        params: &ViParamsPack,
        out: &mut ViFramebuffer,
    ) -> Result<(), Trap> {
        let _ = (self, color, coverage, params, out);
        Err(stage_trap(Stage::Vi))
    }
}

fn stage_trap(stage: Stage) -> Trap {
    Trap::UnimplementedRegister {
        device: "EdgeWalker",
        offset: stage.trap_offset(),
        is_write: true,
    }
}

/// `ceil` of an s10.2 value already masked to 12 bits. Matches `span.wgsl`.
fn ceil_10_2(v: u32) -> u32 {
    let whole = v >> 2;
    if v & 3 == 0 {
        whole
    } else {
        whole + 1
    }
}

fn write_u32s(dst: &mut [u8], words: &[u32]) {
    for (index, word) in words.iter().enumerate() {
        let start = index * 4;
        dst[start..start + 4].copy_from_slice(&word.to_le_bytes());
    }
}

fn write_i32s(dst: &mut [u8], words: &[i32]) {
    for (index, word) in words.iter().enumerate() {
        let start = index * 4;
        dst[start..start + 4].copy_from_slice(&word.to_le_bytes());
    }
}

fn read_u32(bytes: &[u8], offset: usize, stage: Stage) -> Result<u32, Trap> {
    let end = offset.checked_add(4).ok_or_else(|| stage_trap(stage))?;
    let word = bytes.get(offset..end).ok_or_else(|| stage_trap(stage))?;
    let mut buf = [0u8; 4];
    buf.copy_from_slice(word);
    Ok(u32::from_le_bytes(buf))
}

#[cfg(feature = "vulkan-rdp")]
fn require_word_len(bytes: &[u8], stage: Stage) -> Result<(), Trap> {
    if !bytes.is_empty() && !bytes.len().is_multiple_of(4) {
        return Err(stage_trap(stage));
    }
    Ok(())
}

#[cfg(feature = "vulkan-rdp")]
fn pack_coverage(bytes: &[u8]) -> Result<Vec<u8>, Trap> {
    let len = bytes
        .len()
        .checked_mul(4)
        .ok_or_else(|| stage_trap(Stage::Span))?;
    let mut out = Vec::new();
    out.try_reserve(len).map_err(|_| stage_trap(Stage::Span))?;
    for &byte in bytes {
        out.extend_from_slice(&u32::from(byte).to_le_bytes());
    }
    Ok(out)
}

#[cfg(feature = "vulkan-rdp")]
fn unpack_coverage(packed: &[u8], dst: &mut [u8]) -> Result<(), Trap> {
    let expect = dst
        .len()
        .checked_mul(4)
        .ok_or_else(|| stage_trap(Stage::Span))?;
    if packed.len() != expect {
        return Err(stage_trap(Stage::Span));
    }
    for (index, slot) in dst.iter_mut().enumerate() {
        *slot = packed[index * 4];
    }
    Ok(())
}

#[cfg(feature = "vulkan-rdp")]
fn pipeline_for(device: &ComputeDevice, stage: Stage) -> Result<wgpu::ComputePipeline, Trap> {
    device.create_compute_pipeline(shader_wgsl(stage), stage.entry_point())
}

#[cfg(feature = "vulkan-rdp")]
fn upload(device: &ComputeDevice, stage: Stage, bytes: &[u8]) -> Result<wgpu::Buffer, Trap> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(stage_trap(stage));
    }
    let size = u64::try_from(bytes.len()).map_err(|_| stage_trap(stage))?;
    if size > device.device().limits().max_buffer_size {
        return Err(stage_trap(stage));
    }
    let buffer = device.create_storage_buffer(size)?;
    device.write_buffer(&buffer, 0, bytes)?;
    Ok(buffer)
}

#[cfg(feature = "vulkan-rdp")]
fn upload_zeros(device: &ComputeDevice, stage: Stage, len: usize) -> Result<wgpu::Buffer, Trap> {
    let mut bytes = Vec::new();
    bytes.try_reserve(len).map_err(|_| stage_trap(stage))?;
    bytes.resize(len, 0);
    upload(device, stage, &bytes)
}

#[cfg(feature = "vulkan-rdp")]
fn read_exact(
    device: &ComputeDevice,
    stage: Stage,
    buffer: &wgpu::Buffer,
    len: usize,
) -> Result<Vec<u8>, Trap> {
    let got = device.readback(buffer)?;
    if got.len() != len {
        return Err(stage_trap(stage));
    }
    Ok(got)
}

#[cfg(feature = "vulkan-rdp")]
fn dispatch_threads(
    device: &ComputeDevice,
    stage: Stage,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    threads: u32,
) -> Result<(), Trap> {
    let groups = if threads == 0 {
        1
    } else {
        threads.div_ceil(WORKGROUP_SIZE)
    };
    let limit = device
        .device()
        .limits()
        .max_compute_workgroups_per_dimension;
    if groups > limit {
        return Err(stage_trap(stage));
    }
    device.dispatch_workgroups(pipeline, Some(group), groups, 1, 1)
}

#[cfg(feature = "vulkan-rdp")]
fn pack_commands(commands: &[u64]) -> Result<Vec<u8>, Trap> {
    let len = commands
        .len()
        .checked_mul(INGEST_WORD_STRIDE)
        .ok_or_else(|| stage_trap(Stage::Ingest))?;
    let mut out = Vec::new();
    out.try_reserve(len)
        .map_err(|_| stage_trap(Stage::Ingest))?;
    for &word in commands {
        let hi = u32::try_from(word >> 32).map_err(|_| stage_trap(Stage::Ingest))?;
        let lo = u32::try_from(word & 0xFFFF_FFFF).map_err(|_| stage_trap(Stage::Ingest))?;
        out.extend_from_slice(&hi.to_le_bytes());
        out.extend_from_slice(&lo.to_le_bytes());
    }
    Ok(out)
}

#[cfg(feature = "vulkan-rdp")]
fn pack_prims(prims: &[PrimBounds]) -> Result<Vec<u8>, Trap> {
    let len = prims
        .len()
        .checked_mul(PRIM_BOUNDS_STRIDE)
        .ok_or_else(|| stage_trap(Stage::TileBin))?;
    let mut out = Vec::new();
    out.try_reserve(len)
        .map_err(|_| stage_trap(Stage::TileBin))?;
    for prim in prims {
        out.extend_from_slice(&prim.to_bytes());
    }
    Ok(out)
}

#[cfg(feature = "vulkan-rdp")]
fn unpack_u32s(bytes: &[u8], stage: Stage) -> Result<Vec<u32>, Trap> {
    let (chunks, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return Err(stage_trap(stage));
    }
    let mut out = Vec::new();
    out.try_reserve(chunks.len())
        .map_err(|_| stage_trap(stage))?;
    for chunk in chunks {
        out.push(u32::from_le_bytes(*chunk));
    }
    Ok(out)
}

#[cfg(feature = "vulkan-rdp")]
fn unpack_pairs(bytes: &[u8], stage: Stage) -> Result<Vec<(u32, u32)>, Trap> {
    let (chunks, rest) = bytes.as_chunks::<8>();
    if !rest.is_empty() {
        return Err(stage_trap(stage));
    }
    let mut out = Vec::new();
    out.try_reserve(chunks.len())
        .map_err(|_| stage_trap(stage))?;
    for chunk in chunks {
        out.push((read_u32(chunk, 0, stage)?, read_u32(chunk, 4, stage)?));
    }
    Ok(out)
}

#[cfg(feature = "vulkan-rdp")]
fn sync_from_gpu_kind(kind: u32) -> Result<Option<RdpSyncKind>, Trap> {
    match kind {
        INGEST_KIND_NONE => Ok(None),
        INGEST_KIND_PIPE => Ok(Some(RdpSyncKind::Pipe)),
        INGEST_KIND_TILE => Ok(Some(RdpSyncKind::Tile)),
        INGEST_KIND_LOAD => Ok(Some(RdpSyncKind::Load)),
        INGEST_KIND_FULL => Ok(Some(RdpSyncKind::Full)),
        _ => Err(stage_trap(Stage::Ingest)),
    }
}

#[cfg(feature = "vulkan-rdp")]
fn classify_barriers(
    commands: &[u64],
    barriers: &[(u32, u32)],
) -> Result<Vec<(u64, Option<RdpSyncKind>)>, Trap> {
    if barriers.len() != commands.len() {
        return Err(stage_trap(Stage::Ingest));
    }
    let mut out = Vec::new();
    out.try_reserve(commands.len())
        .map_err(|_| stage_trap(Stage::Ingest))?;
    for (index, (&word, &(kind, word_index))) in commands.iter().zip(barriers.iter()).enumerate() {
        let index_u32 = u32::try_from(index).map_err(|_| stage_trap(Stage::Ingest))?;
        if word_index != index_u32 {
            return Err(stage_trap(Stage::Ingest));
        }
        out.push((word, sync_from_gpu_kind(kind)?));
    }
    Ok(out)
}

#[cfg(feature = "vulkan-rdp")]
fn append_classified(ring: &mut SnapshotRing, classified: Vec<(u64, Option<RdpSyncKind>)>) {
    for (word, barrier) in classified {
        if let Some(barrier) = barrier {
            ring.closed.push(Snapshot {
                words: std::mem::take(&mut ring.open),
                barrier,
                barrier_word: word,
            });
        } else {
            ring.open.push(word);
        }
    }
}

#[cfg(feature = "vulkan-rdp")]
fn gather_bins(
    grid: &BinGridParams,
    counts: &[u32],
    pairs: &[(u32, u32)],
) -> Result<TileBinPass, Trap> {
    if grid.bins_x == 0 || grid.bins_y == 0 {
        if counts.iter().any(|&count| count != 0) {
            return Err(stage_trap(Stage::TileBin));
        }
        return Ok(TileBinPass::default());
    }
    let bins_x = usize::try_from(grid.bins_x).map_err(|_| stage_trap(Stage::TileBin))?;
    let bins_y = usize::try_from(grid.bins_y).map_err(|_| stage_trap(Stage::TileBin))?;
    let bin_count = bins_x
        .checked_mul(bins_y)
        .ok_or_else(|| stage_trap(Stage::TileBin))?;
    let mut bins = Vec::new();
    bins.try_reserve(bin_count)
        .map_err(|_| stage_trap(Stage::TileBin))?;
    for id in 0..bin_count {
        bins.push(ScreenBin {
            x: u32::try_from(id % bins_x).map_err(|_| stage_trap(Stage::TileBin))?,
            y: u32::try_from(id / bins_x).map_err(|_| stage_trap(Stage::TileBin))?,
            primitive_ids: Vec::new(),
        });
    }
    let max_bins =
        usize::try_from(grid.max_bins_per_prim).map_err(|_| stage_trap(Stage::TileBin))?;
    for (prim, &count) in counts.iter().enumerate() {
        if u64::from(count) > u64::from(grid.max_bins_per_prim) {
            return Err(stage_trap(Stage::TileBin));
        }
        if count == 0 || max_bins == 0 {
            continue;
        }
        let prim_id = u32::try_from(prim).map_err(|_| stage_trap(Stage::TileBin))?;
        let base = prim
            .checked_mul(max_bins)
            .ok_or_else(|| stage_trap(Stage::TileBin))?;
        for slot in 0..usize::try_from(count).map_err(|_| stage_trap(Stage::TileBin))? {
            let (bin_id, pair_prim) = *pairs
                .get(base + slot)
                .ok_or_else(|| stage_trap(Stage::TileBin))?;
            if bin_id == BIN_SENTINEL || pair_prim == BIN_SENTINEL || pair_prim != prim_id {
                return Err(stage_trap(Stage::TileBin));
            }
            let bin_index = usize::try_from(bin_id).map_err(|_| stage_trap(Stage::TileBin))?;
            let bin = bins
                .get_mut(bin_index)
                .ok_or_else(|| stage_trap(Stage::TileBin))?;
            bin.primitive_ids.push(pair_prim);
        }
    }
    Ok(TileBinPass { bins })
}

#[cfg(all(test, feature = "vulkan-rdp"))]
#[path = "pipeline_tests.rs"]
mod tests;
