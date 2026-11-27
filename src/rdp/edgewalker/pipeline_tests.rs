// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Live GPU tests covering all four EdgeWalker compute pipeline stages.
//!
//! Written directly against N64brew RDP and VI hardware specs.
//! Cited:
//!   - https://n64brew.dev/wiki/Reality_Display_Processor
//!   - https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   - https://n64brew.dev/wiki/Video_Interface

use super::{
    BinGridParams, ComputeDevice, Pipeline, PrimBounds, SpanParamsPack, ViFramebuffer,
    ViParamsPack, COLOR_IMAGE_SIZE_16, CYCLE_TYPE_FILL, INGEST_KIND_FULL, INGEST_KIND_LOAD,
    INGEST_KIND_NONE, INGEST_KIND_PIPE, INGEST_KIND_TILE, VI_COLOR_RGBA16,
};

#[test]
fn dispatch_ingest_classifies_all_four_sync_barriers_on_gpu() {
    let device = ComputeDevice::open().expect("EdgeWalker compute device");
    let words = [
        0x37u64 << 56, // SetFillColor -> INGEST_KIND_NONE
        0x27u64 << 56, // SyncPipe
        0x28u64 << 56, // SyncTile
        0x26u64 << 56, // SyncLoad
        0x29u64 << 56, // SyncFull
    ];
    let mut pipe = Pipeline::new();
    let pairs = pipe
        .dispatch_ingest(&device, &words)
        .expect("Pipeline::dispatch_ingest");
    assert_eq!(
        pairs,
        vec![
            (INGEST_KIND_NONE, 0),
            (INGEST_KIND_PIPE, 1),
            (INGEST_KIND_TILE, 2),
            (INGEST_KIND_LOAD, 3),
            (INGEST_KIND_FULL, 4),
        ]
    );
    assert_eq!(pipe.snapshots().len(), 4);
    eprintln!(
        "Pipeline::dispatch_ingest Ok ({})",
        device.adapter_info().name
    );
}

#[test]
fn dispatch_tile_bin_bins_primitives_on_gpu() {
    let device = ComputeDevice::open().expect("EdgeWalker compute device");
    let prims = [
        PrimBounds {
            x0: 0,
            y0: 0,
            x1: 16,
            y1: 16,
        },
        PrimBounds {
            x0: 16,
            y0: 16,
            x1: 48,
            y1: 48,
        },
    ];
    let grid = BinGridParams {
        bins_x: 2,
        bins_y: 2,
        bin_size: 32,
        max_bins_per_prim: 4,
    };
    let pass = Pipeline::new()
        .dispatch_tile_bin(&device, &prims, &grid)
        .expect("Pipeline::dispatch_tile_bin");

    assert_eq!(pass.bins.len(), 4);
    assert_eq!(pass.bins[0].primitive_ids, vec![0, 1]);
    assert_eq!(pass.bins[1].primitive_ids, vec![1]);
    assert_eq!(pass.bins[2].primitive_ids, vec![1]);
    assert_eq!(pass.bins[3].primitive_ids, vec![1]);
    eprintln!(
        "Pipeline::dispatch_tile_bin Ok ({})",
        device.adapter_info().name
    );
}

#[test]
fn dispatch_span_fill_smoke_does_not_fault_device() {
    let device = ComputeDevice::open().expect("EdgeWalker compute device");
    let params = SpanParamsPack {
        other_modes_lo: 0,
        other_modes_hi: CYCLE_TYPE_FILL << 20,
        combine_lo: 0,
        combine_hi: 0,
        fill_color: 0x1111_2222,
        color_dram_addr: 0,
        color_width_px: 16,
        color_size: COLOR_IMAGE_SIZE_16,
        rect_ulx: 0,
        rect_uly: 0,
        rect_lrx: 4,
        rect_lry: 0,
        scissor_ulx: 0,
        scissor_uly: 0,
        scissor_lrx: 4,
        scissor_lry: 4,
        scissor_field: 0,
        scissor_odd: 0,
    };
    let packed = params.to_bytes();
    let mut color = [0u8; 64];
    let mut coverage = [0u8; 0];
    let mut depth = [0u8; 0];
    Pipeline::new()
        .dispatch_span(&device, &packed, &mut color, &mut coverage, &mut depth)
        .expect("Pipeline::dispatch_span");
    assert_eq!(&color[..4], &[0x11, 0x11, 0x22, 0x22]);
    assert!(color[4..].iter().all(|&byte| byte == 0));
    eprintln!(
        "Pipeline::dispatch_span fill smoke Ok ({})",
        device.adapter_info().name
    );
}

#[test]
fn dispatch_vi_rgba16_replicate_scanout_on_gpu() {
    let device = ComputeDevice::open().expect("EdgeWalker compute device");
    let params = ViParamsPack {
        origin: 0,
        width: 4,
        height: 1,
        color_type: VI_COLOR_RGBA16,
    };
    let color = [0xF8, 0x01, 0x07, 0xC1, 0x00, 0x3F, 0x00, 0x00];
    let mut out = ViFramebuffer::default();
    Pipeline::new()
        .dispatch_vi(&device, &color, &[], &params, &mut out)
        .expect("Pipeline::dispatch_vi");

    assert_eq!(out.width, 4);
    assert_eq!(out.height, 1);
    assert_eq!(
        out.rgba8888,
        vec![0xFF, 0, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0]
    );
    eprintln!("Pipeline::dispatch_vi Ok ({})", device.adapter_info().name);
}
