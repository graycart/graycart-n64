//! 4 KiB TMEM shell. Load/tile commands trap until the sampler is implemented.
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   docs/04-rdp.md
//!
//! Opcodes routed here from the command walk: `0x30` Load TLUT, `0x32` Set Tile
//! Size, `0x33` Load Block, `0x34` Load Tile, `0x35` Set Tile.

#[cfg(test)]
#[path = "tmem_tests.rs"]
mod tests;

use crate::rdp::CpuRdpLle;
use crate::trap::Trap;

/// Clear TMEM to zeros (power-on / soft reset helper).
pub fn clear_tmem(rdp: &mut CpuRdpLle) {
    rdp.tmem.fill(0);
}

/// On-chip texture memory size in bytes.
pub const TMEM_SIZE: usize = 4096;

/// Load/tile commands. Always a trap in the M5.0 seam.
pub fn unimplemented_tmem_op(cmd_id: u8, word: u64) -> Result<(), Trap> {
    Err(Trap::UnimplementedRdpCommand { cmd_id, word })
}
