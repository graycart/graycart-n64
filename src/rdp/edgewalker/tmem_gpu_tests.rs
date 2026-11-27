// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Unit and live GPU compute tests for EdgeWalker TMEM load and texel sampling.
//!
//! Cited: N64brew — Reality Display Processor / Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use super::*;
use crate::rdp::RdpImageState;

#[test]
fn decode_set_tile_and_size_update_descriptor_fields() {
    let mut tiles = [TileDesc::default(); 8];
    let set_tile_word = (0x35u64 << 56) | (2u64 << 51) | (4u64 << 41) | (1u64 << 24);
    decode_set_tile(&mut tiles, set_tile_word).expect("set tile");
    assert_eq!(tiles[1].format, 0);
    assert_eq!(tiles[1].size, 2);
    assert_eq!(tiles[1].line, 4);

    let set_size_word = (0x32u64 << 56) | (1u64 << 24) | (60u64 << 12) | 60u64;
    decode_set_tile_size(&mut tiles, set_size_word).expect("set size");
    assert_eq!(tiles[1].sh, 60);
    assert_eq!(tiles[1].th, 60);
}

#[test]
fn decode_load_tile_and_block_and_tlut_validate_alignment_and_sizes() {
    let mut tiles = [TileDesc::default(); 8];
    tiles[1].size = 2;
    tiles[1].line = 4;

    let img = RdpImageState {
        addr: 0,
        width: 15,
        size: 2,
        format: 0,
    };

    let load_tile = (0x34u64 << 56) | (1u64 << 24) | (60u64 << 12) | 60u64;
    let params = decode_load_tile(&mut tiles, img, load_tile).expect("load tile");
    assert_eq!(params.op, super::super::gpu_types::TMEM_OP_LOAD_TILE);
    assert_eq!(params.width, 16);
    assert_eq!(params.height, 16);

    let bad_img = RdpImageState { addr: 1, ..img };
    assert!(decode_load_tile(&mut tiles, bad_img, load_tile).is_err());

    let load_block = (0x33u64 << 56) | (1u64 << 24) | (7u64 << 12) | 512u64;
    let bparams = decode_load_block(&mut tiles, img, load_block)
        .expect("load block")
        .expect("count <= 2048");
    assert_eq!(bparams.op, super::super::gpu_types::TMEM_OP_LOAD_BLOCK);
    assert_eq!(bparams.width, 8);

    tiles[0].size = 0;
    tiles[0].format = 2;
    let load_tlut = (0x30u64 << 56) | (12u64 << 12);
    let tparams = decode_load_tlut(&mut tiles, img, load_tlut).expect("load tlut");
    assert_eq!(tparams.op, super::super::gpu_types::TMEM_OP_LOAD_TLUT);
    assert_eq!(tparams.width, 4);
}

#[test]
fn validate_tile_sample_accepts_in_bounds_and_traps_unwrapped_out_of_bounds() {
    let mut tile = TileDesc {
        format: 0,
        size: 2,
        sl: 0,
        tl: 0,
        sh: 60,
        th: 60,
        clamp_s: true,
        clamp_t: true,
        ..TileDesc::default()
    };
    validate_tile_sample(&tile, 0, 15 << 5, 15 << 5).expect("clamped valid");

    tile.clamp_s = false;
    assert!(validate_tile_sample(&tile, 0, 16 << 5, 15 << 5).is_err());
}

#[cfg(feature = "vulkan-rdp")]
#[test]
fn gpu_tmem_load_tile_and_sample_rgba16_on_rtx() {
    use super::super::gpu_types::GpuTileDescPack;
    use super::super::ComputeDevice;

    let device = ComputeDevice::open().expect("EdgeWalker compute device");
    let load_pipe = device
        .create_compute_pipeline(TMEM_LOAD_WGSL, "tmem_load_main")
        .expect("compile tmem_load_main");

    let mut tiles = [TileDesc::default(); 8];
    decode_set_tile(
        &mut tiles,
        (0x35u64 << 56) | (2u64 << 51) | (1u64 << 41), // RGBA16, line=1 (8 bytes/row)
    )
    .unwrap();
    let img = RdpImageState {
        format: 0,
        size: 2,
        width: 1, // 2 pixels wide
        addr: 0,
    };
    let lparams = decode_load_tile(
        &mut tiles,
        img,
        (0x34u64 << 56) | (4u64 << 12) | 4u64, // 2x2 texels
    )
    .unwrap();

    // 4 RGBA16 pixels in RDRAM: (0,0)=0xF801 (red), (1,0)=0x07C1 (green), (0,1)=0x003F (blue), (1,1)=0xFFFF (white)
    let rdram = [0xF8u8, 0x01, 0x07, 0xC1, 0x00, 0x3F, 0xFF, 0xFF];
    let lp_buf = device.create_storage_buffer(64).unwrap();
    device
        .write_buffer(&lp_buf, 0, &lparams.to_bytes())
        .unwrap();
    let rdram_buf = device.create_storage_buffer(8).unwrap();
    device.write_buffer(&rdram_buf, 0, &rdram).unwrap();
    let tmem_buf = device.create_storage_buffer(4096).unwrap();
    device.write_buffer(&tmem_buf, 0, &[0u8; 4096]).unwrap();

    let group = device
        .create_storage_bind_group(
            &load_pipe,
            0,
            &[(0, &lp_buf), (1, &rdram_buf), (2, &tmem_buf)],
        )
        .unwrap();
    device
        .dispatch_workgroups(&load_pipe, Some(&group), 1, 1, 1)
        .unwrap();

    // Sample row 1 col 0 (blue, odd-row swapped to offset +4)
    let sample_wgsl = format!(
        "{TMEM_SAMPLE_WGSL}\n
@group(0) @binding(0) var<storage, read_write> out_rgba: array<u32>;
@compute @workgroup_size(1)
fn sample_test_main() {{
    let p = sample_texel(0u, 0, 32, 0u);
    out_rgba[0] = p.x;
    out_rgba[1] = p.y;
    out_rgba[2] = p.z;
    out_rgba[3] = p.w;
}}
"
    );
    let sample_pipe = device
        .create_compute_pipeline(&sample_wgsl, "sample_test_main")
        .expect("compile tmem_sample");
    let tiles_bytes = GpuTileDescPack::pack_all_tiles(&tiles);
    let tiles_buf = device
        .create_storage_buffer(tiles_bytes.len() as u64)
        .unwrap();
    device.write_buffer(&tiles_buf, 0, &tiles_bytes).unwrap();
    let out_buf = device.create_storage_buffer(16).unwrap();
    let sgroup = device
        .create_storage_bind_group(
            &sample_pipe,
            0,
            &[(0, &out_buf), (1, &tiles_buf), (2, &tmem_buf)],
        )
        .unwrap();
    device
        .dispatch_workgroups(&sample_pipe, Some(&sgroup), 1, 1, 1)
        .unwrap();
    let got = device.readback(&out_buf).unwrap();
    let rgba = [
        u32::from_le_bytes(got[0..4].try_into().unwrap()),
        u32::from_le_bytes(got[4..8].try_into().unwrap()),
        u32::from_le_bytes(got[8..12].try_into().unwrap()),
        u32::from_le_bytes(got[12..16].try_into().unwrap()),
    ];
    assert_eq!(rgba, [0, 0, 255, 255]);
}
