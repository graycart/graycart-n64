// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors
//
// EdgeWalker — Graycart's bespoke Vulkan compute LLE RDP.
// GPU TMEM load and sample host decoders and validators.
// Bespoke Graycart module. No third-party RDP code.
//
// Cited: N64brew — Reality Display Processor / Commands
//   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//   Load Tile, Load Block, Load TLUT, Set Tile, Set Tile Size.

#[cfg(test)]
#[path = "tmem_gpu_tests.rs"]
mod tests;

use super::gpu_types::TmemLoadParamsPack;
use crate::rdp::tmem::TileDesc;
use crate::rdp::RdpImageState;
use crate::trap::Trap;

pub const TMEM_LOAD_WGSL: &str = include_str!("shaders/tmem_load.wgsl");
pub const TMEM_SAMPLE_WGSL: &str = include_str!("shaders/tmem_sample.wgsl");

/// Decode `Set Tile` (`0x35`) into tile slots.
pub fn decode_set_tile(tiles: &mut [TileDesc; 8], word: u64) -> Result<(), Trap> {
    if ((word >> 56) & 0x3F) as u8 != crate::rdp::tmem::CMD_SET_TILE {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: ((word >> 56) & 0x3F) as u8,
            word,
        });
    }
    let index = ((word >> 24) & 7) as usize;
    let prev = tiles[index];
    tiles[index] = TileDesc {
        format: ((word >> 53) & 7) as u8,
        size: ((word >> 51) & 3) as u8,
        line: ((word >> 41) & 0x1FF) as u16,
        tmem_addr: ((word >> 32) & 0x1FF) as u16,
        palette: ((word >> 20) & 0xF) as u8,
        clamp_t: word & (1 << 19) != 0,
        mirror_t: word & (1 << 18) != 0,
        mask_t: ((word >> 14) & 0xF) as u8,
        shift_t: ((word >> 10) & 0xF) as u8,
        clamp_s: word & (1 << 9) != 0,
        mirror_s: word & (1 << 8) != 0,
        mask_s: ((word >> 4) & 0xF) as u8,
        shift_s: (word & 0xF) as u8,
        sl: prev.sl,
        tl: prev.tl,
        sh: prev.sh,
        th: prev.th,
    };
    Ok(())
}

/// Decode `Set Tile Size` (`0x32`) into tile extents.
pub fn decode_set_tile_size(tiles: &mut [TileDesc; 8], word: u64) -> Result<(), Trap> {
    if ((word >> 56) & 0x3F) as u8 != crate::rdp::tmem::CMD_SET_TILE_SIZE {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: ((word >> 56) & 0x3F) as u8,
            word,
        });
    }
    let uls = ((word >> 44) & 0xFFF) as u16;
    let ult = ((word >> 32) & 0xFFF) as u16;
    let tile = ((word >> 24) & 7) as usize;
    let lrs = ((word >> 12) & 0xFFF) as u16;
    let lrt = (word & 0xFFF) as u16;
    let t = &mut tiles[tile];
    t.sl = uls;
    t.tl = ult;
    t.sh = lrs;
    t.th = lrt;
    Ok(())
}

/// Decode `Load Tile` (`0x34`).
pub fn decode_load_tile(
    tiles: &mut [TileDesc; 8],
    img: RdpImageState,
    word: u64,
) -> Result<TmemLoadParamsPack, Trap> {
    if ((word >> 56) & 0x3F) as u8 != crate::rdp::tmem::CMD_LOAD_TILE {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: ((word >> 56) & 0x3F) as u8,
            word,
        });
    }
    let uls = ((word >> 44) & 0xFFF) as u16;
    let ult = ((word >> 32) & 0xFFF) as u16;
    let tile_idx = ((word >> 24) & 7) as usize;
    let lrs = ((word >> 12) & 0xFFF) as u16;
    let lrt = (word & 0xFFF) as u16;

    if (uls | lrs | ult | lrt) & 3 != 0 || lrs < uls || lrt < ult {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: crate::rdp::tmem::CMD_LOAD_TILE,
            word,
        });
    }
    let s0 = u32::from(uls >> 2);
    let t0 = u32::from(ult >> 2);
    let width = u32::from((lrs - uls) >> 2) + 1;
    let height = u32::from((lrt - ult) >> 2) + 1;

    let tile = tiles[tile_idx];
    if tile.size != img.size || img.addr & 7 != 0 || tile.size > 3 {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: crate::rdp::tmem::CMD_LOAD_TILE,
            word,
        });
    }
    tiles[tile_idx].sl = uls;
    tiles[tile_idx].tl = ult;
    tiles[tile_idx].sh = lrs;
    tiles[tile_idx].th = lrt;

    Ok(TmemLoadParamsPack {
        op: super::gpu_types::TMEM_OP_LOAD_TILE,
        img_addr: img.addr,
        img_width: u32::from(img.width) + 1,
        img_size: u32::from(img.size),
        tile_size: u32::from(tile.size),
        tile_line: u32::from(tile.line),
        tile_tmem: u32::from(tile.tmem_addr),
        s0,
        t0,
        width,
        height,
        dxt: 0,
        transfer: 0,
        pad0: 0,
        pad1: 0,
        pad2: 0,
    })
}

/// Decode `Load Block` (`0x33`). Returns `Ok(None)` if texel count > 2048.
pub fn decode_load_block(
    tiles: &mut [TileDesc; 8],
    img: RdpImageState,
    word: u64,
) -> Result<Option<TmemLoadParamsPack>, Trap> {
    if ((word >> 56) & 0x3F) as u8 != crate::rdp::tmem::CMD_LOAD_BLOCK {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: ((word >> 56) & 0x3F) as u8,
            word,
        });
    }
    let uls = ((word >> 44) & 0xFFF) as u16;
    let ult = ((word >> 32) & 0xFFF) as u16;
    let tile_idx = ((word >> 24) & 7) as usize;
    let lrs = ((word >> 12) & 0xFFF) as u16;
    let dxt = (word & 0xFFF) as u32;

    if lrs < uls {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: crate::rdp::tmem::CMD_LOAD_BLOCK,
            word,
        });
    }
    let count = u32::from(lrs - uls) + 1;
    if img.addr & 7 != 0 {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: crate::rdp::tmem::CMD_LOAD_BLOCK,
            word,
        });
    }
    let transfer = match (img.size, tiles[tile_idx].size) {
        (0, 0) => 0u32,
        (1, 1) => 1u32,
        (2, 2) | (2, 0) | (2, 1) => 2u32,
        _ => {
            return Err(Trap::UnimplementedRdpCommand {
                cmd_id: crate::rdp::tmem::CMD_LOAD_BLOCK,
                word,
            })
        }
    };

    tiles[tile_idx].sl = uls;
    tiles[tile_idx].tl = ult;
    tiles[tile_idx].sh = lrs;
    tiles[tile_idx].th = dxt as u16;

    if count > 2048 {
        return Ok(None);
    }

    let tile = tiles[tile_idx];
    Ok(Some(TmemLoadParamsPack {
        op: super::gpu_types::TMEM_OP_LOAD_BLOCK,
        img_addr: img.addr,
        img_width: u32::from(img.width) + 1,
        img_size: u32::from(img.size),
        tile_size: u32::from(tile.size),
        tile_line: u32::from(tile.line),
        tile_tmem: u32::from(tile.tmem_addr),
        s0: u32::from(uls),
        t0: u32::from(ult),
        width: count,
        height: 1,
        dxt,
        transfer,
        pad0: 0,
        pad1: 0,
        pad2: 0,
    }))
}

/// Decode `Load TLUT` (`0x30`).
pub fn decode_load_tlut(
    tiles: &mut [TileDesc; 8],
    img: RdpImageState,
    word: u64,
) -> Result<TmemLoadParamsPack, Trap> {
    if ((word >> 56) & 0x3F) as u8 != crate::rdp::tmem::CMD_LOAD_TLUT {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: ((word >> 56) & 0x3F) as u8,
            word,
        });
    }
    let uls = ((word >> 44) & 0xFFF) as u16;
    let ult = ((word >> 32) & 0xFFF) as u16;
    let tile_idx = ((word >> 24) & 7) as usize;
    let lrs = ((word >> 12) & 0xFFF) as u16;
    let lrt = (word & 0xFFF) as u16;

    if (uls | lrs | ult | lrt) & 3 != 0 || lrs < uls || lrt < ult {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: crate::rdp::tmem::CMD_LOAD_TLUT,
            word,
        });
    }
    let tile = tiles[tile_idx];
    if tile.size != 0 || tile.format == 0 || tile.format == 1 || img.size != 2 || img.addr & 7 != 0
    {
        return Err(Trap::UnimplementedRdpCommand {
            cmd_id: crate::rdp::tmem::CMD_LOAD_TLUT,
            word,
        });
    }
    tiles[tile_idx].sl = uls;
    tiles[tile_idx].tl = ult;
    tiles[tile_idx].sh = lrs;
    tiles[tile_idx].th = lrt;

    let s0 = u32::from(uls >> 2);
    let t0 = u32::from(ult >> 2);
    let width = u32::from((lrs - uls) >> 2) + 1;
    let height = u32::from((lrt - ult) >> 2) + 1;

    Ok(TmemLoadParamsPack {
        op: super::gpu_types::TMEM_OP_LOAD_TLUT,
        img_addr: img.addr,
        img_width: u32::from(img.width) + 1,
        img_size: u32::from(img.size),
        tile_size: u32::from(tile.size),
        tile_line: u32::from(tile.line),
        tile_tmem: u32::from(tile.tmem_addr),
        s0,
        t0,
        width,
        height,
        dxt: 0,
        transfer: 0,
        pad0: 0,
        pad1: 0,
        pad2: 0,
    })
}

/// Validate tile format and sampling coordinates against N64 RDP hardware rules.
pub fn validate_tile_sample(
    tile: &TileDesc,
    tile_idx: u8,
    s_in: i32,
    t_in: i32,
) -> Result<(), Trap> {
    let kind = match (tile.format, tile.size) {
        (0, 2) | (0, 3) | (2, 0) | (2, 1) | (3, 0) | (3, 1) | (3, 2) => true,
        (fmt, 0) | (fmt, 1) if fmt >= 4 => true,
        _ => false,
    };
    let trap = || Trap::UnimplementedRdpCommand {
        cmd_id: crate::rdp::tmem::CMD_SET_TILE,
        word: (u64::from(crate::rdp::tmem::CMD_SET_TILE) << 56)
            | (u64::from(tile.format) << 53)
            | (u64::from(tile.size) << 51)
            | (u64::from(tile_idx) << 24),
    };
    if !kind {
        return Err(trap());
    }
    validate_axis(
        s_in,
        tile.shift_s,
        tile.mask_s,
        tile.clamp_s,
        tile.sl,
        tile.sh,
    )
    .map_err(|()| trap())?;
    validate_axis(
        t_in,
        tile.shift_t,
        tile.mask_t,
        tile.clamp_t,
        tile.tl,
        tile.th,
    )
    .map_err(|()| trap())?;
    Ok(())
}

fn validate_axis(
    coord: i32,
    shift: u8,
    mask: u8,
    clamp_en: bool,
    lo: u16,
    hi: u16,
) -> Result<(), ()> {
    let c = i32::from(coord as i16);
    let shifted = match shift & 0xF {
        0 => c,
        n @ 1..=10 => c >> n,
        n => c.wrapping_shl(u32::from(16 - n)),
    };
    let sl = i32::from(lo >> 2);
    let sh = i32::from(hi >> 2);
    if sl > sh {
        return Err(());
    }
    let s = shifted >> 5;
    let m = mask & 0xF;
    if m == 0 {
        if !clamp_en && (s < sl || s > sh) {
            return Err(());
        }
        return Ok(());
    }
    if sl != 0 {
        return Err(());
    }
    Ok(())
}
