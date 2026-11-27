// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Unit and WGSL assembly tests for EdgeWalker rasterizer host decoder.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use crate::rdp::{RdpImageState, RdpScissor};

use super::*;

fn default_state(tiles: &[TileDesc; 8]) -> HostRasterState<'_> {
    HostRasterState {
        color_image: RdpImageState {
            format: 0,
            size: 2,
            width: 63,
            addr: 0,
        },
        depth_image_addr: 0x1000,
        scissor: RdpScissor {
            ulx: 0,
            uly: 0,
            lrx: 64 << 2,
            lry: 32 << 2,
            field: false,
            odd: false,
        },
        other_modes: 3 << 52,
        combine_mode: 0,
        fill_color: 0x1111_2222,
        prim_color: 0xFFFF_FFFF,
        env_color: 0,
        fog_color: 0,
        blend_color: 0,
        prim_lod_frac: 0,
        prim_z: 0,
        prim_dz: 0,
        convert_k4: 0,
        convert_k5: 0,
        key_center: [0; 3],
        key_scale: [0; 3],
        tiles,
    }
}

#[test]
fn assemble_raster_wgsl_combines_stages() {
    let wgsl = assemble_raster_wgsl("// tmem_sample", "// combine_blend");
    assert!(wgsl.contains("// tmem_sample"));
    assert!(wgsl.contains("// combine_blend"));
    assert!(wgsl.contains("raster_main"));
}

#[test]
fn primitive_word_count_matches_n64_opcodes() {
    assert_eq!(primitive_word_count(0x36u64 << 56), 1);
    assert_eq!(primitive_word_count(0x24u64 << 56), 2);
    assert_eq!(primitive_word_count(0x25u64 << 56), 2);
    assert_eq!(primitive_word_count(0x08u64 << 56), 4);
    assert_eq!(primitive_word_count(0x0Cu64 << 56), 12);
    assert_eq!(primitive_word_count(0x0Fu64 << 56), 22);
}

#[test]
fn prepare_fill_rectangle_computes_tight_bbox() {
    let tiles = [TileDesc::default(); 8];
    let state = default_state(&tiles);
    // ulx=0, uly=0, lrx=15<<2, lry=15<<2 -> 16x16 inclusive
    let word = (0x36u64 << 56) | ((15u64 * 4) << 44) | ((15u64 * 4) << 32);
    let (pack, threads) = prepare_primitive(&state, &[word])
        .expect("fill ok")
        .expect("non-empty");
    assert_eq!(pack.prim_kind, PRIM_KIND_FILL_RECT);
    assert_eq!(pack.bbox_x0, 0);
    assert_eq!(pack.bbox_y0, 0);
    assert_eq!(pack.bbox_w, 16);
    assert_eq!(pack.bbox_h, 16);
    assert_eq!(threads, 256);
}

#[test]
fn prepare_primitive_enforces_mode_and_alignment_traps() {
    let tiles = [TileDesc::default(); 8];
    let mut state = default_state(&tiles);
    state.other_modes = 2 << 52; // CYCLE_COPY
    let fill = 0x36u64 << 56;
    assert!(matches!(
        prepare_primitive(&state, &[fill]),
        Err(Trap::UnimplementedRdpCommand { cmd_id: 0x36, .. })
    ));

    state.other_modes = 3 << 52; // FILL
    state.color_image.addr = 1; // misaligned for size 2 (16-bit)
    assert!(matches!(
        prepare_primitive(&state, &[fill]),
        Err(Trap::MisalignedAccess { pa: 1, size: 2 })
    ));
}
