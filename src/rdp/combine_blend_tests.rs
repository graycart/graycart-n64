//! Cited: N64brew — Reality Display Processor/Commands
//!   https://n64brew.dev/wiki/Reality_Display_Processor/Commands
//!   (`Set Combine Mode` 0x3C, `Set Other Modes` 0x2F)

use super::*;
use crate::bus::Rdram;
use crate::rdp::CpuRdpLle;

fn bits(cycle: [u8; 8], which: u8) -> u64 {
    if which == 0 {
        u64::from(cycle[0] & 0xF) << 52
            | u64::from(cycle[2] & 0x1F) << 47
            | u64::from(cycle[4] & 7) << 44
            | u64::from(cycle[6] & 7) << 41
            | u64::from(cycle[1] & 0xF) << 28
            | u64::from(cycle[3] & 7) << 15
            | u64::from(cycle[5] & 7) << 12
            | u64::from(cycle[7] & 7) << 9
    } else {
        u64::from(cycle[0] & 0xF) << 37
            | u64::from(cycle[2] & 0x1F) << 32
            | u64::from(cycle[4] & 7) << 21
            | u64::from(cycle[6] & 7) << 18
            | u64::from(cycle[1] & 0xF) << 24
            | u64::from(cycle[3] & 7) << 6
            | u64::from(cycle[5] & 7) << 3
            | u64::from(cycle[7] & 7)
    }
}

fn load_combine(rdp: &mut CpuRdpLle, c0: [u8; 8], c1: [u8; 8]) {
    rdp.combine_mode = bits(c0, 0) | bits(c1, 1);
}

fn load_modes(rdp: &mut CpuRdpLle, cycle: u8, b0: [u8; 4], b1: [u8; 4]) {
    let mut w = u64::from(cycle & 3) << 52;
    w |= u64::from(b0[0] & 3) << 30;
    w |= u64::from(b0[1] & 3) << 26;
    w |= u64::from(b0[2] & 3) << 22;
    w |= u64::from(b0[3] & 3) << 18;
    w |= u64::from(b1[0] & 3) << 28;
    w |= u64::from(b1[1] & 3) << 24;
    w |= u64::from(b1[2] & 3) << 20;
    w |= u64::from(b1[3] & 3) << 16;
    rdp.other_modes = w;
}

#[test]
fn sync_full_clears_busy_and_raises_dp() {
    let mut rdp = CpuRdpLle::new();
    rdp.dpc_status |= CpuRdpLle::STATUS_PIPE_BUSY | CpuRdpLle::STATUS_CMD_BUSY;
    let mut rdram = Rdram::new(true);
    let action = sync_full(&mut rdp, &mut rdram).unwrap();
    assert_eq!(action, RdpAction::RaiseDpIntr);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_PIPE_BUSY, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_CMD_BUSY, 0);
    assert_eq!(rdp.dpc_status & CpuRdpLle::STATUS_START_GCLK, 0);
}

#[test]
fn unimplemented_combine_still_traps() {
    let err = unimplemented_combine(0x3C, 0x3C00_0000_0000_0000).unwrap_err();
    assert!(matches!(
        err,
        crate::trap::Trap::UnimplementedRdpCommand { cmd_id: 0x3C, .. }
    ));
}

#[test]
fn submit_latches_combine_and_one_cycle_reads_primitive() {
    let mut rdp = CpuRdpLle::new();
    let mut rdram = Rdram::new(false);
    let prim = (0x3Au64 << 56) | 0xFF00_0080;
    // 1-cycle uses cycle 1. D = primitive, every other source is 0.
    let combine = (0x3Cu64 << 56) | bits([8, 8, 16, 3, 7, 7, 7, 3], 1);
    rdp.submit_commands(&[prim, combine], &mut rdram).unwrap();
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color, Rgba8::new(0xFF, 0, 0, 0x80));
    assert_eq!(cycle_type(&rdp), CycleType::One);
}

#[test]
fn one_cycle_ignores_cycle0_muxes() {
    let mut rdp = CpuRdpLle::new();
    rdp.prim_color = 0xFF00_0000;
    rdp.env_color = 0x00FF_0000;
    // Cycle 0 would emit primitive red. Cycle 1 emits environment green.
    load_combine(
        &mut rdp,
        [8, 8, 16, 3, 7, 7, 7, 7],
        [8, 8, 16, 5, 7, 7, 7, 7],
    );
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color, Rgba8::new(0, 0xFF, 0, 0));
}

#[test]
fn one_source_is_256_and_texel_times_shade_shifts() {
    let mut rdp = CpuRdpLle::new();
    rdp.prim_color = 0xFF00_0000;
    // (1 - 0) * primitive. 1 is 256, so 256*255>>8 = 255.
    load_combine(&mut rdp, [0; 8], [6, 8, 3, 7, 7, 7, 7, 7]);
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color.r, 255);

    // (texel - 0) * shade. 255*255>>8 = 254.
    load_combine(&mut rdp, [0; 8], [1, 8, 4, 7, 7, 7, 7, 7]);
    let inputs = CombineInputs {
        texel0: Rgba8::new(255, 0, 0, 255),
        shade: Rgba8::new(255, 0, 0, 255),
        ..CombineInputs::default()
    };
    let color = eval_combine(&rdp, &inputs).unwrap();
    assert_eq!(color.r, 254);
}

#[test]
fn two_cycle_passes_unclamped_combined_into_cycle1() {
    let mut rdp = CpuRdpLle::new();
    rdp.other_modes = 1 << 52;
    rdp.prim_color = 0xFF00_0000;
    rdp.env_color = 0x2D00_0000; // 45
                                 // Cycle 0: (1 * primitive) + env = 255 + 45 = 300, not clamped.
                                 // Cycle 1: combined * shade(128) >> 8 = 150. Clamped-early would be 127.
    load_combine(&mut rdp, [6, 8, 3, 5, 7, 7, 7, 7], [0, 8, 4, 7, 7, 7, 7, 7]);
    let inputs = CombineInputs {
        shade: Rgba8::new(128, 0, 0, 0),
        ..CombineInputs::default()
    };
    let color = eval_combine(&rdp, &inputs).unwrap();
    assert_eq!(color.r, 150);
}

#[test]
fn final_9bit_clamp_saturates_256_to_383_and_zeroes_384_to_511() {
    let mut rdp = CpuRdpLle::new();
    rdp.other_modes = 1 << 52;
    rdp.prim_color = 0xFF00_0000;
    // Cycle 1 passes the unclamped cycle-0 sum through D = combined.
    let pass = [8, 8, 16, 0, 7, 7, 7, 7];
    load_combine(&mut rdp, [6, 8, 3, 5, 7, 7, 7, 7], pass);
    rdp.env_color = 0x2D00_0000; // 255 + 45 = 300 → 255
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color.r, 255);
    rdp.env_color = 0x9100_0000; // 255 + 145 = 400 → 0
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color.r, 0);
}

#[test]
fn negative_9bit_window_saturates_to_255() {
    let mut rdp = CpuRdpLle::new();
    // (0 - K4) * K5, K4 = 129, K5 = 256 → -129. Low 9 bits are 383 → 255.
    let word = (0x2Cu64 << 56) | (129 << 9) | 256;
    latch_convert(&mut rdp, word).unwrap();
    assert_eq!(rdp.convert_k4, 129);
    assert_eq!(rdp.convert_k5, 256);
    load_combine(&mut rdp, [0; 8], [8, 7, 15, 7, 7, 7, 7, 7]);
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color.r, 255);
}

#[test]
fn combined_alpha_is_unclamped_across_cycles() {
    let mut rdp = CpuRdpLle::new();
    rdp.other_modes = 1 << 52;
    rdp.prim_color = 0x0000_00FF;
    rdp.env_color = 0x0000_00C8; // alpha 200
                                 // Cycle 0 alpha: 256*255>>8 + 200 = 455.
                                 // Cycle 1 RGB: 1 * combined_alpha = 455 → 9-bit clamp 0.
                                 // An inter-cycle clamp to 255 would leave red = 255.
    load_combine(
        &mut rdp,
        [8, 8, 16, 7, 6, 7, 3, 5],
        [6, 8, 7, 7, 7, 7, 7, 7],
    );
    let color = eval_combine(&rdp, &CombineInputs::default()).unwrap();
    assert_eq!(color.r, 0);
}

#[test]
fn noise_is_required_and_lod_fraction_traps_in_one_cycle() {
    let mut rdp = CpuRdpLle::new();
    load_combine(&mut rdp, [0; 8], [7, 8, 4, 7, 7, 7, 7, 7]);
    assert!(eval_combine(&rdp, &CombineInputs::default()).is_err());
    let inputs = CombineInputs {
        noise: Some(0x20),
        shade: Rgba8::new(255, 0, 0, 0),
        ..CombineInputs::default()
    };
    // 0x20 * 255 >> 8 = 31.
    let color = eval_combine(&rdp, &inputs).unwrap();
    assert_eq!(color.r, 31);

    load_combine(&mut rdp, [0; 8], [8, 8, 13, 7, 7, 7, 7, 7]);
    assert!(eval_combine(&rdp, &CombineInputs::default()).is_err());
}

#[test]
fn fill_and_copy_bypass_and_exotic_blender_modes_trap() {
    let mut rdp = CpuRdpLle::new();
    rdp.fill_color = 0xAABB_CCDD;
    load_modes(&mut rdp, 3, [0, 0, 0, 0], [0, 0, 0, 0]);
    assert_eq!(
        eval_blend(&rdp, &BlendInputs::default()).unwrap(),
        BlendOutput::Fill(0xAABB_CCDD)
    );
    assert!(eval_combine(&rdp, &CombineInputs::default()).is_err());

    load_modes(&mut rdp, 2, [0, 0, 0, 0], [0, 0, 0, 0]);
    assert_eq!(
        eval_blend(&rdp, &BlendInputs::default()).unwrap(),
        BlendOutput::Copy
    );

    // 1-cycle, B = memory coverage.
    load_modes(&mut rdp, 0, [0, 0, 0, 1], [0, 0, 0, 0]);
    assert!(eval_blend(&rdp, &BlendInputs::default()).is_err());

    load_modes(&mut rdp, 0, [0, 3, 0, 2], [0, 0, 0, 0]);
    rdp.other_modes |= 1 << 3; // antialias
    assert!(eval_blend(&rdp, &BlendInputs::default()).is_err());
    rdp.other_modes = (rdp.other_modes & !(1 << 3)) | (1 << 7); // color_on_cvg
    assert!(eval_blend(&rdp, &BlendInputs::default()).is_err());
    rdp.other_modes = (rdp.other_modes & !(1 << 7)) | (1 << 40); // key_en
    assert!(eval_blend(&rdp, &BlendInputs::default()).is_err());
    assert!(eval_combine(&rdp, &CombineInputs::default()).is_err());
}

#[test]
fn one_cycle_opaque_passthrough_and_alpha_lerp() {
    let mut rdp = CpuRdpLle::new();
    // P = combined, A = 0, M = combined, B = 1 → combined color.
    load_modes(&mut rdp, 0, [0, 3, 0, 2], [0, 0, 0, 0]);
    let inputs = BlendInputs {
        combined: Rgba8::new(1, 2, 3, 4),
        ..BlendInputs::default()
    };
    assert_eq!(
        eval_blend(&rdp, &inputs).unwrap(),
        BlendOutput::Color(Rgba8::new(1, 2, 3, 4))
    );

    // P = combined, A = combined alpha, M = memory, B = 1-A.
    load_modes(&mut rdp, 0, [0, 0, 1, 0], [0, 0, 0, 0]);
    let inputs = BlendInputs {
        combined: Rgba8::new(255, 0, 0, 128),
        memory: Rgba8::new(0, 0, 255, 255),
        ..BlendInputs::default()
    };
    assert_eq!(
        eval_blend(&rdp, &inputs).unwrap(),
        BlendOutput::Color(Rgba8::new(127, 0, 127, 128))
    );
}

#[test]
fn two_cycle_blender_uses_first_cycle_then_second_mux() {
    let mut rdp = CpuRdpLle::new();
    rdp.fog_color = 0x0000_FF00;
    // Cycle 0 mixes combined red with fog blue at shade alpha 128 → (127, 0, 127).
    // Cycle 1 passes that color through (P/M select 0 is the first cycle, not the combiner).
    load_modes(&mut rdp, 1, [0, 2, 3, 0], [0, 3, 0, 2]);
    let inputs = BlendInputs {
        combined: Rgba8::new(255, 0, 0, 9),
        shade_alpha: 128,
        ..BlendInputs::default()
    };
    assert_eq!(
        eval_blend(&rdp, &inputs).unwrap(),
        BlendOutput::Color(Rgba8::new(127, 0, 127, 9))
    );
}

#[test]
fn two_cycle_memory_is_previous_and_shade_alpha_is_next() {
    let mut rdp = CpuRdpLle::new();
    // Cycle 0: mix combined red with memory at 128. Cycle 1 passes that color through.
    load_modes(&mut rdp, 1, [0, 2, 1, 0], [0, 3, 0, 2]);
    let inputs = BlendInputs {
        combined: Rgba8::new(255, 0, 0, 4),
        shade_alpha: 128,
        memory: Rgba8::new(0, 255, 0, 0),
        memory_prev: None,
        ..BlendInputs::default()
    };
    assert!(eval_blend(&rdp, &inputs).is_err());
    let inputs = BlendInputs {
        memory_prev: Some(Rgba8::new(0, 0, 255, 0)),
        ..inputs
    };
    assert_eq!(
        eval_blend(&rdp, &inputs).unwrap(),
        BlendOutput::Color(Rgba8::new(127, 0, 127, 4))
    );

    // Cycle 1 A = shade alpha of the *next* pixel (0), so the mix is pure memory.
    load_modes(&mut rdp, 1, [0, 3, 0, 2], [0, 2, 1, 0]);
    let inputs = BlendInputs {
        combined: Rgba8::new(255, 0, 0, 4),
        shade_alpha: 255,
        shade_alpha_next: None,
        memory: Rgba8::new(0, 255, 0, 0),
        ..BlendInputs::default()
    };
    assert!(eval_blend(&rdp, &inputs).is_err());
    let inputs = BlendInputs {
        shade_alpha_next: Some(0),
        ..inputs
    };
    assert_eq!(
        eval_blend(&rdp, &inputs).unwrap(),
        BlendOutput::Color(Rgba8::new(0, 255, 0, 4))
    );
}

#[test]
fn key_registers_follow_set_key_commands() {
    let mut rdp = CpuRdpLle::new();
    let gb = (0x2Au64 << 56) | (0x11u64 << 24) | (0x22 << 16) | (0x33 << 8) | 0x44;
    latch_key_gb(&mut rdp, gb).unwrap();
    let red = (0x2Bu64 << 56) | (0x55u64 << 8) | 0x66;
    latch_key_r(&mut rdp, red).unwrap();
    assert_eq!(rdp.key_center, [0x55, 0x11, 0x33]);
    assert_eq!(rdp.key_scale, [0x66, 0x22, 0x44]);
    assert!(latch_key_r(&mut rdp, 0x3Cu64 << 56).is_err());
}
