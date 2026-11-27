// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024-2026 Graycart Contributors

//! Host validation and parameter packing for EdgeWalker Color Combiner & Blender.
//!
//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands

use crate::trap::Trap;

use super::gpu_types::CombineStatePack;

pub const COMBINE_BLEND_WGSL: &str = include_str!("shaders/combine_blend.wgsl");

/// Bundle host registers into a 64-byte WGSL [`CombineStatePack`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn combine_state_pack(
    combine_mode: u64,
    other_modes: u64,
    prim_color: u32,
    env_color: u32,
    fog_color: u32,
    blend_color: u32,
    prim_lod_frac: u8,
    convert_k4: u16,
    convert_k5: u16,
    key_center: [u8; 3],
    key_scale: [u8; 3],
) -> CombineStatePack {
    let key_center_rgb = (u32::from(key_center[0]) << 16)
        | (u32::from(key_center[1]) << 8)
        | u32::from(key_center[2]);
    let key_scale_rgb =
        (u32::from(key_scale[0]) << 16) | (u32::from(key_scale[1]) << 8) | u32::from(key_scale[2]);
    CombineStatePack {
        combine_lo: combine_mode as u32,
        combine_hi: (combine_mode >> 32) as u32,
        other_modes_lo: other_modes as u32,
        other_modes_hi: (other_modes >> 32) as u32,
        prim_color,
        env_color,
        fog_color,
        blend_color,
        prim_lod_frac: u32::from(prim_lod_frac),
        convert_k4: u32::from(convert_k4),
        convert_k5: u32::from(convert_k5),
        key_center_rgb,
        key_scale_rgb,
        pad0: 0,
        pad1: 0,
        pad2: 0,
    }
}

/// Validate color combiner and blender modes against N64 RDP rules before GPU dispatch.
pub fn validate_combine_and_blend(
    combine_mode: u64,
    other_modes: u64,
    prim_color: u32,
    fog_color: u32,
) -> Result<(), Trap> {
    if combine_mode == 0 {
        return Ok(());
    }

    let cycle_type = (other_modes >> 52) & 3;
    if cycle_type >= 2 {
        return Err(trap_combine(combine_mode));
    }

    // Chroma key (`key_en`, bit 40).
    if (other_modes & (1 << 40)) != 0 {
        return Err(trap_combine(combine_mode));
    }

    let tex_lod_en = (other_modes & (1 << 48)) != 0;
    let cycles: &[u8] = if cycle_type == 1 { &[0, 1] } else { &[1] };
    for &which in cycles {
        let (rgb_a, rgb_c, alpha_c) = if which == 0 {
            (
                ((combine_mode >> 52) & 0xF) as u8,
                ((combine_mode >> 47) & 0x1F) as u8,
                ((combine_mode >> 41) & 0x7) as u8,
            )
        } else {
            (
                ((combine_mode >> 37) & 0xF) as u8,
                ((combine_mode >> 32) & 0x1F) as u8,
                ((combine_mode >> 18) & 0x7) as u8,
            )
        };
        // NOISE (rgb_a == 7) traps.
        if rgb_a == 7 {
            return Err(trap_combine(combine_mode));
        }
        // LOD_FRACTION requires 2-cycle mode AND tex_lod_en.
        if (rgb_c == 13 || alpha_c == 0) && (cycle_type == 0 || !tex_lod_en) {
            return Err(trap_combine(combine_mode));
        }
    }

    // Blender only runs when antialias(3), color_on_cvg(7), cvg_x_alpha(12),
    // and alpha_cvg_select(13) are all clear in `other_modes[31:0]`.
    let blend_bypassed =
        (other_modes & ((1 << 3) | (1 << 7) | (1 << 12) | (1 << 13) | (1 << 40))) != 0;
    if !blend_bypassed {
        let mux_sets: &[u8] = if cycle_type == 1 { &[0, 1] } else { &[0] };
        let fog_a = fog_color & 0xFF;
        let _ = prim_color;
        for &set in mux_sets {
            let (a_sel, b_sel) = if set == 0 {
                (
                    ((other_modes >> 26) & 3) as u8,
                    ((other_modes >> 18) & 3) as u8,
                )
            } else {
                (
                    ((other_modes >> 24) & 3) as u8,
                    ((other_modes >> 16) & 3) as u8,
                )
            };
            // Memory coverage (B == 1) traps.
            if b_sel == 1 {
                return Err(trap_blend(other_modes));
            }
            // Known zero denominator (B == 3 and factor_a == 0) traps.
            if b_sel == 3 && (a_sel == 3 || (a_sel == 1 && fog_a == 0)) {
                return Err(trap_blend(other_modes));
            }
        }
    }

    Ok(())
}

fn trap_combine(combine_mode: u64) -> Trap {
    Trap::UnimplementedRdpCommand {
        cmd_id: 0x3C,
        word: (0x3Cu64 << 56) | (combine_mode & 0x00FF_FFFF_FFFF_FFFF),
    }
}

fn trap_blend(other_modes: u64) -> Trap {
    Trap::UnimplementedRdpCommand {
        cmd_id: 0x2F,
        word: (0x2Fu64 << 56) | (other_modes & 0x00FF_FFFF_FFFF_FFFF),
    }
}

#[cfg(test)]
#[path = "combine_gpu_tests.rs"]
mod tests;
