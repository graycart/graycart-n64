//! Reality Display Processor (RDP) backend trait and bespoke LLE implementations.
//!
//! Two 100%-bespoke Graycart LLE RDP implementations share [`RdpBackend`]:
//! - [`CpuRdpLle`] (M5): pure-Rust software LLE reference (default in CI).
//! - [`EdgeWalker`] (M6): Graycart's standalone bespoke Vulkan compute LLE RDP
//!   (written directly from N64 RDP hardware specs, zero CPU fallback; traps
//!   when `vulkan-rdp` is disabled).
//!   **M6 gate:** bit-exact frame hash EdgeWalker == CpuRdpLle.
//!   M11 enhancements (upscaling, SSAA, widescreen, VI/CRT filters, Oracle
//!   Lens, Live Frame Inspector) stay behind `EnhancementConfig`.
//!
//! Cited: N64brew — Reality Display Processor / Interface
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Interface
//!   docs/04-rdp.md

pub mod combine_blend;
pub mod edgewalker;
pub mod raster;
pub mod regs;
pub mod tmem;

#[cfg(feature = "vulkan-rdp")]
pub use edgewalker::ComputeDevice;
pub use edgewalker::EdgeWalker;

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

/// Explicit RDP pipeline synchronization barriers.
///
/// Distinct barriers MUST NOT be collapsed into `Full` in M5/M6 (see deep-dive §02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RdpSyncKind {
    Pipe,
    Tile,
    Load,
    Full,
}

/// Active RDP execution backend behind [`crate::bus::Bus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RdpBackendKind {
    /// Pure-Rust software LLE reference ([`CpuRdpLle`]).
    #[default]
    CpuLle,
    /// Bespoke Vulkan compute LLE ([`EdgeWalker`]).
    EdgeWalker,
}

/// Side effects [`crate::bus::Bus`] applies after a DPC/DPS write or `SyncFull`.
///
/// Register writes return [`RdpAction::None`]. `SyncFull` returns
/// [`RdpAction::RaiseDpIntr`] from [`CpuRdpLle::submit_commands`] and from
/// [`combine_blend::sync_full`]. [`RdpBackend::submit`] / [`RdpBackend::sync`]
/// perform the same work and drop the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RdpAction {
    None,
    /// Assert `MI_INTR_DP` (raised by `SyncFull`).
    RaiseDpIntr,
}

/// Latched color or texture image (`Set Color Image` / `Set Texture Image`).
///
/// Field widths follow n64brew: format [55:53], size [52:51], width [41:32]
/// (pixels minus one), DRAM address [23:0].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RdpImageState {
    pub format: u8,
    pub size: u8,
    pub width: u16,
    pub addr: u32,
}

/// Latched `Set Scissor` rectangle (u10.2 coordinates) plus interlace bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RdpScissor {
    pub ulx: u16,
    pub uly: u16,
    pub lrx: u16,
    pub lry: u16,
    pub field: bool,
    pub odd: bool,
}

/// Last edge-walker span. Updated as a triangle or rectangle is stepped.
///
/// `x_major` / `x_minor` are s11.16, matching the triangle edge fields.
/// `valid` is false until the first span of a primitive is produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RdpSpanTemps {
    pub y: i32,
    pub x_major: i64,
    pub x_minor: i64,
    pub valid: bool,
}

/// Low-level RDP command execution contract shared by [`CpuRdpLle`] and `EdgeWalker`.
pub trait RdpBackend {
    /// Submit one or more 64-bit RDP command words targeting `rdram` (+ 9th-bit side array).
    fn submit(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<(), Trap>;

    /// Flush pipeline work up to the specified hardware barrier.
    fn sync(&mut self, kind: RdpSyncKind, rdram: &mut Rdram) -> Result<(), Trap>;
}

/// Bespoke pure-Rust software LLE RDP reference oracle (implemented in M5).
///
/// Single source of truth for DPC/DPS MMIO lives on [`crate::bus::Bus::rdp`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuRdpLle {
    /// 4 KiB on-chip texture memory (TMEM).
    pub tmem: Vec<u8>,
    /// Deterministic synchronous execution flag (always true in hash/test mode).
    pub deterministic_sync: bool,

    // --- DPC command registers (`0x0410_0000`, mirrored every 0x20) ---
    /// `DPC_START` bits 23:0 (8-byte aligned; low 3 bits ignored).
    pub dpc_start: u32,
    /// `DPC_END` bits 23:0 (exclusive bound).
    pub dpc_end: u32,
    /// `DPC_CURRENT` bits 23:0 (DMA read pointer of the transfer in progress).
    pub dpc_current: u32,
    /// Exclusive end of the transfer `DPC_CURRENT` is walking.
    ///
    /// Equals [`Self::dpc_end`] except while `END_VALID` is set: then `dpc_end`
    /// is the pending buffer and this field is the one still being fetched.
    pub dpc_active_end: u32,
    /// Latched `DPC_STATUS` flag bits (XBUS/FREEZE/FLUSH/GCLK/busy/pending).
    pub dpc_status: u32,
    /// `DPC_CLOCK` 24-bit RCP-frequency counter.
    pub dpc_clock: u32,
    /// `DPC_CMD_BUSY` / `DPC_BUF_BUSY` 24-bit activity counter.
    pub dpc_bufbusy: u32,
    /// `DPC_PIPE_BUSY` 24-bit counter.
    pub dpc_pipebusy: u32,
    /// `DPC_TMEM_BUSY` 24-bit counter.
    pub dpc_tmem: u32,

    // --- DPS span / test registers (`0x0420_0000`) ---
    pub dps_tbist: u32,
    /// `DPS_TEST_MODE` bit 0 (`TEST_ENABLE`). Read forces the fixed-1 bits.
    pub dps_test_mode: u32,
    /// `DPS_BUFTEST_ADDR` bits 6:0 (word index into [`Self::dps_span`]).
    pub dps_buftest_addr: u32,
    /// Span RAM window. 128 words; a 7-bit index covers 512 bytes, of which the
    /// real buffer is 288 bytes. Word index `n & 3 == 2` keeps only the low byte,
    /// and `n & 3 == 3` is hardwired 0 (n64brew `DPS_BUFTEST_DATA` map).
    pub dps_span: Vec<u32>,

    /// `Set Color Image` (0x3F).
    pub color_image: RdpImageState,
    /// `Set Depth Image` (0x3E) DRAM address, bits 23:0.
    pub depth_image_addr: u32,
    /// `Set Texture Image` (0x3D).
    pub texture_image: RdpImageState,
    /// `Set Scissor` (0x2D).
    pub scissor: RdpScissor,
    /// `Set Other Modes` (0x2F), bits 55:0 (command id stripped).
    ///
    /// Cycle type and blender muxes are evaluated by [`combine_blend::eval_blend`].
    pub other_modes: u64,
    /// `Set Fill Color` (0x37), bits 31:0.
    pub fill_color: u32,
    /// `Set Fog Color` (0x38), RGBA8888 in bits 31:0.
    pub fog_color: u32,
    /// `Set Blend Color` (0x39), RGBA8888 in bits 31:0.
    pub blend_color: u32,
    /// `Set Primitive Color` (0x3A) RGBA8888.
    pub prim_color: u32,
    /// `Set Primitive Color` minimum LOD, bits 47:40.
    pub prim_min_level: u8,
    /// `Set Primitive Color` LOD fraction, bits 39:32.
    pub prim_lod_frac: u8,
    /// `Set Environment Color` (0x3B), RGBA8888.
    pub env_color: u32,
    /// `Set Combine Mode` (0x3C) mux selects, bits 55:0.
    ///
    /// Evaluated by [`combine_blend::eval_combine`]: `(A - B) * C + D`.
    pub combine_mode: u64,
    /// `Set Convert` K4, bits 17:9. Combiner RGB B input 7.
    pub convert_k4: u16,
    /// `Set Convert` K5, bits 8:0. Combiner RGB C input 15.
    pub convert_k5: u16,
    /// Chroma-key center, RGB. Combiner RGB B input 6 (`Set Key R` / `Set Key GB`).
    pub key_center: [u8; 3],
    /// Chroma-key scale, RGB. Combiner RGB C input 6 (`Set Key R` / `Set Key GB`).
    pub key_scale: [u8; 3],
    /// Eight tile descriptors (`Set Tile`, `Set Tile Size`, and the TMEM loads).
    pub tiles: [tmem::TileDesc; tmem::TILE_COUNT],
    /// `Set Primitive Depth` (0x2E) integer Z. Raster latches this; the command
    /// walker does not route `0x2E` yet (`raster::set_primitive_depth`).
    pub prim_z: u16,
    /// `Set Primitive Depth` deltaz. Hardware wants a power of two.
    pub prim_dz: u16,
    /// Edge-walker span temps (last scanline of the primitive just drawn).
    pub span: RdpSpanTemps,
}

impl CpuRdpLle {
    /// `DPC_STATUS` read bit: XBUS DMEM source when set.
    pub const STATUS_XBUS_DMEM_DMA: u32 = 1 << 0;
    pub const STATUS_FREEZE: u32 = 1 << 1;
    pub const STATUS_FLUSH: u32 = 1 << 2;
    pub const STATUS_START_GCLK: u32 = 1 << 3;
    pub const STATUS_TMEM_BUSY: u32 = 1 << 4;
    pub const STATUS_PIPE_BUSY: u32 = 1 << 5;
    pub const STATUS_CMD_BUSY: u32 = 1 << 6;
    pub const STATUS_CBUF_READY: u32 = 1 << 7;
    pub const STATUS_DMA_BUSY: u32 = 1 << 8;
    pub const STATUS_END_VALID: u32 = 1 << 9;
    pub const STATUS_START_VALID: u32 = 1 << 10;

    #[must_use]
    pub fn new() -> Self {
        Self {
            tmem: vec![0; tmem::TMEM_SIZE],
            deterministic_sync: true,
            dpc_start: 0,
            dpc_end: 0,
            dpc_current: 0,
            dpc_active_end: 0,
            dpc_status: Self::STATUS_CBUF_READY,
            dpc_clock: 0,
            dpc_bufbusy: 0,
            dpc_pipebusy: 0,
            dpc_tmem: 0,
            dps_tbist: 0,
            dps_test_mode: 0,
            dps_buftest_addr: 0,
            dps_span: vec![0; 128],
            color_image: RdpImageState::default(),
            depth_image_addr: 0,
            texture_image: RdpImageState::default(),
            scissor: RdpScissor::default(),
            other_modes: 0,
            fill_color: 0,
            fog_color: 0,
            blend_color: 0,
            prim_color: 0,
            prim_min_level: 0,
            prim_lod_frac: 0,
            env_color: 0,
            combine_mode: 0,
            convert_k4: 0,
            convert_k5: 0,
            key_center: [0, 0, 0],
            key_scale: [0, 0, 0],
            tiles: [tmem::TileDesc::default(); tmem::TILE_COUNT],
            prim_z: 0,
            prim_dz: 0,
            span: RdpSpanTemps::default(),
        }
    }

    /// Retire the in-flight command DMA (`CURRENT = active end`) and start a
    /// transfer that was left pending (`START_VALID` + `END_VALID`).
    ///
    /// [`RdpBackend::submit`] does this after a command list is accepted.
    /// [`crate::bus::Bus`] copies `[dpc_current, dpc_active_end)` from RDRAM,
    /// or from `sp_dmem` when XBUS is set, on `DPC_END` / incremental END /
    /// freeze or flush release, then calls [`Self::submit_commands`].
    pub fn complete_cmd_dma(&mut self) {
        regs::complete_cmd_dma(self);
    }

    /// Run a command list. `SyncFull` (`0x29`) yields [`RdpAction::RaiseDpIntr`].
    ///
    /// [`RdpBackend::submit`] is the same walk but drops the action. Bus code that
    /// must raise `MI_INTR_DP` should use this return value (or `sync_full`).
    pub fn submit_commands(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<RdpAction, Trap> {
        regs::submit_commands(self, cmds, rdram)
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn read_cmd_reg(&self, offset: u32) -> Result<u32, Trap> {
        regs::read_cmd_reg(self, offset)
    }

    pub fn write_cmd_reg_action(&mut self, offset: u32, value: u32) -> Result<RdpAction, Trap> {
        regs::write_cmd_reg_action(self, offset, value)
    }

    pub fn write_cmd_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_cmd_reg_action(offset, value).map(|_| ())
    }

    pub fn read_span_reg(&self, offset: u32) -> Result<u32, Trap> {
        regs::read_span_reg(self, offset)
    }

    pub fn write_span_reg_action(&mut self, offset: u32, value: u32) -> Result<RdpAction, Trap> {
        regs::write_span_reg_action(self, offset, value)
    }

    pub fn write_span_reg(&mut self, offset: u32, value: u32) -> Result<(), Trap> {
        self.write_span_reg_action(offset, value).map(|_| ())
    }
}

impl Default for CpuRdpLle {
    fn default() -> Self {
        Self::new()
    }
}

impl RdpBackend for CpuRdpLle {
    fn submit(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<(), Trap> {
        regs::submit_commands(self, cmds, rdram).map(|_| ())
    }

    fn sync(&mut self, kind: RdpSyncKind, rdram: &mut Rdram) -> Result<(), Trap> {
        match kind {
            RdpSyncKind::Full => combine_blend::sync_full(self, rdram).map(|_| ()),
            other => regs::sync_barrier(self, other, rdram),
        }
    }
}
