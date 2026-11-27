//! 4 KiB TMEM, eight tile descriptors, and the point texel sampler.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   Set Tile, Set Tile Size, Load Block, Load Tile, Load TLUT,
//!   Set Other Modes (`tlut_type` is bit 46).
//! Cited: N64 Programming Manual, Chapter 13 — Texture Memory
//!   http://ultra64.ca/files/documentation/online-manuals/man/pro-man/pro13/13-08.html
//!   RGBA32 low/high TMEM split, CI palette in the upper half, 8-bit texel expand.
//!   docs/04-rdp.md
//!
//! [`unimplemented_tmem_op`] stays a trap so the M5-W1 command walk can keep
//! calling it. W1 switches that arm to [`exec_tmem_command`] (or the individual
//! `exec_*` functions). Loads read [`CpuRdpLle::texture_image`], which W1
//! latches from Set Texture Image. CI palette entries expand as RGBA16 unless
//! other-modes bit 46 ([`OTHER_MODES_TLUT_IA16`]) is set.
//!
//! [`sample_texel`] is a point sample. It does not apply the bilinear filter
//! (`sample_type`, other-modes bit 45); the rasterizer fetches the texels it
//! needs and filters them.

#[cfg(test)]
#[path = "tmem_tests.rs"]
mod tests;

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::rdp::{CpuRdpLle, RdpImageState};
use crate::trap::Trap;

/// On-chip texture memory size in bytes.
pub const TMEM_SIZE: usize = 4096;

/// 64-bit words in TMEM (`TMEM_SIZE / 8`).
pub const TMEM_WORDS: usize = TMEM_SIZE / 8;

/// Tile descriptor slots.
pub const TILE_COUNT: usize = 8;

/// Load TLUT.
pub const CMD_LOAD_TLUT: u8 = 0x30;
/// Set Tile Size.
pub const CMD_SET_TILE_SIZE: u8 = 0x32;
/// Load Block.
pub const CMD_LOAD_BLOCK: u8 = 0x33;
/// Load Tile.
pub const CMD_LOAD_TILE: u8 = 0x34;
/// Set Tile.
pub const CMD_SET_TILE: u8 = 0x35;

/// `Set Other Modes` bit 46. Set: TLUT texels are IA16. Clear: RGBA16.
pub const OTHER_MODES_TLUT_IA16: u64 = 1 << 46;

const TMEM_MASK: usize = TMEM_SIZE - 1;
/// Load Block writes nothing when the texel count exceeds this.
const LOAD_BLOCK_MAX_TEXELS: u32 = 2048;
/// First 64-bit word of high TMEM (byte `0x800`).
const TLUT_WORD_BASE: u16 = 0x100;

/// One of the eight RDP tile descriptors.
///
/// `sl`/`tl`/`sh`/`th` are the raw 12-bit tile-size fields. Set Tile Size and
/// Load Tile store u10.2 coordinates. Load Block stores `(uls, ult, lrs, dxt)`
/// in those same fields (n64brew: not a useful rectangle for rendering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TileDesc {
    /// Texel format. 0 RGBA, 1 YUV, 2 CI, 3 IA, 4+ I.
    pub format: u8,
    /// Texel size. 0 = 4-bit, 1 = 8-bit, 2 = 16-bit, 3 = 32-bit.
    pub size: u8,
    /// Row stride in 64-bit TMEM words.
    pub line: u16,
    /// TMEM word (64-bit) address of the tile origin.
    pub tmem_addr: u16,
    /// CI4 palette. Upper nibble of the 8-bit TLUT index. Ignored for CI8.
    pub palette: u8,
    pub clamp_t: bool,
    pub mirror_t: bool,
    pub mask_t: u8,
    pub shift_t: u8,
    pub clamp_s: bool,
    pub mirror_s: bool,
    pub mask_s: u8,
    pub shift_s: u8,
    pub sl: u16,
    pub tl: u16,
    pub sh: u16,
    pub th: u16,
}

/// One texel expanded to 8-bit RGBA, the texture unit's output width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexelRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Clear TMEM to zeros. Tile descriptors are left alone.
pub fn clear_tmem(rdp: &mut CpuRdpLle) {
    rdp.tmem.fill(0);
}

/// Previous seam hook. Still a trap until W1 calls [`exec_tmem_command`].
pub fn unimplemented_tmem_op(cmd_id: u8, word: u64) -> Result<(), Trap> {
    Err(Trap::UnimplementedRdpCommand { cmd_id, word })
}

/// Dispatch one TMEM command word. W1 calls this from the FIFO walk.
pub fn exec_tmem_command(rdp: &mut CpuRdpLle, word: u64, rdram: &Rdram) -> Result<(), Trap> {
    match command_id(word) {
        CMD_LOAD_TLUT => exec_load_tlut(rdp, word, rdram),
        CMD_SET_TILE_SIZE => exec_set_tile_size(rdp, word),
        CMD_LOAD_BLOCK => exec_load_block(rdp, word, rdram),
        CMD_LOAD_TILE => exec_load_tile(rdp, word, rdram),
        CMD_SET_TILE => exec_set_tile(rdp, word),
        cmd_id => Err(Trap::UnimplementedRdpCommand { cmd_id, word }),
    }
}

/// `Set Tile` (`0x35`). Replaces every field except the tile-size rectangle.
pub fn exec_set_tile(rdp: &mut CpuRdpLle, word: u64) -> Result<(), Trap> {
    require_op(word, CMD_SET_TILE)?;
    let index = ((word >> 24) & 7) as usize;
    let prev = rdp.tiles[index];
    rdp.tiles[index] = TileDesc {
        format: ((word >> 53) & 7) as u8,
        size: ((word >> 51) & 3) as u8,
        // Bit 50 is the gap between size and line (n64brew bits 49:41).
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

/// `Set Tile Size` (`0x32`). Stores the raw u10.2 extents.
pub fn exec_set_tile_size(rdp: &mut CpuRdpLle, word: u64) -> Result<(), Trap> {
    require_op(word, CMD_SET_TILE_SIZE)?;
    let rect = decode_rect(word);
    latch_extents(&mut rdp.tiles[usize::from(rect.tile)], &rect);
    Ok(())
}

/// `Load Tile` (`0x34`).
///
/// Coordinates are u10.2 and inclusive (`lrs = count - 1` in the integer part,
/// matching the Load TLUT example on n64brew). A non-zero fraction bit is
/// trapped: the unaligned-start shift is not specified tightly enough to guess.
/// Image size must match the tile size. RGBA32 is split across the two TMEM
/// halves (low = RG, high = BA at `+0x800`). Odd destination rows swap the
/// 32-bit halves of each 64-bit word (`addr ^= 4`).
pub fn exec_load_tile(rdp: &mut CpuRdpLle, word: u64, rdram: &Rdram) -> Result<(), Trap> {
    require_op(word, CMD_LOAD_TILE)?;
    let rect = decode_rect(word);
    let (s0, width) = origin_and_count(rect.uls, rect.lrs).map_err(|_| trap_word(word))?;
    let (t0, height) = origin_and_count(rect.ult, rect.lrt).map_err(|_| trap_word(word))?;
    let idx = usize::from(rect.tile);
    let size = rdp.tiles[idx].size;
    if size != rdp.texture_image.size || !image_aligned(&rdp.texture_image) || size > 3 {
        return Err(trap_word(word));
    }
    latch_extents(&mut rdp.tiles[idx], &rect);
    let tile = rdp.tiles[idx];
    let image = rdp.texture_image;
    let span = TexelRect {
        s0,
        t0,
        width,
        height,
    };
    match tile.size {
        0 => copy_rect_nibbles(&mut rdp.tmem, rdram, &image, &tile, span),
        1 => copy_rect_bytes(&mut rdp.tmem, rdram, &image, &tile, span, 1),
        2 => copy_rect_bytes(&mut rdp.tmem, rdram, &image, &tile, span, 2),
        3 => copy_rect_rgba32(&mut rdp.tmem, rdram, &image, &tile, span),
        _ => return Err(trap_word(word)),
    }
    Ok(())
}

/// `Load Block` (`0x33`).
///
/// `uls`/`ult`/`lrs` are u12.0. Texel count is `lrs - uls + 1` (inclusive).
/// n64brew also says the difference alone "determines the number"; the Load
/// TLUT `lrs = count - 1` example and the 2048-texel ceiling (a load with
/// `lrs = 2047` is the full TMEM of 16-bit texels) fix the count as inclusive.
/// Counts above 2048 latch the tile size and write nothing.
///
/// For each 64-bit word, the line number is `(word_index * dxt) >> 11`
/// (dxt is u1.11; the programming manual increments a counter by dxt per word
/// and uses the integer part). Odd lines swap 32-bit halves.
///
/// A 16-bit texture image may feed a 4-bit or 8-bit tile (n64brew: that is how
/// a load passes the 2048-texel counter). A 32-bit Load Block is trapped: the
/// low/high split plus the dxt swap is the "advance preparation" layout
/// n64brew declines to specify.
pub fn exec_load_block(rdp: &mut CpuRdpLle, word: u64, rdram: &Rdram) -> Result<(), Trap> {
    require_op(word, CMD_LOAD_BLOCK)?;
    let rect = decode_rect(word);
    if rect.lrs < rect.uls {
        return Err(trap_word(word));
    }
    let count = u32::from(rect.lrs - rect.uls) + 1;
    let idx = usize::from(rect.tile);
    if !image_aligned(&rdp.texture_image) {
        return Err(trap_word(word));
    }
    let transfer = block_transfer(rdp.texture_image.size, rdp.tiles[idx].size)
        .ok_or_else(|| trap_word(word))?;
    latch_extents(&mut rdp.tiles[idx], &rect);
    if count > LOAD_BLOCK_MAX_TEXELS {
        return Ok(());
    }
    let tile = rdp.tiles[idx];
    let image = rdp.texture_image;
    let dst = usize::from(tile.tmem_addr) * 8;
    let dxt = u32::from(rect.lrt);
    match transfer {
        Transfer::Nibble => {
            let start = u32::from(rect.ult) * image_width(&image) + u32::from(rect.uls);
            copy_block_nibbles(&mut rdp.tmem, rdram, image.addr, start, dst, count, dxt);
        }
        Transfer::Bytes(bpp) => {
            let src = dram_texel(&image, u32::from(rect.uls), u32::from(rect.ult), bpp);
            let nbytes = (u64::from(count) * u64::from(bpp)) as usize;
            copy_block_bytes(&mut rdp.tmem, rdram, src, dst, nbytes, dxt);
        }
    }
    Ok(())
}

/// `Load TLUT` (`0x30`).
///
/// Each 16-bit image texel is written four times into one 64-bit TMEM word
/// (n64brew: `256 * 2 * 4 = 0x800` bytes for a full palette). The tile must be
/// 4-bit and must not be RGBA or YUV; the texture image must be 16-bit.
/// Coordinates are u10.2, same inclusive span as Load Tile.
pub fn exec_load_tlut(rdp: &mut CpuRdpLle, word: u64, rdram: &Rdram) -> Result<(), Trap> {
    require_op(word, CMD_LOAD_TLUT)?;
    let rect = decode_rect(word);
    let (s0, width) = origin_and_count(rect.uls, rect.lrs).map_err(|_| trap_word(word))?;
    let (t0, height) = origin_and_count(rect.ult, rect.lrt).map_err(|_| trap_word(word))?;
    let idx = usize::from(rect.tile);
    let tile = rdp.tiles[idx];
    if tile.size != 0 || tile.format == 0 || tile.format == 1 {
        return Err(trap_word(word));
    }
    if rdp.texture_image.size != 2 || !image_aligned(&rdp.texture_image) {
        return Err(trap_word(word));
    }
    latch_extents(&mut rdp.tiles[idx], &rect);
    let image = rdp.texture_image;
    let mut word_addr = tile.tmem_addr & 0x1FF;
    for row in 0..height {
        for col in 0..width {
            let src = dram_texel(&image, s0 + col, t0 + row, 2);
            let hi = rdram.read_u8(src);
            let lo = rdram.read_u8(src.wrapping_add(1));
            let dst = usize::from(word_addr) * 8;
            for copy in 0..4 {
                tmem_store(&mut rdp.tmem, dst + copy * 2, hi);
                tmem_store(&mut rdp.tmem, dst + copy * 2 + 1, lo);
            }
            word_addr = word_addr.wrapping_add(1) & 0x1FF;
        }
    }
    Ok(())
}

/// Point-sample tile `tile_index`.
///
/// `s` and `t` are signed s10.5 coordinates (low 16 bits). The tile shift is
/// applied first (n64brew: 1..=10 shift right, 11..=15 shift left by
/// `16 - shift`). The integer texel is `coord >> 5`.
///
/// Mask 0 uses the full integer coordinate. Clamp restricts it to the inclusive
/// tile extent, and the texel read from TMEM is `coord - sl` so the upper-left
/// extent is the tile origin. Mask != 0 keeps that many integer bits (wrap).
/// Mirror flips the index when the bit just above the mask is set. Mask with a
/// non-zero upper-left integer extent is trapped (the two address spaces are
/// not specified together). Sampling outside the extent with neither mask nor
/// clamp is trapped rather than inventing a wrap.
///
/// Odd rows undo the load-time 32-bit swap. CI4 addresses high TMEM as
/// `0x100 | (palette << 4) | index`. CI8 uses `0x100 | index` and ignores the
/// palette. YUV and the unsupported format/size pairs trap.
pub fn sample_texel(rdp: &CpuRdpLle, tile_index: u8, s: i32, t: i32) -> Result<TexelRgba, Trap> {
    let tile = tile_at(rdp, tile_index)?;
    let kind =
        TexelKind::decode(tile.format, tile.size).ok_or_else(|| trap_sample(tile_index, tile))?;
    let s = axis_index(
        s,
        tile.shift_s,
        tile.mask_s,
        tile.mirror_s,
        tile.clamp_s,
        tile.sl,
        tile.sh,
    )
    .map_err(|_| trap_sample(tile_index, tile))?;
    let t = axis_index(
        t,
        tile.shift_t,
        tile.mask_t,
        tile.mirror_t,
        tile.clamp_t,
        tile.tl,
        tile.th,
    )
    .map_err(|_| trap_sample(tile_index, tile))?;
    Ok(expand(rdp, tile, kind, s, t))
}

fn expand(rdp: &CpuRdpLle, tile: &TileDesc, kind: TexelKind, s: usize, t: usize) -> TexelRgba {
    match kind {
        TexelKind::Rgba16 => expand_rgba16(read_u16(rdp, tile, s, t)),
        TexelKind::Rgba32 => {
            let low = swizzled_addr(tile, t, s * 2, false);
            let high = swizzled_addr(tile, t, s * 2, true);
            TexelRgba {
                r: tmem_byte(&rdp.tmem, low),
                g: tmem_byte(&rdp.tmem, low + 1),
                b: tmem_byte(&rdp.tmem, high),
                a: tmem_byte(&rdp.tmem, high + 1),
            }
        }
        TexelKind::Ci4 => {
            let index = read_nibble_tile(&rdp.tmem, tile, s, t);
            let word = TLUT_WORD_BASE | (u16::from(tile.palette) << 4) | u16::from(index);
            expand_tlut(rdp, word)
        }
        TexelKind::Ci8 => {
            let index = tmem_byte(&rdp.tmem, swizzled_addr(tile, t, s, false));
            expand_tlut(rdp, TLUT_WORD_BASE | u16::from(index))
        }
        TexelKind::Ia16 => expand_ia16(read_u16(rdp, tile, s, t)),
        TexelKind::Ia8 => expand_ia8(tmem_byte(&rdp.tmem, swizzled_addr(tile, t, s, false))),
        TexelKind::Ia4 => expand_ia4(read_nibble_tile(&rdp.tmem, tile, s, t)),
        TexelKind::I8 => expand_i8(tmem_byte(&rdp.tmem, swizzled_addr(tile, t, s, false))),
        TexelKind::I4 => expand_i4(read_nibble_tile(&rdp.tmem, tile, s, t)),
    }
}

fn expand_tlut(rdp: &CpuRdpLle, word: u16) -> TexelRgba {
    let addr = usize::from(word) * 8;
    let raw = u16::from_be_bytes([tmem_byte(&rdp.tmem, addr), tmem_byte(&rdp.tmem, addr + 1)]);
    if rdp.other_modes & OTHER_MODES_TLUT_IA16 != 0 {
        expand_ia16(raw)
    } else {
        expand_rgba16(raw)
    }
}

fn read_u16(rdp: &CpuRdpLle, tile: &TileDesc, s: usize, t: usize) -> u16 {
    let addr = swizzled_addr(tile, t, s * 2, false);
    u16::from_be_bytes([tmem_byte(&rdp.tmem, addr), tmem_byte(&rdp.tmem, addr + 1)])
}

#[derive(Clone, Copy)]
enum TexelKind {
    Rgba16,
    Rgba32,
    Ci4,
    Ci8,
    Ia4,
    Ia8,
    Ia16,
    I4,
    I8,
}

impl TexelKind {
    fn decode(format: u8, size: u8) -> Option<Self> {
        match (format, size) {
            (0, 2) => Some(Self::Rgba16),
            (0, 3) => Some(Self::Rgba32),
            (2, 0) => Some(Self::Ci4),
            (2, 1) => Some(Self::Ci8),
            (3, 0) => Some(Self::Ia4),
            (3, 1) => Some(Self::Ia8),
            (3, 2) => Some(Self::Ia16),
            (fmt, 0) if fmt >= 4 => Some(Self::I4),
            (fmt, 1) if fmt >= 4 => Some(Self::I8),
            _ => None,
        }
    }
}

fn replicate5(v: u16) -> u8 {
    let v = (v & 0x1F) as u8;
    (v << 3) | (v >> 2)
}

fn replicate4(v: u8) -> u8 {
    let v = v & 0xF;
    (v << 4) | v
}

fn replicate3(v: u8) -> u8 {
    let v = v & 0x7;
    (v << 5) | (v << 2) | (v >> 1)
}

fn expand_rgba16(c: u16) -> TexelRgba {
    TexelRgba {
        r: replicate5(c >> 11),
        g: replicate5(c >> 6),
        b: replicate5(c >> 1),
        a: if c & 1 == 1 { 0xFF } else { 0 },
    }
}

fn expand_ia16(c: u16) -> TexelRgba {
    let i = (c >> 8) as u8;
    let a = c as u8;
    TexelRgba {
        r: i,
        g: i,
        b: i,
        a,
    }
}

fn expand_ia8(c: u8) -> TexelRgba {
    let i = replicate4(c >> 4);
    let a = replicate4(c);
    TexelRgba {
        r: i,
        g: i,
        b: i,
        a,
    }
}

fn expand_ia4(n: u8) -> TexelRgba {
    let i = replicate3(n >> 1);
    let a = if n & 1 == 1 { 0xFF } else { 0 };
    TexelRgba {
        r: i,
        g: i,
        b: i,
        a,
    }
}

fn expand_i8(c: u8) -> TexelRgba {
    TexelRgba {
        r: c,
        g: c,
        b: c,
        a: c,
    }
}

fn expand_i4(n: u8) -> TexelRgba {
    let i = replicate4(n);
    TexelRgba {
        r: i,
        g: i,
        b: i,
        a: i,
    }
}

/// Shift, then integer clamp / mask / mirror. See [`sample_texel`].
fn axis_index(
    coord: i32,
    shift: u8,
    mask: u8,
    mirror: bool,
    clamp_en: bool,
    lo: u16,
    hi: u16,
) -> Result<usize, ()> {
    let c = apply_shift(coord, shift);
    let sl = i32::from(lo >> 2);
    let sh = i32::from(hi >> 2);
    if sl > sh {
        return Err(());
    }
    let s = c >> 5;
    let m = mask & 0xF;
    if m == 0 {
        if !clamp_en && (s < sl || s > sh) {
            return Err(());
        }
        let idx = if clamp_en { s.clamp(sl, sh) } else { s };
        return Ok((idx - sl) as usize);
    }
    // Power-of-two wrap produces an index whose origin is texel 0. A non-zero
    // upper-left integer would be a second origin; that combination is trapped.
    if sl != 0 {
        return Err(());
    }
    let period = 1i32 << u32::from(m);
    let mut idx = if mirror {
        let doubled = s & ((period << 1) - 1);
        let mut i = doubled & (period - 1);
        if doubled & period != 0 {
            i = (period - 1) - i;
        }
        i
    } else {
        s & (period - 1)
    };
    if clamp_en && idx > sh {
        idx = sh;
    }
    Ok(idx as usize)
}

fn apply_shift(coord: i32, shift: u8) -> i32 {
    let c = i32::from(coord as i16);
    match shift & 0xF {
        0 => c,
        n @ 1..=10 => c >> n,
        n => c.wrapping_shl(u32::from(16 - n)),
    }
}

enum Transfer {
    Nibble,
    Bytes(u32),
}

fn block_transfer(image_size: u8, tile_size: u8) -> Option<Transfer> {
    match (image_size, tile_size) {
        (0, 0) => Some(Transfer::Nibble),
        (1, 1) => Some(Transfer::Bytes(1)),
        (2, 2) | (2, 0) | (2, 1) => Some(Transfer::Bytes(2)),
        _ => None,
    }
}

/// Load Tile rectangle: integer origin plus texel counts.
#[derive(Clone, Copy)]
struct TexelRect {
    s0: u32,
    t0: u32,
    width: u32,
    height: u32,
}

fn copy_rect_bytes(
    tmem: &mut [u8],
    rdram: &Rdram,
    image: &RdpImageState,
    tile: &TileDesc,
    rect: TexelRect,
    bpp: u32,
) {
    for row in 0..rect.height {
        let src = dram_texel(image, rect.s0, rect.t0 + row, bpp);
        let nbytes = (u64::from(rect.width) * u64::from(bpp)) as usize;
        for i in 0..nbytes {
            let addr = swizzled_addr(tile, row as usize, i, false);
            tmem_store(tmem, addr, rdram.read_u8(src.wrapping_add(i as u32)));
        }
    }
}

fn copy_rect_rgba32(
    tmem: &mut [u8],
    rdram: &Rdram,
    image: &RdpImageState,
    tile: &TileDesc,
    rect: TexelRect,
) {
    for row in 0..rect.height {
        for col in 0..rect.width {
            let src = dram_texel(image, rect.s0 + col, rect.t0 + row, 4);
            let low = swizzled_addr(tile, row as usize, (col as usize) * 2, false);
            let high = swizzled_addr(tile, row as usize, (col as usize) * 2, true);
            tmem_store(tmem, low, rdram.read_u8(src));
            tmem_store(tmem, low + 1, rdram.read_u8(src.wrapping_add(1)));
            tmem_store(tmem, high, rdram.read_u8(src.wrapping_add(2)));
            tmem_store(tmem, high + 1, rdram.read_u8(src.wrapping_add(3)));
        }
    }
}

fn copy_rect_nibbles(
    tmem: &mut [u8],
    rdram: &Rdram,
    image: &RdpImageState,
    tile: &TileDesc,
    rect: TexelRect,
) {
    let pitch = image_width(image);
    for row in 0..rect.height {
        for col in 0..rect.width {
            let src_texel = (rect.t0 + row) * pitch + (rect.s0 + col);
            let nibble = read_image_nibble(rdram, image.addr, src_texel);
            let addr = swizzled_addr(tile, row as usize, (col as usize) / 2, false);
            write_nibble(tmem, addr, col & 1 == 0, nibble);
        }
    }
}

fn copy_block_bytes(
    tmem: &mut [u8],
    rdram: &Rdram,
    src: u32,
    dst_base: usize,
    nbytes: usize,
    dxt: u32,
) {
    for i in 0..nbytes {
        let addr = block_addr(dst_base, i, dxt);
        tmem_store(tmem, addr, rdram.read_u8(src.wrapping_add(i as u32)));
    }
}

fn copy_block_nibbles(
    tmem: &mut [u8],
    rdram: &Rdram,
    image_addr: u32,
    src_texel0: u32,
    dst_base: usize,
    count: u32,
    dxt: u32,
) {
    for i in 0..count {
        let nibble = read_image_nibble(rdram, image_addr, src_texel0 + i);
        let byte_off = (i / 2) as usize;
        let addr = block_addr(dst_base, byte_off, dxt);
        write_nibble(tmem, addr, i & 1 == 0, nibble);
    }
}

/// Line of this byte is the integer part of `word_index * dxt` (u1.11).
fn block_addr(dst_base: usize, byte_index: usize, dxt: u32) -> usize {
    let word_index = (byte_index / 8) as u32;
    let line = word_index.wrapping_mul(dxt) >> 11;
    let mut addr = dst_base + byte_index;
    if line & 1 == 1 {
        addr ^= 4;
    }
    addr
}

fn row_base(tile: &TileDesc, row: usize) -> usize {
    usize::from(tile.tmem_addr) * 8 + row * usize::from(tile.line) * 8
}

/// Byte address of a texel byte. Odd rows swap 32-bit halves (`^= 4`).
/// `high_half` adds `0x800` for the BA half of an RGBA32 texel.
fn swizzled_addr(tile: &TileDesc, row: usize, byte_off: usize, high_half: bool) -> usize {
    let mut addr = row_base(tile, row) + byte_off;
    if high_half {
        addr += 0x800;
    }
    if row & 1 == 1 {
        addr ^= 4;
    }
    addr
}

fn read_nibble_tile(tmem: &[u8], tile: &TileDesc, s: usize, t: usize) -> u8 {
    let addr = swizzled_addr(tile, t, s / 2, false);
    let byte = tmem_byte(tmem, addr);
    if s & 1 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    }
}

fn read_image_nibble(rdram: &Rdram, base: u32, texel: u32) -> u8 {
    let byte = rdram.read_u8(base.wrapping_add(texel / 2));
    if texel & 1 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    }
}

fn write_nibble(tmem: &mut [u8], addr: usize, high: bool, nibble: u8) {
    let i = addr & TMEM_MASK;
    let shift = if high { 4 } else { 0 };
    let mask = 0xFu8 << shift;
    tmem[i] = (tmem[i] & !mask) | ((nibble & 0xF) << shift);
}

fn tmem_store(tmem: &mut [u8], addr: usize, byte: u8) {
    tmem[addr & TMEM_MASK] = byte;
}

fn tmem_byte(tmem: &[u8], addr: usize) -> u8 {
    tmem[addr & TMEM_MASK]
}

fn image_width(image: &RdpImageState) -> u32 {
    u32::from(image.width) + 1
}

fn image_aligned(image: &RdpImageState) -> bool {
    image.addr & 7 == 0
}

fn dram_texel(image: &RdpImageState, s: u32, t: u32, bpp: u32) -> u32 {
    let off = (u64::from(t) * u64::from(image_width(image)) + u64::from(s)) * u64::from(bpp);
    image.addr.wrapping_add(off as u32)
}

struct RectCmd {
    uls: u16,
    ult: u16,
    tile: u8,
    lrs: u16,
    lrt: u16,
}

fn decode_rect(word: u64) -> RectCmd {
    RectCmd {
        uls: ((word >> 44) & 0xFFF) as u16,
        ult: ((word >> 32) & 0xFFF) as u16,
        tile: ((word >> 24) & 7) as u8,
        lrs: ((word >> 12) & 0xFFF) as u16,
        lrt: (word & 0xFFF) as u16,
    }
}

fn latch_extents(tile: &mut TileDesc, rect: &RectCmd) {
    tile.sl = rect.uls;
    tile.tl = rect.ult;
    tile.sh = rect.lrs;
    tile.th = rect.lrt;
}

/// Integer origin and inclusive texel count of a u10.2 pair.
/// Fraction bits and an inverted span are rejected.
fn origin_and_count(lo: u16, hi: u16) -> Result<(u32, u32), ()> {
    if (lo | hi) & 3 != 0 || hi < lo {
        return Err(());
    }
    Ok((u32::from(lo >> 2), u32::from((hi - lo) >> 2) + 1))
}

fn command_id(word: u64) -> u8 {
    ((word >> 56) & 0x3F) as u8
}

fn trap_word(word: u64) -> Trap {
    Trap::UnimplementedRdpCommand {
        cmd_id: command_id(word),
        word,
    }
}

fn trap_sample(tile_index: u8, tile: &TileDesc) -> Trap {
    let word = (u64::from(CMD_SET_TILE) << 56)
        | (u64::from(tile.format) << 53)
        | (u64::from(tile.size) << 51)
        | (u64::from(tile_index) << 24);
    Trap::UnimplementedRdpCommand {
        cmd_id: CMD_SET_TILE,
        word,
    }
}

fn require_op(word: u64, expect: u8) -> Result<(), Trap> {
    let id = command_id(word);
    if id == expect {
        Ok(())
    } else {
        Err(Trap::UnimplementedRdpCommand { cmd_id: id, word })
    }
}

fn tile_at(rdp: &CpuRdpLle, index: u8) -> Result<&TileDesc, Trap> {
    rdp.tiles
        .get(usize::from(index))
        .ok_or(Trap::UnimplementedRdpCommand {
            cmd_id: CMD_SET_TILE,
            word: (u64::from(CMD_SET_TILE) << 56) | (u64::from(index) << 24),
        })
}
