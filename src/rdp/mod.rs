//! Reality Display Processor (RDP) backend trait and bespoke LLE shells.
//!
//! Two 100%-bespoke Graycart LLE RDP implementations share [`RdpBackend`]:
//! - [`CpuRdpLle`] (M5): pure-Rust software LLE reference hash oracle (default in CI).
//! - **`EdgeWalker`** (M6, behind a future opt-in Vulkan compute backend): our bespoke
//!   Vulkan compute LLE RDP written from scratch and cross-checked bit-for-bit against
//!   [`CpuRdpLle`] and external hardware-verified goldens, with opt-in M11 enhancements
//!   (upscaling, SSAA, widescreen, VI/CRT filters, Oracle Lens, Live Frame Inspector).
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor · docs/04-rdp.md

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

/// Low-level RDP command execution contract shared by [`CpuRdpLle`] and `EdgeWalker`.
pub trait RdpBackend {
    /// Submit one or more 64-bit RDP command words targeting `rdram` (+ 9th-bit side array).
    fn submit(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<(), Trap>;

    /// Flush pipeline work up to the specified hardware barrier.
    fn sync(&mut self, kind: RdpSyncKind, rdram: &mut Rdram) -> Result<(), Trap>;
}

/// Bespoke pure-Rust software LLE RDP reference oracle (implemented in M5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuRdpLle {
    /// 4 KiB on-chip texture memory (TMEM).
    pub tmem: Vec<u8>,
    /// Deterministic synchronous execution flag (always true in hash/test mode).
    pub deterministic_sync: bool,
}

impl CpuRdpLle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tmem: vec![0; 4096],
            deterministic_sync: true,
        }
    }

    pub fn read_cmd_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "DP_CMD",
            offset,
            is_write: false,
        })
    }

    pub fn write_cmd_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "DP_CMD",
            offset,
            is_write: true,
        })
    }

    pub fn read_span_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "DP_SPAN",
            offset,
            is_write: false,
        })
    }

    pub fn write_span_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "DP_SPAN",
            offset,
            is_write: true,
        })
    }
}

impl Default for CpuRdpLle {
    fn default() -> Self {
        Self::new()
    }
}

impl RdpBackend for CpuRdpLle {
    fn submit(&mut self, cmds: &[u64], _rdram: &mut Rdram) -> Result<(), Trap> {
        if let Some(&word) = cmds.first() {
            let cmd_id = ((word >> 56) & 0x3F) as u8;
            return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
        }
        Ok(())
    }

    fn sync(&mut self, _kind: RdpSyncKind, _rdram: &mut Rdram) -> Result<(), Trap> {
        Ok(())
    }
}
