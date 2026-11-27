//! Reality Display Processor (RDP) backend trait and bespoke LLE shells.
//!
//! Two 100%-bespoke Graycart LLE RDP implementations share [`RdpBackend`]:
//! - [`CpuRdpLle`] (M5): pure-Rust software LLE reference hash oracle (default in CI).
//! - **`EdgeWalker`** (M6, behind a future opt-in Vulkan compute backend): our bespoke
//!   Vulkan compute LLE RDP written from scratch and cross-checked bit-for-bit against
//!   [`CpuRdpLle`] and external hardware-verified goldens, with opt-in M11 enhancements
//!   (upscaling, SSAA, widescreen, VI/CRT filters, Oracle Lens, Live Frame Inspector).
//!
//! Cited: N64brew — Reality Display Processor / Interface
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Interface
//!   docs/04-rdp.md

pub mod combine_blend;
pub mod raster;
pub mod regs;
pub mod tmem;

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
        }
    }

    /// Retire the in-flight command DMA (`CURRENT = active end`) and start a
    /// transfer that was left pending (`START_VALID` + `END_VALID`).
    ///
    /// [`RdpBackend::submit`] does this after a command list is accepted.
    /// Register writes do not fetch RDRAM/DMEM; the bus copies
    /// `[dpc_current, dpc_active_end)` and then calls this or [`Self::submit_commands`].
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
