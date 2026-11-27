//! EdgeWalker compute-device seam (`vulkan-rdp` feature).
//!
//! Compiled only with `--features vulkan-rdp`. Opening the device traps until
//! W1 compute shaders exist. wgpu is the planned pure-Rust compute API
//! (no C++ FFI); the crate is linked when the first shader lands, not in W0.
//!
//! **M6 gate:** bit-exact frame hash EdgeWalker == CpuRdpLle.
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor

use crate::trap::Trap;

use super::compute_unavailable;

/// EdgeWalker's Vulkan compute device. Unopened in W0.
#[derive(Debug)]
pub struct ComputeDevice;

impl ComputeDevice {
    /// Acquire the compute device. Traps until W1 shaders land.
    pub fn open() -> Result<Self, Trap> {
        Err(compute_unavailable())
    }
}
