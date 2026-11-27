//! DPC/DPS register and command-walk tests (M5.0).

use super::*;
use crate::bus::{Bus, CacheAttr, Rdram};
use crate::mi::MI_INTR_DP;
use crate::rdp::{CpuRdpLle, RdpAction};
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

    // Fill Rectangle is dispatched. The power-on scissor excludes every
    // pixel, so the command completes without a color write.
    let fill = 0x36u64 << 56;
    let action = submit_commands(&mut rdp, &[fill], &mut rdram).unwrap();
    assert_eq!(action, RdpAction::None);
    assert_eq!(rdram.read_u8(0), 0);

    // Load Tile is dispatched. A u10.2 fraction bit is still a trap.
    let load_tile = (0x34u64 << 56) | 1;
    let err = submit_commands(&mut rdp, &[load_tile], &mut rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand {
            cmd_id: 0x34,
            word,
        } if word == load_tile
    ));

    // Set Convert (0x2C) is outside the TMEM opcodes and still traps.
    let set_convert = 0x2Cu64 << 56;
    let err = submit_commands(&mut rdp, &[set_convert], &mut rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand {
            cmd_id: 0x2C,
            word,
        } if word == set_convert
    ));
}

#[test]
fn submit_load_tile_copies_texel_when_texture_image_is_set() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    rdram.write_u16(0, 0xF801).unwrap();

    // Set Texture Image: RGBA, 16-bit, one texel wide, DRAM address 0.
    let set_tex = (0x3Du64 << 56) | (2 << 51);
    // Set Tile: RGBA, 16-bit, line = 1, TMEM word 0, tile 0.
    let set_tile = (0x35u64 << 56) | (2 << 51) | (1 << 41);
    // Load Tile: inclusive u10.2 span of one texel at (0, 0), tile 0.
    let load_tile = 0x34u64 << 56;

    let action = submit_commands(&mut rdp, &[set_tex, set_tile, load_tile], &mut rdram).unwrap();
    assert_eq!(action, RdpAction::None);
    assert_eq!(rdp.texture_image.size, 2);
    assert_eq!(rdp.texture_image.addr, 0);
    assert_eq!(rdp.tiles[0].format, 0);
    assert_eq!(rdp.tiles[0].size, 2);
    assert_eq!(rdp.tiles[0].line, 1);
    assert_eq!(rdp.tiles[0].tmem_addr, 0);
    assert_eq!(rdp.tiles[0].sh, 0);
    assert_eq!(rdp.tiles[0].th, 0);
    assert_eq!(rdp.tmem[0], 0xF8);
    assert_eq!(rdp.tmem[1], 0x01);
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
fn command_word_count_matches_n64brew_lengths() {
    assert_eq!(command_word_count(0x08u64 << 56), 4);
    assert_eq!(command_word_count(0x09u64 << 56), 6);
    assert_eq!(command_word_count(0x0Au64 << 56), 12);
    assert_eq!(command_word_count(0x0Cu64 << 56), 12);
    assert_eq!(command_word_count(0x0Eu64 << 56), 20);
    assert_eq!(command_word_count(0x0Fu64 << 56), 22);
    assert_eq!(command_word_count(0x24u64 << 56), 2);
    assert_eq!(command_word_count(0x25u64 << 56), 2);
    assert_eq!(command_word_count(0x29u64 << 56), 1);
    assert_eq!(command_word_count(0x3Fu64 << 56), 1);
    assert_eq!(command_word_count(0), 1);
}

#[test]
fn sync_pipe_tile_load_do_not_raise_or_clear_pipe_busy() {
    let mut rdp = CpuRdpLle::new();
    rdp.dpc_status |= CpuRdpLle::STATUS_PIPE_BUSY;
    let mut rdram = Rdram::new(false);
    let action = submit_commands(
        &mut rdp,
        &[0x27u64 << 56, 0x26u64 << 56, 0x28u64 << 56],
        &mut rdram,
    )
    .unwrap();
    assert_eq!(action, RdpAction::None);
    assert_ne!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);

    let action = submit_commands(&mut rdp, &[0x29u64 << 56], &mut rdram).unwrap();
    assert_eq!(action, RdpAction::RaiseDpIntr);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
}

#[test]
fn set_color_image_stores_format_size_width_addr() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    // CI format (2), 16-bit (2), width field 319, DRAM 0x0010_0000.
    let word = (0x3Fu64 << 56) | (2 << 53) | (2 << 51) | (319 << 32) | 0x0010_0000;
    submit_commands(&mut rdp, &[word], &mut rdram).unwrap();
    assert_eq!(rdp.color_image.format, 2);
    assert_eq!(rdp.color_image.size, 2);
    assert_eq!(rdp.color_image.width, 319);
    assert_eq!(rdp.color_image.addr, 0x0010_0000);
}

#[test]
fn state_setters_latch_scissor_modes_colors_and_combine() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    let scissor = (0x2Du64 << 56) | (0x10 << 44) | (0x20 << 32) | (1 << 25) | (0x100 << 12) | 0x80;
    let other = (0x2Fu64 << 56) | (1 << 55) | (3 << 52);
    let fill = (0x37u64 << 56) | 0xAABB_CCDD;
    let fog = (0x38u64 << 56) | 0x0102_0304;
    let blend = (0x39u64 << 56) | 0x1122_3344;
    let prim = (0x3Au64 << 56) | (0x10 << 40) | (0x20 << 32) | 0x5566_7788;
    let env = (0x3Bu64 << 56) | 0x99AA_BBCC;
    let combine = (0x3Cu64 << 56) | (3 << 52);
    let texture = (0x3Du64 << 56) | (4 << 53) | (1 << 51) | (63 << 32) | 0x0000_3000;
    let depth = (0x3Eu64 << 56) | 0x0020_0000;
    submit_commands(
        &mut rdp,
        &[
            scissor, other, fill, fog, blend, prim, env, combine, texture, depth,
        ],
        &mut rdram,
    )
    .unwrap();

    assert_eq!(rdp.scissor.ulx, 0x10);
    assert_eq!(rdp.scissor.uly, 0x20);
    assert!(rdp.scissor.field);
    assert!(!rdp.scissor.odd);
    assert_eq!(rdp.scissor.lrx, 0x100);
    assert_eq!(rdp.scissor.lry, 0x80);
    assert_eq!(rdp.other_modes, (1 << 55) | (3 << 52));
    assert_eq!(rdp.fill_color, 0xAABB_CCDD);
    assert_eq!(rdp.fog_color, 0x0102_0304);
    assert_eq!(rdp.blend_color, 0x1122_3344);
    assert_eq!(rdp.prim_min_level, 0x10);
    assert_eq!(rdp.prim_lod_frac, 0x20);
    assert_eq!(rdp.prim_color, 0x5566_7788);
    assert_eq!(rdp.env_color, 0x99AA_BBCC);
    assert_eq!(rdp.combine_mode, 3 << 52);
    assert_eq!(rdp.texture_image.format, 4);
    assert_eq!(rdp.texture_image.size, 1);
    assert_eq!(rdp.texture_image.width, 63);
    assert_eq!(rdp.texture_image.addr, 0x3000);
    assert_eq!(rdp.depth_image_addr, 0x0020_0000);
}

#[test]
fn truncated_triangle_does_not_run_the_following_word() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    let tri = 0x0Fu64 << 56;
    let color = (0x3Fu64 << 56) | (2 << 53) | (2 << 51) | (319 << 32) | 0x0010_0000;
    let err = submit_commands(&mut rdp, &[tri, color], &mut rdram).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand { cmd_id: 0x0F, .. }
    ));
    assert_eq!(rdp.color_image.addr, 0);
}

#[test]
fn submit_fill_rectangle_writes_known_pixel() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    // RGBA16, 16 pixels wide, DRAM address 0.
    let color = (0x3Fu64 << 56) | (2 << 51) | (15 << 32);
    // FILL cycle (bits 53:52) and RGB dither disabled (bits 39:38).
    let modes = (0x2Fu64 << 56) | (3 << 52) | (3 << 38);
    let fill = (0x37u64 << 56) | 0x1111_2222;
    // FILL scissor: inclusive x through 1.0, exclusive y at 1.0.
    let scissor = (0x2Du64 << 56) | (4 << 12) | 4;
    // Inclusive fill rectangle through x = 1.0, y = 0.0.
    let rect = (0x36u64 << 56) | (4 << 44);
    let action =
        submit_commands(&mut rdp, &[color, modes, fill, scissor, rect], &mut rdram).unwrap();
    assert_eq!(action, RdpAction::None);
    // Even x takes the high half of the fill color; odd x takes the low half.
    assert_eq!(rdram.read_u16(0).unwrap(), 0x1111);
    assert_eq!(rdram.read_u16(2).unwrap(), 0x2222);
    assert_eq!(rdram.read_u16(4).unwrap(), 0);
}

#[test]
fn submit_set_primitive_depth_latches_z() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    let word = (0x2Eu64 << 56) | (0x1234 << 16) | 0x0010;
    submit_commands(&mut rdp, &[word], &mut rdram).unwrap();
    assert_eq!(rdp.prim_z, 0x1234);
    assert_eq!(rdp.prim_dz, 0x0010);
}

#[test]
fn submit_triangle_uses_the_full_command() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    // RGBA32, 16 pixels wide.
    let color = (0x3Fu64 << 56) | (3 << 51) | (15 << 32);
    // Exclusive scissor covering the 16×16 image.
    let scissor = (0x2Du64 << 56) | (64 << 12) | 64;
    // 1-cycle, RGB dither disabled.
    let modes = (0x2Fu64 << 56) | (3 << 38);
    let prim = (0x3Au64 << 56) | 0x00FF_0000;
    // Left-major, zero slopes, exclusive box [0, 4) × [0, 2).
    let header = (0x08u64 << 56) | (1 << 55) | (8 << 32) | (8 << 16);
    let xl = 4u64 << 48;
    let xh = 0u64;
    let xm = 4u64 << 48;
    submit_commands(
        &mut rdp,
        &[color, scissor, modes, prim, header, xl, xh, xm],
        &mut rdram,
    )
    .unwrap();
    // Unshaded 1-cycle writes prim color. Coverage 7 sits in alpha[7:5].
    assert_eq!(rdram.read_u32(0).unwrap(), 0x00FF_00E0);
    assert_eq!(rdram.read_u32(16).unwrap(), 0);
    assert_eq!(rdram.read_u32(2 * 16 * 4).unwrap(), 0);
}

fn dpc_write(bus: &mut Bus, offset: u32, value: u32) -> Result<(), Trap> {
    bus.write_u32(0x0410_0000 + offset, value, CacheAttr::Uncached)
}

#[test]
fn dpc_end_fetches_two_syncfulls_from_rdram_and_raises_dp() {
    let mut bus = Bus::new(false, false);
    let sync = 0x29u64 << 56;
    bus.write_u64(0x0010_0000, sync, CacheAttr::Uncached)
        .unwrap();
    bus.write_u64(0x0010_0008, sync, CacheAttr::Uncached)
        .unwrap();
    dpc_write(&mut bus, 0x00, 0x0010_0000).unwrap();
    dpc_write(&mut bus, 0x04, 0x0010_0010).unwrap();

    assert_eq!(bus.rdp.dpc_current, 0x0010_0010);
    assert_eq!(bus.rdp.dpc_current, bus.rdp.dpc_end);
    assert_eq!(bus.rdp.dpc_status & CpuRdpLle::STATUS_DMA_BUSY, 0);
    assert_eq!(bus.mi.intr & MI_INTR_DP, MI_INTR_DP);
}

#[test]
fn dpc_end_incremental_fetches_the_new_span() {
    let mut bus = Bus::new(false, false);
    let sync = 0x29u64 << 56;
    bus.write_u64(0x0010_0000, sync, CacheAttr::Uncached)
        .unwrap();
    bus.write_u64(0x0010_0008, sync, CacheAttr::Uncached)
        .unwrap();
    dpc_write(&mut bus, 0x00, 0x0010_0000).unwrap();
    dpc_write(&mut bus, 0x04, 0x0010_0008).unwrap();
    assert_eq!(bus.rdp.dpc_current, 0x0010_0008);
    assert_eq!(bus.mi.intr & MI_INTR_DP, MI_INTR_DP);

    dpc_write(&mut bus, 0x04, 0x0010_0010).unwrap();
    assert_eq!(bus.rdp.dpc_current, 0x0010_0010);
    assert_eq!(bus.rdp.dpc_active_end, 0x0010_0010);
    assert_eq!(bus.rdp.dpc_status & CpuRdpLle::STATUS_DMA_BUSY, 0);
}

#[test]
fn dpc_end_xbus_fetches_syncfull_from_sp_dmem() {
    let mut bus = Bus::new(false, false);
    let sync = 0x29u64 << 56;
    bus.sp_dmem[0..8].copy_from_slice(&sync.to_be_bytes());
    bus.sp_dmem[8..16].copy_from_slice(&sync.to_be_bytes());
    // SET_XBUS is status write bit 1.
    dpc_write(&mut bus, 0x0C, 1 << 1).unwrap();
    dpc_write(&mut bus, 0x00, 0).unwrap();
    dpc_write(&mut bus, 0x04, 16).unwrap();

    assert_ne!(bus.rdp.dpc_status & CpuRdpLle::STATUS_XBUS_DMEM_DMA, 0);
    assert_eq!(bus.rdp.dpc_current, 16);
    assert_eq!(bus.mi.intr & MI_INTR_DP, MI_INTR_DP);
    assert_eq!(bus.rdram.read_u64(0).unwrap(), 0);
}

#[test]
fn dpc_freeze_blocks_fetch_until_cleared() {
    let mut bus = Bus::new(false, false);
    let sync = 0x29u64 << 56;
    bus.write_u64(0x0020_0000, sync, CacheAttr::Uncached)
        .unwrap();
    bus.write_u64(0x0020_0008, sync, CacheAttr::Uncached)
        .unwrap();
    dpc_write(&mut bus, 0x0C, 1 << 3).unwrap();
    dpc_write(&mut bus, 0x00, 0x0020_0000).unwrap();
    dpc_write(&mut bus, 0x04, 0x0020_0010).unwrap();

    assert_eq!(bus.rdp.dpc_current, 0);
    assert_ne!(bus.rdp.dpc_status & CpuRdpLle::STATUS_FREEZE, 0);
    assert_eq!(bus.mi.intr & MI_INTR_DP, 0);

    dpc_write(&mut bus, 0x0C, 1 << 2).unwrap();
    assert_eq!(bus.rdp.dpc_status & CpuRdpLle::STATUS_FREEZE, 0);
    assert_eq!(bus.rdp.dpc_current, 0x0020_0010);
    assert_eq!(bus.mi.intr & MI_INTR_DP, MI_INTR_DP);
}

#[test]
fn dpc_end_non_state_opcode_traps() {
    let mut bus = Bus::new(false, false);
    // Set Convert is still outside the walker. A fetched trap must not raise DP.
    let set_convert = 0x2Cu64 << 56;
    bus.write_u64(0x0030_0000, set_convert, CacheAttr::Uncached)
        .unwrap();
    dpc_write(&mut bus, 0x00, 0x0030_0000).unwrap();
    let err = dpc_write(&mut bus, 0x04, 0x0030_0008).unwrap_err();
    assert!(matches!(
        err,
        Trap::UnimplementedRdpCommand {
            cmd_id: 0x2C,
            word,
        } if word == set_convert
    ));
    assert_eq!(bus.mi.intr & MI_INTR_DP, 0);
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
