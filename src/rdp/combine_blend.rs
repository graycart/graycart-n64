//! Color combiner and blender.
//!
//! The command walker latches [`CpuRdpLle::combine_mode`], [`CpuRdpLle::other_modes`],
//! and the primitive / environment / blend / fog / fill registers. This module
//! evaluates that state. `SyncFull` (0x29) still raises the DP interrupt.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   (`Set Combine Mode` 0x3C, `Set Other Modes` 0x2F, `Sync Full` 0x29,
//!   `Set Convert` 0x2C, `Set Key R` 0x2B, `Set Key GB` 0x2A)
//!   docs/04-rdp.md
//!
//! Combiner equation, per RGB channel and for alpha: `(A - B) * C + D`.
//! `C` is an 8.8 fraction (`>> 8`). The "1" source is 256, not 255.
//! 1-cycle mode uses only the cycle-1 muxes. 2-cycle mode runs cycle 0 with
//! no clamp, then cycle 1. The value written out keeps the low 9 bits:
//! 0..=255 pass through, 256..=383 saturate to 255, 384..=511 become 0.
//!
//! Blender, for the modes Fill and 1-cycle bring-up need:
//! FILL and COPY bypass the unit. 1-cycle uses the first blender mux set;
//! 2-cycle runs both. The mix is `(P * A + M * B) / (A + B)` with 1.0 = 256.
//! Coverage-as-alpha, `color_on_cvg`, and the anti-aliased edge divider trap.

#[cfg(test)]
#[path = "combine_blend_tests.rs"]
mod tests;

use crate::bus::Rdram;
use crate::rdp::{CpuRdpLle, RdpAction};
use crate::trap::Trap;

const ONE: i32 = 256;
const PAYLOAD: u64 = 0x00FF_FFFF_FFFF_FFFF;

/// 8-bit color. Alpha is the combiner alpha; the blender subset does not replace it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Unclamped combiner channel. Cycle 0 of 2-cycle mode forwards this raw value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbaI {
    pub r: i32,
    pub g: i32,
    pub b: i32,
    pub a: i32,
}

/// Pipeline mode in `other_modes` bits 53:52.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleType {
    /// Bits `00`. Combiner uses cycle-1 muxes. Blender uses the first mux set.
    One,
    /// Bits `01`. Combiner and blender each run both cycles.
    Two,
    /// Bits `10`. Combiner and blender are bypassed; the copy unit writes texels.
    Copy,
    /// Bits `11`. Combiner and blender are bypassed; the fill color is stored verbatim.
    Fill,
}

/// Per-pixel sources the rasterizer feeds the combiner.
///
/// Texel and shade are the values for this pixel. n64brew's 1-cycle `TEX1` and
/// 2-cycle second-cycle texel swaps are pipeline bugs the rasterizer applies
/// before this call. `combined` is the previous pixel's combiner output
/// (ill-defined on cycle 0 / in 1-cycle mode; pass 0 when there is no neighbor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CombineInputs {
    pub texel0: Rgba8,
    pub texel1: Rgba8,
    pub shade: Rgba8,
    pub combined: RgbaI,
    /// 9-bit noise sample. Required only when a mux selects `NOISE`.
    /// Hardware shape: top 3 bits random, low 6 bits fixed at `0b10_0000`.
    pub noise: Option<i32>,
    /// Texture LOD fraction, 0..=255. Valid in 2-cycle mode with `tex_lod_en`.
    pub lod_fraction: u8,
}

/// Per-pixel sources for the blender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlendInputs {
    /// Final color-combiner output (clamped).
    pub combined: Rgba8,
    pub shade_alpha: u8,
    /// 2-cycle second cycle reads the next pixel's shade alpha (n64brew).
    /// Required when that cycle's A mux is shade alpha.
    pub shade_alpha_next: Option<u8>,
    pub memory: Rgba8,
    /// 2-cycle first cycle reads the previous pixel's memory color.
    /// Required when that cycle's P or M mux is memory.
    pub memory_prev: Option<Rgba8>,
}

/// What the blender stage produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendOutput {
    /// 1-cycle or 2-cycle mix. Alpha is the combiner alpha.
    Color(Rgba8),
    /// FILL. The 32-bit fill register is written as stored; the blender does not run.
    Fill(u32),
    /// COPY. The blender does not run.
    Copy,
}

/// `SyncFull` (command `0x29`): the pipe goes idle and the DP interrupt is requested.
///
/// Does not retire `DPC_CURRENT`. Command-DMA completion is
/// [`crate::rdp::CpuRdpLle::complete_cmd_dma`], which `submit_commands` calls
/// after a list is accepted.
pub fn sync_full(rdp: &mut CpuRdpLle, _rdram: &mut Rdram) -> Result<RdpAction, Trap> {
    rdp.dpc_status &= !(CpuRdpLle::STATUS_PIPE_BUSY
        | CpuRdpLle::STATUS_CMD_BUSY
        | CpuRdpLle::STATUS_TMEM_BUSY
        | CpuRdpLle::STATUS_START_GCLK
        | CpuRdpLle::STATUS_DMA_BUSY);
    Ok(RdpAction::RaiseDpIntr)
}

/// Commands that still reach this entry trap.
///
/// `Set Combine Mode` is latched by the command walker into
/// [`CpuRdpLle::combine_mode`]. Math is [`eval_combine`], not this function.
pub fn unimplemented_combine(cmd_id: u8, word: u64) -> Result<(), Trap> {
    Err(Trap::UnimplementedRdpCommand { cmd_id, word })
}

/// Latch K4/K5 from `Set Convert` (0x2C). K0–K3 belong to the texture filter.
pub fn latch_convert(rdp: &mut CpuRdpLle, word: u64) -> Result<(), Trap> {
    expect_cmd(word, 0x2C)?;
    rdp.convert_k4 = ((word >> 9) & 0x1FF) as u16;
    rdp.convert_k5 = (word & 0x1FF) as u16;
    Ok(())
}

/// Latch the red chroma-key center and scale from `Set Key R` (0x2B).
pub fn latch_key_r(rdp: &mut CpuRdpLle, word: u64) -> Result<(), Trap> {
    expect_cmd(word, 0x2B)?;
    rdp.key_center[0] = ((word >> 8) & 0xFF) as u8;
    rdp.key_scale[0] = (word & 0xFF) as u8;
    Ok(())
}

/// Latch the green and blue chroma-key center and scale from `Set Key GB` (0x2A).
pub fn latch_key_gb(rdp: &mut CpuRdpLle, word: u64) -> Result<(), Trap> {
    expect_cmd(word, 0x2A)?;
    rdp.key_center[1] = ((word >> 24) & 0xFF) as u8;
    rdp.key_scale[1] = ((word >> 16) & 0xFF) as u8;
    rdp.key_center[2] = ((word >> 8) & 0xFF) as u8;
    rdp.key_scale[2] = (word & 0xFF) as u8;
    Ok(())
}

/// Cycle type from the latched other-modes word (bits 53:52).
#[must_use]
pub fn cycle_type(rdp: &CpuRdpLle) -> CycleType {
    match (rdp.other_modes >> 52) & 0b11 {
        0 => CycleType::One,
        1 => CycleType::Two,
        2 => CycleType::Copy,
        _ => CycleType::Fill,
    }
}

/// Run the color combiner for the latched cycle type and muxes.
///
/// FILL and COPY trap: those modes bypass the combiner, and a black pixel
/// would hide that. Chroma key (`key_en`) traps; the key stage is not in
/// this bring-up. `LOD_FRACTION` traps in 1-cycle mode and whenever
/// `tex_lod_en` is clear.
pub fn eval_combine(rdp: &CpuRdpLle, inputs: &CombineInputs) -> Result<Rgba8, Trap> {
    let passes: &[u8] = match cycle_type(rdp) {
        CycleType::One => &[1],
        CycleType::Two => &[0, 1],
        CycleType::Copy | CycleType::Fill => return Err(trap_combine(rdp)),
    };
    if key_enabled(rdp) {
        return Err(trap_combine(rdp));
    }
    let mut combined = inputs.combined;
    for &which in passes {
        combined = run_cycle(rdp, inputs, combined, decode_cycle(rdp.combine_mode, which))?;
    }
    Ok(Rgba8 {
        r: clamp9(combined.r),
        g: clamp9(combined.g),
        b: clamp9(combined.b),
        a: clamp9(combined.a),
    })
}

/// Run the blender, or report a FILL/COPY bypass.
///
/// 1-cycle uses blender mux set 0. 2-cycle runs set 0, then set 1, where
/// P/M select 0 is the first cycle's color. Memory coverage (`B = 1`),
/// `alpha_cvg_select`, `cvg_x_alpha`, `color_on_cvg`, `antialias_en`, and
/// `key_en` trap. A zero denominator traps.
pub fn eval_blend(rdp: &CpuRdpLle, inputs: &BlendInputs) -> Result<BlendOutput, Trap> {
    match cycle_type(rdp) {
        CycleType::Fill => return Ok(BlendOutput::Fill(rdp.fill_color)),
        CycleType::Copy => return Ok(BlendOutput::Copy),
        CycleType::One | CycleType::Two => {}
    }
    if key_enabled(rdp) || exotic_blend(rdp.other_modes) {
        return Err(trap_blend(rdp));
    }
    let mux0 = blend_mux(rdp.other_modes, 0);
    if mux0.b == 1 {
        return Err(trap_blend(rdp));
    }
    // The "memory color is the previous pixel" bug is the first cycle of
    // 2-cycle mode only. 1-cycle reads the current pixel.
    let mem0 = if cycle_type(rdp) == CycleType::Two && (mux0.p == 1 || mux0.m == 1) {
        inputs.memory_prev.ok_or_else(|| trap_blend(rdp))?
    } else {
        inputs.memory
    };
    let mid = mix_mux(rdp, inputs, &mux0, mem0, inputs.shade_alpha, None)?;
    if cycle_type(rdp) == CycleType::One {
        return Ok(BlendOutput::Color(mid));
    }
    let mux1 = blend_mux(rdp.other_modes, 1);
    if mux1.b == 1 {
        return Err(trap_blend(rdp));
    }
    let shade1 = if mux1.a == 2 {
        inputs.shade_alpha_next.ok_or_else(|| trap_blend(rdp))?
    } else {
        inputs.shade_alpha
    };
    let out = mix_mux(rdp, inputs, &mux1, inputs.memory, shade1, Some(mid))?;
    Ok(BlendOutput::Color(out))
}

fn expect_cmd(word: u64, id: u8) -> Result<(), Trap> {
    let got = ((word >> 56) & 0x3F) as u8;
    if got == id {
        Ok(())
    } else {
        Err(Trap::UnimplementedRdpCommand { cmd_id: got, word })
    }
}

fn trap_combine(rdp: &CpuRdpLle) -> Trap {
    Trap::UnimplementedRdpCommand {
        cmd_id: 0x3C,
        word: (0x3Cu64 << 56) | (rdp.combine_mode & PAYLOAD),
    }
}

fn trap_blend(rdp: &CpuRdpLle) -> Trap {
    Trap::UnimplementedRdpCommand {
        cmd_id: 0x2F,
        word: (0x2Fu64 << 56) | (rdp.other_modes & PAYLOAD),
    }
}

fn key_enabled(rdp: &CpuRdpLle) -> bool {
    (rdp.other_modes & (1 << 40)) != 0
}

fn tex_lod_enabled(rdp: &CpuRdpLle) -> bool {
    (rdp.other_modes & (1 << 48)) != 0
}

fn exotic_blend(modes: u64) -> bool {
    let antialias = (modes & (1 << 3)) != 0;
    let color_on_cvg = (modes & (1 << 7)) != 0;
    let cvg_x_alpha = (modes & (1 << 12)) != 0;
    let alpha_cvg_select = (modes & (1 << 13)) != 0;
    antialias || color_on_cvg || cvg_x_alpha || alpha_cvg_select
}

/// Low 9 bits of the combiner sum. See the module docs for the windows.
fn clamp9(v: i32) -> u8 {
    let n = (v as u32) & 0x1FF;
    if n & 0x100 == 0 {
        n as u8
    } else if n & 0x80 == 0 {
        255
    } else {
        0
    }
}

fn cc_mul(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let diff = a.wrapping_sub(b);
    let prod = diff.wrapping_mul(c);
    (prod >> 8).wrapping_add(d)
}

struct MuxCycle {
    rgb_a: u8,
    rgb_b: u8,
    rgb_c: u8,
    rgb_d: u8,
    alpha_a: u8,
    alpha_b: u8,
    alpha_c: u8,
    alpha_d: u8,
}

fn decode_cycle(mode: u64, which: u8) -> MuxCycle {
    if which == 0 {
        MuxCycle {
            rgb_a: ((mode >> 52) & 0xF) as u8,
            rgb_c: ((mode >> 47) & 0x1F) as u8,
            alpha_a: ((mode >> 44) & 0x7) as u8,
            alpha_c: ((mode >> 41) & 0x7) as u8,
            rgb_b: ((mode >> 28) & 0xF) as u8,
            rgb_d: ((mode >> 15) & 0x7) as u8,
            alpha_b: ((mode >> 12) & 0x7) as u8,
            alpha_d: ((mode >> 9) & 0x7) as u8,
        }
    } else {
        MuxCycle {
            rgb_a: ((mode >> 37) & 0xF) as u8,
            rgb_c: ((mode >> 32) & 0x1F) as u8,
            alpha_a: ((mode >> 21) & 0x7) as u8,
            alpha_c: ((mode >> 18) & 0x7) as u8,
            rgb_b: ((mode >> 24) & 0xF) as u8,
            rgb_d: ((mode >> 6) & 0x7) as u8,
            alpha_b: ((mode >> 3) & 0x7) as u8,
            alpha_d: (mode & 0x7) as u8,
        }
    }
}

fn run_cycle(
    rdp: &CpuRdpLle,
    inputs: &CombineInputs,
    combined: RgbaI,
    mux: MuxCycle,
) -> Result<RgbaI, Trap> {
    let rgb = |slot: RgbSlot, sel: u8, ch: u8| -> Result<i32, Trap> {
        rgb_source(rdp, inputs, combined, slot, sel, ch)
    };
    let alpha = |slot: AlphaSlot, sel: u8| -> Result<i32, Trap> {
        alpha_source(rdp, inputs, combined, slot, sel)
    };
    Ok(RgbaI {
        r: cc_mul(
            rgb(RgbSlot::A, mux.rgb_a, 0)?,
            rgb(RgbSlot::B, mux.rgb_b, 0)?,
            rgb(RgbSlot::C, mux.rgb_c, 0)?,
            rgb(RgbSlot::D, mux.rgb_d, 0)?,
        ),
        g: cc_mul(
            rgb(RgbSlot::A, mux.rgb_a, 1)?,
            rgb(RgbSlot::B, mux.rgb_b, 1)?,
            rgb(RgbSlot::C, mux.rgb_c, 1)?,
            rgb(RgbSlot::D, mux.rgb_d, 1)?,
        ),
        b: cc_mul(
            rgb(RgbSlot::A, mux.rgb_a, 2)?,
            rgb(RgbSlot::B, mux.rgb_b, 2)?,
            rgb(RgbSlot::C, mux.rgb_c, 2)?,
            rgb(RgbSlot::D, mux.rgb_d, 2)?,
        ),
        a: cc_mul(
            alpha(AlphaSlot::A, mux.alpha_a)?,
            alpha(AlphaSlot::B, mux.alpha_b)?,
            alpha(AlphaSlot::C, mux.alpha_c)?,
            alpha(AlphaSlot::D, mux.alpha_d)?,
        ),
    })
}

#[derive(Clone, Copy)]
enum RgbSlot {
    A,
    B,
    C,
    D,
}

#[derive(Clone, Copy)]
enum AlphaSlot {
    A,
    B,
    C,
    D,
}

fn chan8(c: Rgba8, ch: u8) -> i32 {
    i32::from(match ch {
        0 => c.r,
        1 => c.g,
        _ => c.b,
    })
}

fn chan_i(c: RgbaI, ch: u8) -> i32 {
    match ch {
        0 => c.r,
        1 => c.g,
        _ => c.b,
    }
}

fn rgb_source(
    rdp: &CpuRdpLle,
    inputs: &CombineInputs,
    combined: RgbaI,
    slot: RgbSlot,
    sel: u8,
    ch: u8,
) -> Result<i32, Trap> {
    let prim = rgba_u32(rdp.prim_color);
    let env = rgba_u32(rdp.env_color);
    let color = match slot {
        RgbSlot::A => match sel {
            0 => chan_i(combined, ch),
            1 => chan8(inputs.texel0, ch),
            2 => chan8(inputs.texel1, ch),
            3 => chan8(prim, ch),
            4 => chan8(inputs.shade, ch),
            5 => chan8(env, ch),
            6 => ONE,
            7 => inputs.noise.ok_or_else(|| trap_combine(rdp))?,
            _ => 0,
        },
        RgbSlot::B => match sel {
            0 => chan_i(combined, ch),
            1 => chan8(inputs.texel0, ch),
            2 => chan8(inputs.texel1, ch),
            3 => chan8(prim, ch),
            4 => chan8(inputs.shade, ch),
            5 => chan8(env, ch),
            6 => i32::from(rdp.key_center[ch as usize]),
            7 => i32::from(rdp.convert_k4),
            _ => 0,
        },
        RgbSlot::C => match sel {
            0 => chan_i(combined, ch),
            1 => chan8(inputs.texel0, ch),
            2 => chan8(inputs.texel1, ch),
            3 => chan8(prim, ch),
            4 => chan8(inputs.shade, ch),
            5 => chan8(env, ch),
            6 => i32::from(rdp.key_scale[ch as usize]),
            7 => combined.a,
            8 => i32::from(inputs.texel0.a),
            9 => i32::from(inputs.texel1.a),
            10 => i32::from(prim.a),
            11 => i32::from(inputs.shade.a),
            12 => i32::from(env.a),
            13 => lod_fraction(rdp, inputs)?,
            14 => i32::from(rdp.prim_lod_frac),
            15 => i32::from(rdp.convert_k5),
            _ => 0,
        },
        RgbSlot::D => match sel {
            0 => chan_i(combined, ch),
            1 => chan8(inputs.texel0, ch),
            2 => chan8(inputs.texel1, ch),
            3 => chan8(prim, ch),
            4 => chan8(inputs.shade, ch),
            5 => chan8(env, ch),
            6 => ONE,
            _ => 0,
        },
    };
    Ok(color)
}

fn alpha_source(
    rdp: &CpuRdpLle,
    inputs: &CombineInputs,
    combined: RgbaI,
    slot: AlphaSlot,
    sel: u8,
) -> Result<i32, Trap> {
    let prim_a = i32::from((rdp.prim_color & 0xFF) as u8);
    let env_a = i32::from((rdp.env_color & 0xFF) as u8);
    let tex = |which: u8| -> i32 {
        i32::from(if which == 0 {
            inputs.texel0.a
        } else {
            inputs.texel1.a
        })
    };
    let color = match slot {
        AlphaSlot::A | AlphaSlot::B | AlphaSlot::D => match sel {
            0 => combined.a,
            1 => tex(0),
            2 => tex(1),
            3 => prim_a,
            4 => i32::from(inputs.shade.a),
            5 => env_a,
            6 => ONE,
            _ => 0,
        },
        AlphaSlot::C => match sel {
            0 => lod_fraction(rdp, inputs)?,
            1 => tex(0),
            2 => tex(1),
            3 => prim_a,
            4 => i32::from(inputs.shade.a),
            5 => env_a,
            6 => i32::from(rdp.prim_lod_frac),
            _ => 0,
        },
    };
    Ok(color)
}

fn lod_fraction(rdp: &CpuRdpLle, inputs: &CombineInputs) -> Result<i32, Trap> {
    if cycle_type(rdp) == CycleType::One || !tex_lod_enabled(rdp) {
        Err(trap_combine(rdp))
    } else {
        Ok(i32::from(inputs.lod_fraction))
    }
}

fn rgba_u32(c: u32) -> Rgba8 {
    Rgba8 {
        r: (c >> 24) as u8,
        g: (c >> 16) as u8,
        b: (c >> 8) as u8,
        a: c as u8,
    }
}

struct BlendMux {
    p: u8,
    a: u8,
    m: u8,
    b: u8,
}

fn blend_mux(modes: u64, cycle: u8) -> BlendMux {
    if cycle == 0 {
        BlendMux {
            p: ((modes >> 30) & 3) as u8,
            a: ((modes >> 26) & 3) as u8,
            m: ((modes >> 22) & 3) as u8,
            b: ((modes >> 18) & 3) as u8,
        }
    } else {
        BlendMux {
            p: ((modes >> 28) & 3) as u8,
            a: ((modes >> 24) & 3) as u8,
            m: ((modes >> 20) & 3) as u8,
            b: ((modes >> 16) & 3) as u8,
        }
    }
}

fn mix_mux(
    rdp: &CpuRdpLle,
    inputs: &BlendInputs,
    mux: &BlendMux,
    memory: Rgba8,
    shade_a: u8,
    cycle0: Option<Rgba8>,
) -> Result<Rgba8, Trap> {
    let combined = inputs.combined;
    let fog = rgba_u32(rdp.fog_color);
    let blend = rgba_u32(rdp.blend_color);
    let pick = |sel: u8| -> Rgba8 {
        match sel & 3 {
            0 => cycle0.unwrap_or(combined),
            1 => memory,
            2 => blend,
            _ => fog,
        }
    };
    let factor_a = match mux.a & 3 {
        0 => u32::from(combined.a),
        1 => u32::from(fog.a),
        2 => u32::from(shade_a),
        _ => 0,
    };
    let factor_b = match mux.b & 3 {
        0 => 256u32.saturating_sub(factor_a),
        1 => return Err(trap_blend(rdp)),
        2 => 256,
        _ => 0,
    };
    let denom = factor_a + factor_b;
    if denom == 0 {
        return Err(trap_blend(rdp));
    }
    let p = pick(mux.p);
    let m = pick(mux.m);
    let chan = |pv: u8, mv: u8| -> u8 {
        let num = u32::from(pv) * factor_a + u32::from(mv) * factor_b;
        (num / denom).min(255) as u8
    };
    Ok(Rgba8 {
        r: chan(p.r, m.r),
        g: chan(p.g, m.g),
        b: chan(p.b, m.b),
        a: combined.a,
    })
}
