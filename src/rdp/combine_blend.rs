//! Color combiner / blender shell. `SyncFull` (0x29) is live; other combiner
//! commands trap until the pipe is implemented.
//!
//! Cited: N64brew — Reality Display Processor/Commands (Sync Full 0x29)
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   docs/04-rdp.md

#[cfg(test)]
#[path = "combine_blend_tests.rs"]
mod tests;

use crate::bus::Rdram;
use crate::rdp::{CpuRdpLle, RdpAction};
use crate::trap::Trap;

/// `SyncFull` (command `0x29`): the pipe goes idle and the DP interrupt is requested.
///
/// Does not retire `DPC_CURRENT`. Command-DMA completion is
/// [`crate::rdp::CpuRdpLle::complete_cmd_dma`], which `submit_commands` calls
/// after a list is accepted.
pub fn sync_full(rdp: &mut CpuRdpLle, _rdram: &mut Rdram) -> Result<RdpAction, Trap> {
    rdp.dpc_status &= !(CpuRdpLle::STATUS_PIPE_BUSY
        | CpuRdpLle::STATUS_CMD_BUSY
        | CpuRdpLle::STATUS_TMEM_BUSY
        | CpuRdpLle::STATUS_START_GCLK
        | CpuRdpLle::STATUS_DMA_BUSY);
    Ok(RdpAction::RaiseDpIntr)
}

/// Set Combine and the rest of the combiner/blender block.
pub fn unimplemented_combine(cmd_id: u8, word: u64) -> Result<(), Trap> {
    Err(Trap::UnimplementedRdpCommand { cmd_id, word })
}
