// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Bit-exact RTX 4090 GPU parity suite: EdgeWalker (Vulkan compute LLE) == CpuRdpLle.
//!
//! Cited: N64brew — Reality Display Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use super::parity::{ParityFailure, ParityMismatch};

#[cfg(feature = "vulkan-rdp")]
use crate::rdp::RdpSyncKind;

#[cfg(feature = "vulkan-rdp")]
use super::parity::{
    assert_edgewalker_matches_cpu_rdp_lle, compare_edgewalker_to_cpu_rdp_lle,
    fill_rectangle_smoke_cmds, rgba16_replicate_scanout, SYNC_FULL,
};

#[cfg(feature = "vulkan-rdp")]
fn window() -> crate::vi::Vi {
    rgba16_replicate_scanout(16)
}

#[cfg(feature = "vulkan-rdp")]
fn cmd_color_image(format: u8, size: u8, width_px: u16, addr: u32) -> u64 {
    (0x3Fu64 << 56)
        | (u64::from(format & 7) << 53)
        | (u64::from(size & 3) << 51)
        | (u64::from(width_px.saturating_sub(1) & 0x3FF) << 32)
        | u64::from(addr & 0x00FF_FFFF)
}

#[cfg(feature = "vulkan-rdp")]
fn cmd_texture_image(format: u8, size: u8, width_px: u16, addr: u32) -> u64 {
    (0x3Du64 << 56)
        | (u64::from(format & 7) << 53)
        | (u64::from(size & 3) << 51)
        | (u64::from(width_px.saturating_sub(1) & 0x3FF) << 32)
        | u64::from(addr & 0x00FF_FFFF)
}

#[cfg(feature = "vulkan-rdp")]
fn cmd_depth_image(addr: u32) -> u64 {
    (0x3Eu64 << 56) | u64::from(addr & 0x00FF_FFFF)
}

#[cfg(feature = "vulkan-rdp")]
fn cmd_scissor(ulx: u16, uly: u16, lrx: u16, lry: u16, field: bool, odd: bool) -> u64 {
    (0x2Du64 << 56)
        | (u64::from(ulx) << 44)
        | (u64::from(uly) << 32)
        | (u64::from(field) << 25)
        | (u64::from(odd) << 24)
        | (u64::from(lrx) << 12)
        | u64::from(lry)
}

#[cfg(feature = "vulkan-rdp")]
fn cmd_fill_rect(ulx: u16, uly: u16, lrx: u16, lry: u16) -> u64 {
    (0x36u64 << 56)
        | (u64::from(lrx) << 44)
        | (u64::from(lry) << 32)
        | (u64::from(ulx) << 12)
        | u64::from(uly)
}

#[cfg(feature = "vulkan-rdp")]
#[allow(clippy::too_many_arguments)]
fn cmd_tex_rect(
    flip: bool,
    tile: u8,
    ulx: u16,
    uly: u16,
    lrx: u16,
    lry: u16,
    s: i16,
    t: i16,
    dsdx: i16,
    dtdy: i16,
) -> [u64; 2] {
    let op = if flip { 0x25u64 } else { 0x24u64 };
    let w0 = (op << 56)
        | (u64::from(lrx) << 44)
        | (u64::from(lry) << 32)
        | (u64::from(tile & 7) << 24)
        | (u64::from(ulx) << 12)
        | u64::from(uly);
    let w1 = (u64::from(s as u16) << 48)
        | (u64::from(t as u16) << 32)
        | (u64::from(dsdx as u16) << 16)
        | u64::from(dtdy as u16);
    [w0, w1]
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn empty_submit_matches_cpu_rdp_lle() {
    let receipt = assert_edgewalker_matches_cpu_rdp_lle("empty-submit", &[], &[], &window());
    assert_ne!(receipt.scanout_hash, 0);
    assert_ne!(receipt.raw_fb_fnv, 0);
    assert_ne!(receipt.scanout_hash, receipt.raw_fb_fnv);
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn sync_full_only_matches_cpu_rdp_lle() {
    let vi = window();
    let submitted =
        assert_edgewalker_matches_cpu_rdp_lle("sync-full-command", &[SYNC_FULL], &[], &vi);
    let via_sync =
        assert_edgewalker_matches_cpu_rdp_lle("sync-full-barrier", &[], &[RdpSyncKind::Full], &vi);
    let blank = assert_edgewalker_matches_cpu_rdp_lle("empty-submit", &[], &[], &vi);
    assert_eq!(submitted.scanout_hash, blank.scanout_hash);
    assert_eq!(submitted.raw_fb_fnv, blank.raw_fb_fnv);
    assert_eq!(via_sync.scanout_hash, blank.scanout_hash);
    assert_eq!(via_sync.raw_fb_fnv, blank.raw_fb_fnv);
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn fill_rectangle_smoke_matches_cpu_rdp_lle() {
    let vi = window();
    let filled = assert_edgewalker_matches_cpu_rdp_lle(
        "fill-rectangle",
        &fill_rectangle_smoke_cmds(),
        &[],
        &vi,
    );
    let blank = assert_edgewalker_matches_cpu_rdp_lle("empty-submit", &[], &[], &vi);
    assert_ne!(
        filled.scanout_hash, blank.scanout_hash,
        "FillRectangle must change the VI scanout hash"
    );
    assert_ne!(
        filled.raw_fb_fnv, blank.raw_fb_fnv,
        "FillRectangle must change the raw framebuffer FNV"
    );
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn fill_rectangle_rgba32_and_scissor_matches_cpu_rdp_lle() {
    let mut vi = crate::vi::Vi::new();
    vi.write_reg(0x00, (0b11 << 8) | 3).unwrap(); // RGBA32
    vi.write_reg(0x04, 0).unwrap();
    vi.write_reg(0x08, 16).unwrap();
    vi.write_reg(0x28, 8).unwrap(); // 4 active lines
    vi.write_reg(0x34, 0x400).unwrap();

    let cmds = [
        cmd_color_image(0, 3, 16, 0),
        (0x2Fu64 << 56) | (3 << 52) | (3 << 38),
        (0x37u64 << 56) | 0x2244_6688,
        cmd_scissor(1 << 2, 0, 10 << 2, 4 << 2, true, true), // odd lines only
        cmd_fill_rect(0, 0, 12 << 2, 4 << 2),
        SYNC_FULL,
    ];
    let receipt = assert_edgewalker_matches_cpu_rdp_lle("fill-rgba32-scissor", &cmds, &[], &vi);
    assert_ne!(receipt.raw_fb_fnv, 0);
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn load_tile_and_texture_rectangle_matches_cpu_rdp_lle() {
    let mut vi = rgba16_replicate_scanout(16);
    vi.write_reg(0x28, 8).unwrap();

    // Build a 4x4 RGBA16 source texture at RDRAM 0x2000 using FillRectangle stripes,
    // LoadTile into TMEM tile 0, then draw TextureRectangle and TextureRectangleFlip to RDRAM 0x0.
    let mut cmds = vec![
        cmd_color_image(0, 2, 4, 0x2000),
        (0x2Fu64 << 56) | (3 << 52) | (3 << 38), // FILL
        cmd_scissor(0, 0, 16 << 2, 16 << 2, false, false),
        (0x37u64 << 56) | 0xF801_07C1, // red, green
        cmd_fill_rect(0, 0, 3 << 2, 1 << 2),
        (0x37u64 << 56) | 0x003F_FFFF, // blue, white
        cmd_fill_rect(0, 2 << 2, 3 << 2, 3 << 2),
        cmd_texture_image(0, 2, 4, 0x2000),
        // SetTile 0: RGBA16, line=1 (4 RGBA16 texels = 8B/row), tmem=0, mask_s=2, mask_t=2 (4x4 wrap)
        (0x35u64 << 56) | (2u64 << 51) | (1u64 << 41) | (2u64 << 14) | (2u64 << 4),
        (0x34u64 << 56) | (3u64 << 14) | (3u64 << 2), // LoadTile 0..=3 x 0..=3
        cmd_color_image(0, 2, 16, 0),
        // 1-cycle, RGB dither off (3 << 38), blend bypassed via antialias
        (0x2Fu64 << 56) | (3 << 38) | (1 << 3),
        0x3Cu64 << 56, // combine 0 -> sampled texel0 direct
    ];
    let rect0 = cmd_tex_rect(false, 0, 0, 0, 4 << 2, 4 << 2, 0, 0, 1 << 10, 1 << 10);
    cmds.extend_from_slice(&rect0);
    let rect1 = cmd_tex_rect(true, 0, 5 << 2, 0, 9 << 2, 4 << 2, 0, 0, 1 << 10, 1 << 10);
    cmds.extend_from_slice(&rect1);
    cmds.push(SYNC_FULL);

    let receipt = assert_edgewalker_matches_cpu_rdp_lle("load-tile-and-tex-rect", &cmds, &[], &vi);
    assert_ne!(receipt.raw_fb_fnv, 0);
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn load_tlut_ci4_and_load_block_match_cpu_rdp_lle() {
    let mut vi = rgba16_replicate_scanout(16);
    vi.write_reg(0x28, 8).unwrap();

    // Palette at RDRAM 0x2000 (4 RGBA16 colors), CI4 texels at RDRAM 0x3000.
    let mut cmds = vec![
        cmd_color_image(0, 2, 4, 0x2000),
        (0x2Fu64 << 56) | (3 << 52) | (3 << 38),
        cmd_scissor(0, 0, 16 << 2, 16 << 2, false, false),
        (0x37u64 << 56) | 0xF801_07C1,
        cmd_fill_rect(0, 0, 1 << 2, 0),
        (0x37u64 << 56) | 0x003F_FFE1,
        cmd_fill_rect(2 << 2, 0, 3 << 2, 0),
        // Write 16-bit pattern 0x0123 repeated for CI4 data (nibbles 0,1,2,3) at 0x3000.
        cmd_color_image(0, 2, 8, 0x3000),
        (0x37u64 << 56) | 0x0123_0123,
        cmd_fill_rect(0, 0, 7 << 2, 3 << 2),
        // Load TLUT into high TMEM word 0x100 via tile 1 (format CI, size 4-bit)
        cmd_texture_image(0, 2, 4, 0x2000),
        (0x35u64 << 56) | (2u64 << 53) | (0x100u64 << 32) | (1u64 << 24),
        (0x30u64 << 56) | (1u64 << 24) | (3u64 << 14),
        // Load Block 4-bit CI texels using 16-bit texture image (8 texels -> 16 bytes -> dxt=1024 swaps odd lines)
        cmd_texture_image(0, 2, 8, 0x3000),
        (0x35u64 << 56) | (2u64 << 53) | (1u64 << 41) | (3u64 << 14) | (3u64 << 4),
        (0x33u64 << 56) | (7u64 << 12) | 1024u64,
        (0x32u64 << 56) | (7u64 << 14) | (3u64 << 2),
        cmd_color_image(0, 2, 16, 0),
        (0x2Fu64 << 56) | (3 << 38) | (1 << 3),
    ];
    let rect = cmd_tex_rect(false, 0, 0, 0, 8 << 2, 2 << 2, 0, 0, 1 << 10, 1 << 10);
    cmds.extend_from_slice(&rect);
    cmds.push(SYNC_FULL);

    let receipt = assert_edgewalker_matches_cpu_rdp_lle("load-tlut-ci4-block", &cmds, &[], &vi);
    assert_ne!(receipt.raw_fb_fnv, 0);
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn shaded_z_triangle_2cycle_combine_and_blend_match_cpu_rdp_lle() {
    let mut vi = rgba16_replicate_scanout(16);
    vi.write_reg(0x28, 16).unwrap();

    // Initialize 18-bit Z-buffer at RDRAM 0x4000 to max depth (0xFFFF).
    let om = (1u64 << 52) // 2-cycle
        | (1u64 << 38) // Bayer dither
        | (1u64 << 26) // b0 A = fog_a
        | (2u64 << 22) // b0 M = blend_color
        | (1u64 << 24) // b1 A = fog_a
        | (2u64 << 20) // b1 M = blend_color
        | (1u64 << 5)  // z_update
        | (1u64 << 4); // z_compare
                       // 2-cycle combiner: cycle 0 = (SHADE - ENV)*SHADE_ALPHA + 0, cycle 1 = (COMBINED - 0)*SHADE_ALPHA + ENV
    let combine = (4u64 << 52) // c0 a = shade
        | (11u64 << 47) // c0 c = shade_alpha (255)
        | (5u64 << 28) // c0 b = env
        | (7u64 << 15) // c0 d = 0
        | (6u64 << 44) // c0 aa = ONE
        | (7u64 << 41) // c0 ac = 0
        | (7u64 << 12) // c0 ab = 0
        | (7u64 << 9)  // c0 ad = 0
        | (11u64 << 32) // c1 c = shade_alpha (255), c1 a = combined (0), c1 aa = combined.a (0)
        | (15u64 << 24) // c1 b = 0
        | (5u64 << 6) // c1 d = env
        | (7u64 << 18) // c1 ac = 0
        | (7u64 << 3)
        | 6u64; // c1 ad = ONE
    let mut cmds = vec![
        cmd_color_image(0, 2, 16, 0x4000),
        (0x2Fu64 << 56) | (3 << 52) | (3 << 38),
        (0x37u64 << 56) | 0xFFFF_FFFF,
        cmd_scissor(0, 0, 16 << 2, 8 << 2, false, false),
        cmd_fill_rect(0, 0, 15 << 2, 7 << 2),
        cmd_color_image(0, 2, 16, 0),
        cmd_depth_image(0x4000),
        (0x2Fu64 << 56) | om,
        (0x38u64 << 56) | 0x0000_00C0, // fog = (0,0,0,192)
        (0x39u64 << 56) | 0x1020_30FF, // blend = (16,32,48,255)
        (0x3Bu64 << 56) | 0x2020_20FF, // env = (32,32,32,255)
        (0x3Cu64 << 56) | combine,
    ];

    // Left-major shaded + Z triangle (cmd 0x0D: shade + z, lmajor=1):
    // yh=0, ym=3*4, yl=6*4; xh=2<<16, dxhdy=0, xl=4<<16, xm=8<<16, z=0x1000<<16
    let w0 = (0x0Du64 << 56) | (1u64 << 55) | (24u64 << 32) | (12u64 << 16);
    let w1 = 4u64 << 48; // xl = 4, dxldy = 0
    let w2 = 2u64 << 48; // xh = 2, dxhdy = 0
    let w3 = 8u64 << 48; // xm = 8, dxmdy = 0
    let shade = [
        (200u64 << 48) | (100u64 << 32) | (50u64 << 16) | 255u64, // R=200, G=100, B=50, A=255
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let z = [(0x1000u64 << 32) | (0x0100u64 << 16), 0u64]; // z=0x1000, dzdx=0x0100
    cmds.extend_from_slice(&[w0, w1, w2, w3]);
    cmds.extend_from_slice(&shade);
    cmds.extend_from_slice(&z);

    // Overlapping deeper triangle (z=0x2000 -> rejected where first triangle wrote z!)
    let w0b = (0x0Du64 << 56) | (1u64 << 55) | (24u64 << 32) | (12u64 << 16);
    let w1b = 6u64 << 48;
    let w2b = 4u64 << 48;
    let w3b = 12u64 << 48;
    let shade_b = [
        (50u64 << 48) | (220u64 << 32) | (220u64 << 16) | 255u64,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let z_b = [0x2000u64 << 32, 0u64];
    cmds.extend_from_slice(&[w0b, w1b, w2b, w3b]);
    cmds.extend_from_slice(&shade_b);
    cmds.extend_from_slice(&z_b);
    cmds.push(SYNC_FULL);

    let receipt =
        assert_edgewalker_matches_cpu_rdp_lle("shaded-z-tri-2cycle-blend", &cmds, &[], &vi);
    assert_ne!(receipt.raw_fb_fnv, 0);
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn bus_dpc_fifo_edgewalker_backend_raises_dp_intr_and_draws() {
    use crate::mi::MI_INTR_DP;
    use crate::{CacheAttr, N64Config, N64Core, RdpBackendKind};

    let mut cfg = N64Config::accurate();
    cfg.rdp_backend = RdpBackendKind::EdgeWalker;
    let mut core = N64Core::new(cfg);

    let mut cmds = fill_rectangle_smoke_cmds().to_vec();
    cmds.push(SYNC_FULL);
    for (i, &w) in cmds.iter().enumerate() {
        core.bus
            .write_u64(0x0000_2000 + (i as u32) * 8, w, CacheAttr::Uncached)
            .unwrap();
    }

    let end = 0x0000_2000 + (cmds.len() as u32) * 8;
    core.bus
        .write_u32(0x0410_0000, 0x0000_2000, CacheAttr::Uncached)
        .unwrap(); // DPC_START
    core.bus
        .write_u32(0x0410_0004, end, CacheAttr::Uncached)
        .unwrap(); // DPC_END -> pumps DPC FIFO onto EdgeWalker GPU

    assert_ne!(core.bus.mi.intr & MI_INTR_DP, 0);
    assert_eq!(core.bus.rdp.dpc_current, end);
    assert_eq!(core.bus.rdram.read_u16(0).unwrap(), 0x1111);
    assert_eq!(core.bus.rdram.read_u16(2).unwrap(), 0x2222);
    assert_eq!(core.bus.rdram.read_u16(4).unwrap(), 0);
}

#[test]
fn mismatch_report_names_both_frame_hashes() {
    let err = ParityFailure::Mismatch(ParityMismatch {
        case_name: "synthetic",
        cpu_scanout: 0x1111_2222_3333_4444,
        edge_scanout: 0xAAAA_BBBB_CCCC_DDDD,
        cpu_raw_fb: 0x0102_0304_0506_0708,
        edge_raw_fb: 0x090A_0B0C_0D0E_0F10,
        first_byte_diff: Some(0x20),
        first_hidden_diff: Some(0x22),
    });
    let text = err.to_string();
    assert!(text.contains("EdgeWalker (Vulkan compute LLE)"), "{text}");
    assert!(text.contains("CpuRdpLle"), "{text}");
    assert!(text.contains("0x1111222233334444"), "{text}");
    assert!(text.contains("0xaaaabbbbccccdddd"), "{text}");
    assert!(text.contains("0x0102030405060708"), "{text}");
    assert!(text.contains("0x090a0b0c0d0e0f10"), "{text}");
    assert!(text.contains("first byte diff"), "{text}");
    assert!(text.contains("first 9th-bit diff"), "{text}");
    #[cfg(feature = "vulkan-rdp")]
    assert!(compare_edgewalker_to_cpu_rdp_lle("empty-submit", &[], &[], &window()).is_ok());
}
