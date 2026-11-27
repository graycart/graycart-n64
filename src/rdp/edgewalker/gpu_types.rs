// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Binary-packed host↔WGSL parameter structures for EdgeWalker Wave 2.
//!
//! All field layouts use explicit 32-bit little-endian words so WGSL storage
//! buffers map 1:1 with zero implicit alignment padding.
//!
//! Cited: N64brew — Reality Display Processor / Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use crate::rdp::tmem::TileDesc;

/// Stride in bytes of one WGSL `GpuTileDesc` (`8 × u32`).
pub const TILE_DESC_BYTES: usize = 32;

/// Number of hardware tile descriptors (`Set Tile` 0..=7).
pub const TILE_SLOT_COUNT: usize = 8;

/// Total byte size of the 8-tile descriptor buffer (`8 × 32 = 256`).
pub const ALL_TILES_BYTES: usize = TILE_DESC_BYTES * TILE_SLOT_COUNT;

/// Stride in bytes of one WGSL `TmemLoadParams` (`16 × u32`).
pub const TMEM_LOAD_PARAMS_BYTES: usize = 64;

/// Stride in bytes of one WGSL `CombineState` (`16 × u32`).
pub const COMBINE_STATE_BYTES: usize = 64;

/// Stride in bytes of one WGSL `RasterParams` (`72 × u32`).
pub const RASTER_PARAMS_BYTES: usize = 288;

/// TMEM load operation selector passed in `TmemLoadParamsPack::op`.
pub const TMEM_OP_LOAD_TILE: u32 = 0;
pub const TMEM_OP_LOAD_BLOCK: u32 = 1;
pub const TMEM_OP_LOAD_TLUT: u32 = 2;

/// `LoadBlock` transfer encoding passed in `TmemLoadParamsPack::transfer`.
pub const TMEM_TRANSFER_NIBBLE: u32 = 0;
pub const TMEM_TRANSFER_BYTE: u32 = 1;
pub const TMEM_TRANSFER_HALFWORD: u32 = 2;

/// Primitive selector passed in `RasterParamsPack::prim_kind`.
pub const PRIM_KIND_FILL_RECT: u32 = 0;
pub const PRIM_KIND_TEX_RECT: u32 = 1;
pub const PRIM_KIND_TEX_RECT_FLIP: u32 = 2;
pub const PRIM_KIND_TRIANGLE: u32 = 3;

/// Primitive flag bits packed into `RasterParamsPack::prim_flags`.
pub const PRIM_FLAG_LMAJOR: u32 = 1 << 0;
pub const PRIM_FLAG_SHADED: u32 = 1 << 1;
pub const PRIM_FLAG_TEXTURED: u32 = 1 << 2;
pub const PRIM_FLAG_HAS_Z: u32 = 1 << 3;
pub const PRIM_FLAG_TILE_SHIFT: u32 = 4;

/// Packed hardware tile descriptor matching WGSL `struct GpuTileDesc` (32 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GpuTileDescPack {
    pub format: u32,
    pub size: u32,
    pub line: u32,
    pub tmem_addr: u32,
    /// Bits: `[3:0]=palette`, `[4]=clamp_t`, `[5]=mirror_t`, `[9:6]=mask_t`,
    /// `[13:10]=shift_t`, `[14]=clamp_s`, `[15]=mirror_s`, `[19:16]=mask_s`,
    /// `[23:20]=shift_s`.
    pub flags: u32,
    /// `(sl & 0xFFFF) | ((tl & 0xFFFF) << 16)`.
    pub sl_tl: u32,
    /// `(sh & 0xFFFF) | ((th & 0xFFFF) << 16)`.
    pub sh_th: u32,
    pub pad: u32,
}

impl GpuTileDescPack {
    /// Convert a [`TileDesc`] into its 32-byte WGSL storage representation.
    #[must_use]
    pub fn from_tile(t: &TileDesc) -> Self {
        let flags = u32::from(t.palette & 0xF)
            | (u32::from(t.clamp_t) << 4)
            | (u32::from(t.mirror_t) << 5)
            | (u32::from(t.mask_t & 0xF) << 6)
            | (u32::from(t.shift_t & 0xF) << 10)
            | (u32::from(t.clamp_s) << 14)
            | (u32::from(t.mirror_s) << 15)
            | (u32::from(t.mask_s & 0xF) << 16)
            | (u32::from(t.shift_s & 0xF) << 20);
        Self {
            format: u32::from(t.format),
            size: u32::from(t.size),
            line: u32::from(t.line),
            tmem_addr: u32::from(t.tmem_addr),
            flags,
            sl_tl: u32::from(t.sl) | (u32::from(t.tl) << 16),
            sh_th: u32::from(t.sh) | (u32::from(t.th) << 16),
            pad: 0,
        }
    }

    /// Encode into 32 little-endian bytes.
    #[must_use]
    pub fn to_bytes(self) -> [u8; TILE_DESC_BYTES] {
        let words = [
            self.format,
            self.size,
            self.line,
            self.tmem_addr,
            self.flags,
            self.sl_tl,
            self.sh_th,
            self.pad,
        ];
        pack_u32_words(&words)
    }

    /// Pack all 8 hardware tiles into a 256-byte WGSL storage upload.
    #[must_use]
    pub fn pack_all_tiles(tiles: &[TileDesc; TILE_SLOT_COUNT]) -> [u8; ALL_TILES_BYTES] {
        let mut out = [0u8; ALL_TILES_BYTES];
        for (idx, tile) in tiles.iter().enumerate() {
            let start = idx * TILE_DESC_BYTES;
            out[start..start + TILE_DESC_BYTES].copy_from_slice(&Self::from_tile(tile).to_bytes());
        }
        out
    }
}

/// Packed TMEM DMA load descriptor matching WGSL `struct TmemLoadParams` (64 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TmemLoadParamsPack {
    pub op: u32,
    pub img_addr: u32,
    pub img_width: u32,
    pub img_size: u32,
    pub tile_size: u32,
    pub tile_line: u32,
    pub tile_tmem: u32,
    pub s0: u32,
    pub t0: u32,
    pub width: u32,
    pub height: u32,
    pub dxt: u32,
    pub transfer: u32,
    pub pad0: u32,
    pub pad1: u32,
    pub pad2: u32,
}

impl TmemLoadParamsPack {
    /// Encode into 64 little-endian bytes.
    #[must_use]
    pub fn to_bytes(self) -> [u8; TMEM_LOAD_PARAMS_BYTES] {
        let words = [
            self.op,
            self.img_addr,
            self.img_width,
            self.img_size,
            self.tile_size,
            self.tile_line,
            self.tile_tmem,
            self.s0,
            self.t0,
            self.width,
            self.height,
            self.dxt,
            self.transfer,
            self.pad0,
            self.pad1,
            self.pad2,
        ];
        pack_u32_words(&words)
    }
}

/// Packed combiner/blender register snapshot matching WGSL `struct CombineState` (64 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CombineStatePack {
    pub combine_lo: u32,
    pub combine_hi: u32,
    pub other_modes_lo: u32,
    pub other_modes_hi: u32,
    pub prim_color: u32,
    pub env_color: u32,
    pub fog_color: u32,
    pub blend_color: u32,
    pub prim_lod_frac: u32,
    pub convert_k4: u32,
    pub convert_k5: u32,
    pub key_center_rgb: u32,
    pub key_scale_rgb: u32,
    pub pad0: u32,
    pub pad1: u32,
    pub pad2: u32,
}

impl CombineStatePack {
    /// Encode into 64 little-endian bytes.
    #[must_use]
    pub fn to_bytes(self) -> [u8; COMBINE_STATE_BYTES] {
        let words = [
            self.combine_lo,
            self.combine_hi,
            self.other_modes_lo,
            self.other_modes_hi,
            self.prim_color,
            self.env_color,
            self.fog_color,
            self.blend_color,
            self.prim_lod_frac,
            self.convert_k4,
            self.convert_k5,
            self.key_center_rgb,
            self.key_scale_rgb,
            self.pad0,
            self.pad1,
            self.pad2,
        ];
        pack_u32_words(&words)
    }
}

/// Full primitive + pipeline parameter block matching WGSL `struct RasterParams` (288 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RasterParamsPack {
    // [0..6] Dispatch primitive header & bounding box
    pub prim_kind: u32,
    pub prim_flags: u32,
    pub bbox_x0: i32,
    pub bbox_y0: i32,
    pub bbox_w: u32,
    pub bbox_h: u32,
    // [6..19] Combiner, blender, fill & constant color state
    pub other_modes_lo: u32,
    pub other_modes_hi: u32,
    pub combine_lo: u32,
    pub combine_hi: u32,
    pub fill_color: u32,
    pub prim_color: u32,
    pub env_color: u32,
    pub fog_color: u32,
    pub blend_color: u32,
    pub prim_lod_frac: u32,
    pub prim_z: u32,
    pub prim_dz: u32,
    pub color_addr: u32,
    // [19..29] Targets & scissor
    pub color_width_px: u32,
    pub color_size: u32,
    pub color_format: u32,
    pub depth_addr: u32,
    pub scissor_ulx: u32,
    pub scissor_uly: u32,
    pub scissor_lrx: u32,
    pub scissor_lry: u32,
    pub scissor_field: u32,
    pub scissor_odd: u32,
    // [29..33] Convert / Chroma-key constants
    pub convert_k4: u32,
    pub convert_k5: u32,
    pub key_center_rgb: u32,
    pub key_scale_rgb: u32,
    // [33..41] Rectangle geometry & texture-rectangle derivatives (s10.2 / s5.10)
    pub rect_ulx: i32,
    pub rect_uly: i32,
    pub rect_lrx: i32,
    pub rect_lry: i32,
    pub rect_s: i32,
    pub rect_t: i32,
    pub rect_dsdx: i32,
    pub rect_dtdy: i32,
    // [41..50] Triangle Y endpoints (s11.2) and X edges / slopes (s15.16)
    pub yh: i32,
    pub ym: i32,
    pub yl: i32,
    pub xh: i32,
    pub xm: i32,
    pub xl: i32,
    pub dxhdy: i32,
    pub dxmdy: i32,
    pub dxldy: i32,
    // [50..62] Triangle Gouraud shade anchor and gradients (s8.16 / s15.16)
    pub shade_r: i32,
    pub shade_g: i32,
    pub shade_b: i32,
    pub shade_a: i32,
    pub drdx_r: i32,
    pub drdx_g: i32,
    pub drdx_b: i32,
    pub drdx_a: i32,
    pub drde_r: i32,
    pub drde_g: i32,
    pub drde_b: i32,
    pub drde_a: i32,
    // [62..71] Triangle depth and texture anchor + gradients
    pub z_anchor: i32,
    pub dzdx: i32,
    pub dzde: i32,
    pub tex_s: i32,
    pub tex_t: i32,
    pub tex_dsdx: i32,
    pub tex_dtdx: i32,
    pub tex_dsde: i32,
    pub tex_dtde: i32,
    // [71] Padding to 288 bytes (72 × 4)
    pub pad0: u32,
}

impl RasterParamsPack {
    /// Encode into 288 little-endian bytes (`72 × u32`).
    #[must_use]
    pub fn to_bytes(self) -> [u8; RASTER_PARAMS_BYTES] {
        let words = [
            self.prim_kind,
            self.prim_flags,
            self.bbox_x0 as u32,
            self.bbox_y0 as u32,
            self.bbox_w,
            self.bbox_h,
            self.other_modes_lo,
            self.other_modes_hi,
            self.combine_lo,
            self.combine_hi,
            self.fill_color,
            self.prim_color,
            self.env_color,
            self.fog_color,
            self.blend_color,
            self.prim_lod_frac,
            self.prim_z,
            self.prim_dz,
            self.color_addr,
            self.color_width_px,
            self.color_size,
            self.color_format,
            self.depth_addr,
            self.scissor_ulx,
            self.scissor_uly,
            self.scissor_lrx,
            self.scissor_lry,
            self.scissor_field,
            self.scissor_odd,
            self.convert_k4,
            self.convert_k5,
            self.key_center_rgb,
            self.key_scale_rgb,
            self.rect_ulx as u32,
            self.rect_uly as u32,
            self.rect_lrx as u32,
            self.rect_lry as u32,
            self.rect_s as u32,
            self.rect_t as u32,
            self.rect_dsdx as u32,
            self.rect_dtdy as u32,
            self.yh as u32,
            self.ym as u32,
            self.yl as u32,
            self.xh as u32,
            self.xm as u32,
            self.xl as u32,
            self.dxhdy as u32,
            self.dxmdy as u32,
            self.dxldy as u32,
            self.shade_r as u32,
            self.shade_g as u32,
            self.shade_b as u32,
            self.shade_a as u32,
            self.drdx_r as u32,
            self.drdx_g as u32,
            self.drdx_b as u32,
            self.drdx_a as u32,
            self.drde_r as u32,
            self.drde_g as u32,
            self.drde_b as u32,
            self.drde_a as u32,
            self.z_anchor as u32,
            self.dzdx as u32,
            self.dzde as u32,
            self.tex_s as u32,
            self.tex_t as u32,
            self.tex_dsdx as u32,
            self.tex_dtdx as u32,
            self.tex_dsde as u32,
            self.tex_dtde as u32,
            self.pad0,
        ];
        pack_u32_words(&words)
    }
}

fn pack_u32_words<const N: usize, const B: usize>(words: &[u32; N]) -> [u8; B] {
    let mut out = [0u8; B];
    for (i, word) in words.iter().enumerate() {
        let off = i * 4;
        out[off..off + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}
