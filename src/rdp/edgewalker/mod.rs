//! EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
//!
//! [`EdgeWalker`] implements [`super::RdpBackend`]. It is the M6 GPU LLE:
//! pure Rust, no paraLLEl-RDP / Angrylion / GPL forks, and no C++ FFI.
//!
//! **M6 gate:** bit-exact frame hash EdgeWalker == [`super::CpuRdpLle`].
//!
//! W0 is the seam. [`EdgeWalker::submit`] and [`EdgeWalker::sync`] execute
//! the command pipe on the CPU so default CI stays bit-exact with
//! [`super::CpuRdpLle`] before compute shaders exist (W1+). GPU-only entry
//! [`EdgeWalker::dispatch_compute`] traps. The `vulkan-rdp` Cargo feature
//! compiles the compute-device seam when enabled. wgpu (pure Rust, no C++ FFI) is
//! the compute API that feature will link; ash is not used. The wgpu crate
//! stays out of the build until the first shader lands, so default features
//! and `--all-features` remain free of GPU crates.
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Interface
//! Cross-check: docs/ORCHESTRATOR-HANDOFF.md (Milestone M6)

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

use super::{CpuRdpLle, RdpBackend, RdpSyncKind};

#[cfg(feature = "vulkan-rdp")]
mod gpu;

#[cfg(feature = "vulkan-rdp")]
pub use gpu::ComputeDevice;

/// Compute stages of EdgeWalker: ingestion, tile binning, span, and VI.
///
/// EdgeWalker is Graycart's bespoke Vulkan compute LLE RDP. These stages are
/// that RDP. GPU dispatch stays trapped until the 1× frame hash matches
/// [`CpuRdpLle`].
pub mod pipeline;

pub mod parity;

#[cfg(test)]
mod parity_tests;

#[cfg(test)]
mod tests;

/// Graycart's bespoke Vulkan compute LLE RDP.
///
/// **M6 gate:** bit-exact frame hash EdgeWalker == [`CpuRdpLle`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWalker {
    /// W0 command and raster state. Compute shaders (W1+) take over this pipe
    /// and must keep the frame hash identical to [`CpuRdpLle`].
    parity: CpuRdpLle,
}

impl EdgeWalker {
    /// Offset named by [`Self::dispatch_compute`] until compute shaders land.
    ///
    /// This is not a DPC/DPS register. The trap variant is
    /// [`Trap::UnimplementedRegister`] because that variant carries the device name.
    pub const COMPUTE_DISPATCH: u32 = 0x0001;

    #[must_use]
    pub fn new() -> Self {
        Self {
            parity: CpuRdpLle::new(),
        }
    }

    /// GPU compute dispatch. Traps until W1 shaders exist.
    ///
    /// [`Self::submit`] / [`Self::sync`] remain the bit-exact path.
    pub fn dispatch_compute(&self) -> Result<(), Trap> {
        #[cfg(feature = "vulkan-rdp")]
        {
            ComputeDevice::open().map(|_| ())
        }
        #[cfg(not(feature = "vulkan-rdp"))]
        {
            Err(compute_unavailable())
        }
    }
}

impl Default for EdgeWalker {
    fn default() -> Self {
        Self::new()
    }
}

impl RdpBackend for EdgeWalker {
    /// Run `cmds` against `rdram`.
    ///
    /// **M6 gate:** bit-exact frame hash EdgeWalker == [`CpuRdpLle`].
    /// W0 meets that gate by executing the verified software pipe. W1+ replaces
    /// this body with compute shaders that must keep the same hash.
    fn submit(&mut self, cmds: &[u64], rdram: &mut Rdram) -> Result<(), Trap> {
        self.parity.submit(cmds, rdram)
    }

    /// Honor one hardware sync barrier without collapsing it into another.
    ///
    /// **M6 gate:** bit-exact frame hash EdgeWalker == [`CpuRdpLle`].
    fn sync(&mut self, kind: RdpSyncKind, rdram: &mut Rdram) -> Result<(), Trap> {
        self.parity.sync(kind, rdram)
    }
}

fn compute_unavailable() -> Trap {
    Trap::UnimplementedRegister {
        device: "EdgeWalker",
        offset: EdgeWalker::COMPUTE_DISPATCH,
        is_write: true,
    }
}
