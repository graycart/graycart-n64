//! Edge-walker / rectangle shell. Primitive commands trap until the walker exists.
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   docs/04-rdp.md
//!
//! Opcodes routed here: triangles `0x08..=0x0F`, `0x24` / `0x25` texture
//! rectangles, `0x36` fill rectangle.

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;

use crate::trap::Trap;

/// Triangle and rectangle commands. Always a trap in the M5.0 seam.
pub fn unimplemented_primitive(cmd_id: u8, word: u64) -> Result<(), Trap> {
    Err(Trap::UnimplementedRdpCommand { cmd_id, word })
}
