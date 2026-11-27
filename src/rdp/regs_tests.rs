//! DPC/DPS register and command-walk tests (M5.0).

use super::*;
use crate::bus::Rdram;
use crate::rdp::CpuRdpLle;
use crate::trap::Trap;

fn write_start(rdp: &mut CpuRdpLle, addr: u32) {
    write_cmd_reg_action(rdp, 0x00, addr).unwrap();
}

fn write_end(rdp: &mut CpuRdpLle, addr: u32) {
    write_cmd_reg_action(rdp, 0x04, addr).unwrap();
}

#[test]
fn dpc_start_masks_low_three_bits_and_sets_start_valid() {
    let mut rdp = CpuRdpLle::new();
    write_cmd_reg_action(&mut rdp, 0x00, 0x0010_0007).unwrap();
    assert_eq!(rdp.dpc_start, 0x0010_0000);
    assert_eq!(read_cmd_reg(&rdp, 0x00).unwrap(), 0x0010_0000);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
    assert_eq!(rdp.dpc_current, 0);
}

#[test]
fn dpc_end_new_transfer_sets_current_and_clears_start_valid() {
    let mut rdp = CpuRdpLle::new();
    write_start(&mut rdp, 0x0010_0007);
    write_end(&mut rdp, 0x0010_0040);

    assert_eq!(rdp.dpc_end, 0x0010_0040);
    assert_eq!(rdp.dpc_current, 0x0010_0000);
    assert_eq!(rdp.dpc_active_end, 0x0010_0040);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_END_VALID, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_START_GCLK, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_DMA_BUSY, 0);
}

#[test]
fn dpc_end_incremental_keeps_current() {
    let mut rdp = CpuRdpLle::new();
    write_start(&mut rdp, 0x0020_0000);
    write_end(&mut rdp, 0x0020_0008);
    write_end(&mut rdp, 0x0020_0010);

    assert_eq!(rdp.dpc_current, 0x0020_0000);
    assert_eq!(rdp.dpc_end, 0x0020_0010);
    assert_eq!(rdp.dpc_active_end, 0x0020_0010);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
}

#[test]
fn dpc_status_xbus_pair_and_both_bits_hold() {
    let mut rdp = CpuRdpLle::new();
    write_cmd_reg_action(&mut rdp, 0x0C, 1 << 1).unwrap();
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_XBUS_DMEM_DMA, 0);
    assert_ne!(
        read_cmd_reg(&rdp, 0x0C).unwrap() & CpuRdpLle::STATUS_XBUS_DMEM_DMA,
        0
    );

    write_cmd_reg_action(&mut rdp, 0x0C, 1 << 0).unwrap();
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_XBUS_DMEM_DMA, 0);

    write_cmd_reg_action(&mut rdp, 0x0C, 1 << 1).unwrap();
    write_cmd_reg_action(&mut rdp, 0x0C, (1 << 0) | (1 << 1)).unwrap();
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_XBUS_DMEM_DMA, 0);
}

#[test]
fn dpc_freeze_holds_new_transfer_until_cleared() {
    let mut rdp = CpuRdpLle::new();
    write_cmd_reg_action(&mut rdp, 0x0C, 1 << 3).unwrap();
    write_start(&mut rdp, 0x0030_0000);
    write_end(&mut rdp, 0x0030_0020);

    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_END_VALID, 0);
    assert_eq!(rdp.dpc_current, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);

    write_cmd_reg_action(&mut rdp, 0x0C, 1 << 2).unwrap();
    assert_eq!(rdp.dpc_current, 0x0030_0000);
    assert_eq!(rdp.dpc_active_end, 0x0030_0020);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
}

#[test]
fn dpc_flush_abandons_dma_and_pending_start() {
    let mut rdp = CpuRdpLle::new();
    write_start(&mut rdp, 0x0040_0000);
    write_end(&mut rdp, 0x0040_0010);
    assert_eq!(rdp.dpc_current, 0x0040_0000);

    write_cmd_reg_action(&mut rdp, 0x0C, 1 << 5).unwrap();
    assert_eq!(rdp.dpc_current, 0x0040_0010);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_DMA_BUSY, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_FLUSH, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
}

#[test]
fn dpc_double_buffer_stays_pending_until_dma_completes() {
    let mut rdp = CpuRdpLle::new();
    write_start(&mut rdp, 0x0050_0000);
    write_end(&mut rdp, 0x0050_0010);
    write_start(&mut rdp, 0x0060_0000);
    write_end(&mut rdp, 0x0060_0008);

    assert_eq!(rdp.dpc_current, 0x0050_0000);
    assert_eq!(rdp.dpc_active_end, 0x0050_0010);
    assert_eq!(rdp.dpc_start, 0x0060_0000);
    assert_eq!(rdp.dpc_end, 0x0060_0008);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_END_VALID, 0);

    complete_cmd_dma(&mut rdp);
    assert_eq!(rdp.dpc_current, 0x0060_0000);
    assert_eq!(rdp.dpc_active_end, 0x0060_0008);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_VALID, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_END_VALID, 0);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_DMA_BUSY, 0);
}

#[test]
fn dpc_mirror_every_32_bytes_and_unknown_slot_traps() {
    let mut rdp = CpuRdpLle::new();
    write_cmd_reg_action(&mut rdp, 0x20, 0x0000_0AB7).unwrap();
    assert_eq!(rdp.dpc_start, 0x0000_0AB0);
    assert_eq!(read_cmd_reg(&rdp, 0x00).unwrap(), 0x0000_0AB0);
    assert_eq!(read_cmd_reg(&rdp, 0x40).unwrap(), 0x0000_0AB0);

    let before = rdp.dpc_current;
    write_cmd_reg_action(&mut rdp, 0x08, 0xFFFF_FFFF).unwrap();
    assert_eq!(rdp.dpc_current, before);

    let err = read_cmd_reg(&rdp, 0x02).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRegister {
            device: "DP_CMD",
            offset: 0x02,
            is_write: false,
        }
    ));
    let err = write_cmd_reg_action(&mut rdp, 0x22, 0).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRegister {
            device: "DP_CMD",
            offset: 0x22,
            is_write: true,
        }
    ));
}

#[test]
fn dpc_status_read_masks_to_bits_0_through_10_and_clr_clock() {
    let mut rdp = CpuRdpLle::new();
    rdp.dpc_status |= 1 << 11;
    rdp.dpc_clock = 0x01AB_CDEF;
    rdp.dpc_bufbusy = 0x1111_1111;
    rdp.dpc_pipebusy = 0x2222_2222;
    rdp.dpc_tmem = 0x3333_3333;
    rdp.dpc_status |=
        CpuRdpLle::STATUS_TMEM_BUSY | CpuRdpLle::STATUS_PIPE_BUSY | CpuRdpLle::STATUS_CMD_BUSY;

    assert_eq!(read_cmd_reg(&rdp, 0x0C).unwrap() & (1 << 11), 0);
    assert_eq!(read_cmd_reg(&rdp, 0x10).unwrap(), 0x00AB_CDEF);

    write_cmd_reg_action(&mut rdp, 0x0C, (1 << 9) | (1 << 8) | (1 << 7) | (1 << 6)).unwrap();
    assert_eq!(rdp.dpc_clock, 0);
    assert_eq!(rdp.dpc_bufbusy, 0);
    assert_eq!(rdp.dpc_pipebusy, 0);
    assert_eq!(rdp.dpc_tmem, 0);
    assert_eq!(
        rdp.dpc_status
            & (CpuRdpLle::STATUS_TMEM_BUSY
                | CpuRdpLle::STATUS_PIPE_BUSY
                | CpuRdpLle::STATUS_CMD_BUSY),
        0
    );
    assert_eq!(read_cmd_reg(&rdp, 0x14).unwrap(), 0);
}

#[test]
fn dps_test_mode_fixed_bits_and_span_window() {
    let mut rdp = CpuRdpLle::new();
    write_span_reg_action(&mut rdp, 0x04, 0xA5A5_A5A5).unwrap();
    // Bit 0 follows the write. Bit 2 and bit 7 are fixed 1. Bits 31:28 stay 0.
    assert_eq!(read_span_reg(&rdp, 0x04).unwrap(), (1 << 7) | (1 << 2) | 1);

    write_span_reg_action(&mut rdp, 0x00, 0xFFFF_FFFF).unwrap();
    assert_eq!(read_span_reg(&rdp, 0x00).unwrap(), 0x3);

    write_span_reg_action(&mut rdp, 0x08, 0x180).unwrap();
    assert_eq!(read_span_reg(&rdp, 0x08).unwrap(), 0x00);
    write_span_reg_action(&mut rdp, 0x0C, 0xAABB_CCDD).unwrap();
    assert_eq!(read_span_reg(&rdp, 0x0C).unwrap(), 0xAABB_CCDD);

    write_span_reg_action(&mut rdp, 0x08, 2).unwrap();
    write_span_reg_action(&mut rdp, 0x0C, 0xFFFF_FFFF).unwrap();
    assert_eq!(read_span_reg(&rdp, 0x0C).unwrap(), 0xFF);

    write_span_reg_action(&mut rdp, 0x08, 3).unwrap();
    write_span_reg_action(&mut rdp, 0x0C, 0xFFFF_FFFF).unwrap();
    assert_eq!(read_span_reg(&rdp, 0x0C).unwrap(), 0);

    write_span_reg_action(&mut rdp, 0x04, 0).unwrap();
    write_span_reg_action(&mut rdp, 0x08, 0).unwrap();
    write_span_reg_action(&mut rdp, 0x0C, 0x1111_1111).unwrap();
    assert_eq!(read_span_reg(&rdp, 0x0C).unwrap(), 0);
    assert_eq!(rdp.dps_span[0], 0xAABB_CCDD);
}

#[test]
fn dps_unknown_offset_traps() {
    let mut rdp = CpuRdpLle::new();
    let err = read_span_reg(&rdp, 0x10).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRegister {
            device: "DP_SPAN",
            offset: 0x10,
            is_write: false,
        }
    ));
    let err = write_span_reg_action(&mut rdp, 0x02, 1).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRegister {
            device: "DP_SPAN",
            offset: 0x02,
            is_write: true,
        }
    ));
}

#[test]
fn submit_empty_ok_syncfull_raises_and_other_cmds_trap() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    assert_eq!(
        submit_commands(&mut rdp, &[], &mut rdram).unwrap(),
        RdpAction::None
    );

    write_start(&mut rdp, 0x0070_0000);
    write_end(&mut rdp, 0x0070_0010);
    rdp.dpc_status |= CpuRdpLle::STATUS_CMD_BUSY;

    let sync_full = 0x29u64 << 56;
    let action = submit_commands(&mut rdp, &[sync_full], &mut rdram).unwrap();
    assert_eq!(action, RdpAction::RaiseDpIntr);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_CMD_BUSY, 0);
    assert_eq!(rdp.dpc_current, 0x0070_0010);

    let fill = 0x36u64 << 56;
    let err = submit_commands(&mut rdp, &[fill], &mut rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand {
            cmd_id: 0x36,
            word,
        } if word == fill
    ));

    let set_combine = 0x3Cu64 << 56;
    let err = submit_commands(&mut rdp, &[set_combine], &mut rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x3C, .. }
    ));

    let load_tile = 0x34u64 << 56;
    let err = submit_commands(&mut rdp, &[load_tile], &mut rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x34, .. }
    ));
}

#[test]
fn sync_pipe_does_not_clear_pipe_busy_or_raise() {
    let mut rdp = CpuRdpLle::new();
    rdp.dpc_status |= CpuRdpLle::STATUS_PIPE_BUSY;
    let mut rdram = Rdram::new(false);
    sync_barrier(&mut rdp, RdpSyncKind::Pipe, &mut rdram).unwrap();
    sync_barrier(&mut rdp, RdpSyncKind::Tile, &mut rdram).unwrap();
    sync_barrier(&mut rdp, RdpSyncKind::Load, &mut rdram).unwrap();
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
    assert!(sync_barrier(&mut rdp, RdpSyncKind::Full, &mut rdram).is_err());
}

#[test]
fn reset_restores_power_on_status() {
    let mut rdp = CpuRdpLle::new();
    write_start(&mut rdp, 0x1000);
    write_end(&mut rdp, 0x1010);
    rdp.reset();
    assert_eq!(rdp.dpc_start, 0);
    assert_eq!(rdp.dpc_current, 0);
    assert_eq!(rdp.dpc_status, CpuRdpLle::STATUS_CBUF_READY);
    assert!(rdp.tmem.iter().all(|&b| b == 0));
    assert!(rdp.dps_span.iter().all(|&w| w == 0));
}
