//! DPC/DPS MMIO and the M5.0 command-list walk.
//!
//! Cited: N64brew — Reality Display Processor/Interface
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Interface
//! Cited: N64brew — Reality Display Processor/Commands (`Sync Full` 0x29)
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   docs/04-rdp.md
//!
//! `Bus` passes `offset = pa & 0xFFFF`. DPC aliases every 0x20 across
//! `0x0410_0000..=0x041F_FFFF`. DPS implements only `0x00/0x04/0x08/0x0C`;
//! any other span offset traps.

#[cfg(test)]
#[path = "regs_tests.rs"]
mod tests;

use crate::bus::Rdram;
use crate::rdp::combine_blend;
use crate::rdp::raster;
use crate::rdp::tmem;
use crate::rdp::{CpuRdpLle, RdpAction, RdpSyncKind};
use crate::trap::Trap;

/// `DPC_START` / `DPC_END` / `DPC_CURRENT`: bits 23:0, 8-byte aligned.
const DPC_PTR_MASK: u32 = 0x00FF_FFF8;
const DPC_STATUS_READ_MASK: u32 = 0x07FF;
const DPC_COUNTER_MASK: u32 = 0x00FF_FFFF;

/// `Sync Full`. Top two bits of the command byte are ignored (`0xE9` → `0x29`).
const CMD_SYNC_FULL: u8 = 0x29;

/// Load TLUT / Set Tile Size / Load Block / Load Tile / Set Tile.
const CMD_LOAD_TLUT: u8 = 0x30;
const CMD_SET_TILE_SIZE: u8 = 0x32;
const CMD_LOAD_BLOCK: u8 = 0x33;
const CMD_LOAD_TILE: u8 = 0x34;
const CMD_SET_TILE: u8 = 0x35;

/// Triangles `0x08..=0x0F`, texture rectangles, fill rectangle.
const CMD_TEX_RECT: u8 = 0x24;
const CMD_TEX_RECT_FLIP: u8 = 0x25;
const CMD_FILL_RECT: u8 = 0x36;

/// Set Combine Mode — owned by the combiner shell.
const CMD_SET_COMBINE: u8 = 0x3C;

pub fn read_cmd_reg(rdp: &CpuRdpLle, offset: u32) -> Result<u32, Trap> {
    match offset & 0x1F {
        0x00 => Ok(rdp.dpc_start & DPC_PTR_MASK),
        0x04 => Ok(rdp.dpc_end & DPC_PTR_MASK),
        0x08 => Ok(rdp.dpc_current & DPC_PTR_MASK),
        0x0C => Ok(rdp.dpc_status & DPC_STATUS_READ_MASK),
        0x10 => Ok(rdp.dpc_clock & DPC_COUNTER_MASK),
        0x14 => Ok(rdp.dpc_bufbusy & DPC_COUNTER_MASK),
        0x18 => Ok(rdp.dpc_pipebusy & DPC_COUNTER_MASK),
        0x1C => Ok(rdp.dpc_tmem & DPC_COUNTER_MASK),
        _ => Err(Trap::UnimplementedRegister {
            device: "DP_CMD",
            offset,
            is_write: false,
        }),
    }
}

pub fn write_cmd_reg_action(
    rdp: &mut CpuRdpLle,
    offset: u32,
    value: u32,
) -> Result<RdpAction, Trap> {
    match offset & 0x1F {
        0x00 => {
            // Latches the pointer and raises START_VALID. Does not start DMA.
            rdp.dpc_start = value & DPC_PTR_MASK;
            rdp.dpc_status |= CpuRdpLle::STATUS_START_VALID;
            Ok(RdpAction::None)
        }
        0x04 => {
            rdp.dpc_end = value & DPC_PTR_MASK;
            if (rdp.dpc_status & CpuRdpLle::STATUS_START_VALID) != 0 {
                // New buffer. `release_gates` starts it, or leaves END_VALID set
                // when FREEZE/FLUSH is set or the previous DMA is still walking.
                rdp.dpc_status |= CpuRdpLle::STATUS_END_VALID;
            }
            release_gates(rdp);
            Ok(RdpAction::None)
        }
        0x08 | 0x10 | 0x14 | 0x18 | 0x1C => {
            // CURRENT and the four activity counters are read-only.
            // Counters reset only through DPC_STATUS clear bits.
            Ok(RdpAction::None)
        }
        0x0C => {
            apply_status_write(rdp, value);
            Ok(RdpAction::None)
        }
        _ => Err(Trap::UnimplementedRegister {
            device: "DP_CMD",
            offset,
            is_write: true,
        }),
    }
}

fn apply_status_write(rdp: &mut CpuRdpLle, value: u32) {
    // Paired CLR/SET. Both bits in one write leave the flag unchanged
    // (same rule as SP_STATUS in this core).
    apply_pair(rdp, value, 0, 1, CpuRdpLle::STATUS_XBUS_DMEM_DMA);
    apply_pair(rdp, value, 2, 3, CpuRdpLle::STATUS_FREEZE);
    apply_pair(rdp, value, 4, 5, CpuRdpLle::STATUS_FLUSH);
    if (value & (1 << 5)) != 0 && (value & (1 << 4)) == 0 {
        // SET_FLUSH: abandon the walking DMA and any pending buffer.
        terminate_dma(rdp);
    }
    if (value & (1 << 6)) != 0 {
        rdp.dpc_tmem = 0;
        rdp.dpc_status &= !CpuRdpLle::STATUS_TMEM_BUSY;
    }
    if (value & (1 << 7)) != 0 {
        rdp.dpc_pipebusy = 0;
        rdp.dpc_status &= !CpuRdpLle::STATUS_PIPE_BUSY;
    }
    if (value & (1 << 8)) != 0 {
        rdp.dpc_bufbusy = 0;
        rdp.dpc_status &= !CpuRdpLle::STATUS_CMD_BUSY;
    }
    if (value & (1 << 9)) != 0 {
        rdp.dpc_clock = 0;
    }
    release_gates(rdp);
}

fn apply_pair(rdp: &mut CpuRdpLle, value: u32, clear_bit: u32, set_bit: u32, flag: u32) {
    let clear = (value & (1 << clear_bit)) != 0;
    let set = (value & (1 << set_bit)) != 0;
    if clear && !set {
        rdp.dpc_status &= !flag;
    } else if set && !clear {
        rdp.dpc_status |= flag;
    }
}

fn dma_running(rdp: &CpuRdpLle) -> bool {
    rdp.dpc_current != rdp.dpc_active_end
}

fn begin_transfer(rdp: &mut CpuRdpLle) {
    rdp.dpc_current = rdp.dpc_start;
    rdp.dpc_active_end = rdp.dpc_end;
    rdp.dpc_status &= !(CpuRdpLle::STATUS_START_VALID | CpuRdpLle::STATUS_END_VALID);
    // PIPE_BUSY and GCLK rise when a DMA is accepted. They stay up until SyncFull
    // even after CURRENT catches END (n64brew: pipe stays busy across the frame).
    rdp.dpc_status |= CpuRdpLle::STATUS_PIPE_BUSY | CpuRdpLle::STATUS_START_GCLK;
    if rdp.dpc_current != rdp.dpc_active_end {
        rdp.dpc_status |= CpuRdpLle::STATUS_DMA_BUSY;
    } else {
        rdp.dpc_status &= !CpuRdpLle::STATUS_DMA_BUSY;
    }
}

fn terminate_dma(rdp: &mut CpuRdpLle) {
    rdp.dpc_current = rdp.dpc_end;
    rdp.dpc_active_end = rdp.dpc_end;
    rdp.dpc_status &=
        !(CpuRdpLle::STATUS_DMA_BUSY | CpuRdpLle::STATUS_END_VALID | CpuRdpLle::STATUS_START_VALID);
}

/// Start a held transfer, or apply an end pointer that was written while gated.
fn release_gates(rdp: &mut CpuRdpLle) {
    if (rdp.dpc_status & (CpuRdpLle::STATUS_FREEZE | CpuRdpLle::STATUS_FLUSH)) != 0 {
        return;
    }
    let pending_new = (rdp.dpc_status & CpuRdpLle::STATUS_START_VALID) != 0
        && (rdp.dpc_status & CpuRdpLle::STATUS_END_VALID) != 0;
    if pending_new && !dma_running(rdp) {
        begin_transfer(rdp);
        return;
    }
    // Incremental END (START_VALID already clear): extend the walking buffer.
    if (rdp.dpc_status & CpuRdpLle::STATUS_START_VALID) == 0 && rdp.dpc_active_end != rdp.dpc_end {
        rdp.dpc_active_end = rdp.dpc_end;
        if dma_running(rdp) {
            rdp.dpc_status |= CpuRdpLle::STATUS_PIPE_BUSY
                | CpuRdpLle::STATUS_START_GCLK
                | CpuRdpLle::STATUS_DMA_BUSY;
        } else {
            rdp.dpc_status &= !CpuRdpLle::STATUS_DMA_BUSY;
        }
    }
}

/// Mark the walking DMA finished and promote a double-buffered START/END pair.
pub fn complete_cmd_dma(rdp: &mut CpuRdpLle) {
    rdp.dpc_current = rdp.dpc_active_end;
    rdp.dpc_status &= !CpuRdpLle::STATUS_DMA_BUSY;
    release_gates(rdp);
}

pub fn read_span_reg(rdp: &CpuRdpLle, offset: u32) -> Result<u32, Trap> {
    match offset {
        0x00 => Ok(rdp.dps_tbist & 0x7FF),
        0x04 => Ok(read_test_mode(rdp)),
        0x08 => Ok(rdp.dps_buftest_addr & 0x7F),
        0x0C => Ok(read_buftest_data(rdp)),
        _ => Err(Trap::UnimplementedRegister {
            device: "DP_SPAN",
            offset,
            is_write: false,
        }),
    }
}

pub fn write_span_reg_action(
    rdp: &mut CpuRdpLle,
    offset: u32,
    value: u32,
) -> Result<RdpAction, Trap> {
    match offset {
        0x00 => {
            // CHECK (0) and GO (1) latch. DONE/FAIL are status and stay 0
            // until a real BIST exists — do not report a pass.
            rdp.dps_tbist = value & 0x3;
            Ok(RdpAction::None)
        }
        0x04 => {
            // Only TEST_ENABLE (bit 0) is writable.
            if (value & 1) != 0 {
                rdp.dps_test_mode |= 1;
            } else {
                rdp.dps_test_mode &= !1;
            }
            Ok(RdpAction::None)
        }
        0x08 => {
            rdp.dps_buftest_addr = value & 0x7F;
            Ok(RdpAction::None)
        }
        0x0C => {
            write_buftest_data(rdp, value);
            Ok(RdpAction::None)
        }
        _ => Err(Trap::UnimplementedRegister {
            device: "DP_SPAN",
            offset,
            is_write: true,
        }),
    }
}

/// Bits 31:28 read as 0. Bit 7 and bit 2 read as 1. Bit 0 is TEST_ENABLE.
/// Span counters are 0 until a primitive walker exists.
fn read_test_mode(rdp: &CpuRdpLle) -> u32 {
    const FIXED_ONES: u32 = (1 << 7) | (1 << 2);
    (rdp.dps_test_mode & 1) | FIXED_ONES
}

fn span_word_mask(index: u32) -> u32 {
    match index & 3 {
        0 | 1 => 0xFFFF_FFFF,
        2 => 0x0000_00FF,
        _ => 0,
    }
}

fn read_buftest_data(rdp: &CpuRdpLle) -> u32 {
    if (rdp.dps_test_mode & 1) == 0 {
        return 0;
    }
    let index = rdp.dps_buftest_addr & 0x7F;
    rdp.dps_span[index as usize] & span_word_mask(index)
}

fn write_buftest_data(rdp: &mut CpuRdpLle, value: u32) {
    if (rdp.dps_test_mode & 1) == 0 {
        return;
    }
    let index = rdp.dps_buftest_addr & 0x7F;
    let mask = span_word_mask(index);
    rdp.dps_span[index as usize] = value & mask;
}

pub fn submit_commands(
    rdp: &mut CpuRdpLle,
    cmds: &[u64],
    rdram: &mut Rdram,
) -> Result<RdpAction, Trap> {
    if cmds.is_empty() {
        return Ok(RdpAction::None);
    }
    let mut action = RdpAction::None;
    for &word in cmds {
        let cmd_id = ((word >> 56) & 0x3F) as u8;
        match cmd_id {
            CMD_SYNC_FULL => {
                action = combine_blend::sync_full(rdp, rdram)?;
            }
            CMD_LOAD_TLUT | CMD_SET_TILE_SIZE | CMD_LOAD_BLOCK | CMD_LOAD_TILE | CMD_SET_TILE => {
                tmem::unimplemented_tmem_op(cmd_id, word)?;
            }
            0x08..=0x0F | CMD_TEX_RECT | CMD_TEX_RECT_FLIP | CMD_FILL_RECT => {
                raster::unimplemented_primitive(cmd_id, word)?;
            }
            CMD_SET_COMBINE => {
                combine_blend::unimplemented_combine(cmd_id, word)?;
            }
            _ => {
                return Err(Trap::UnimplementedRdpCommand { cmd_id, word });
            }
        }
    }
    // The list was fetched, so the walking DMA has the bytes.
    complete_cmd_dma(rdp);
    Ok(action)
}

/// Pipe / Tile / Load complete immediately on the synchronous reference.
/// They must not raise `MI_INTR_DP` and must not clear `PIPE_BUSY`
/// (`SyncFull` is the only barrier that does either).
pub fn sync_barrier(
    _rdp: &mut CpuRdpLle,
    kind: RdpSyncKind,
    _rdram: &mut Rdram,
) -> Result<(), Trap> {
    match kind {
        RdpSyncKind::Pipe | RdpSyncKind::Tile | RdpSyncKind::Load => Ok(()),
        RdpSyncKind::Full => {
            // `CpuRdpLle::sync` dispatches Full to `combine_blend::sync_full`.
            // Reaching here would collapse Full into a no-op barrier.
            Err(Trap::UnimplementedRdpCommand {
                cmd_id: CMD_SYNC_FULL,
                word: u64::from(CMD_SYNC_FULL) << 56,
            })
        }
    }
}
