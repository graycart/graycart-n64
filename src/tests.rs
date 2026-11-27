//! Core integration and serialization unit tests for `N64Core`.

use super::*;

#[test]
fn accurate_config_disables_all_enhancements() {
    let cfg = N64Config::accurate();
    assert_eq!(cfg.enhancements.resolution_scale, 1);
    assert_eq!(cfg.enhancements.cpu_overclock_percent, 100);
    assert!(!cfg.enhancements.vi_deblur);
    assert!(!cfg.enhancements.widescreen);
}

#[test]
fn core_state_roundtrips_via_serde() {
    let core = N64Core::new(N64Config::accurate());
    let json = serde_json::to_string(&core).expect("N64Core must serialize cleanly from M0");
    let restored: N64Core =
        serde_json::from_str(&json).expect("N64Core must deserialize cleanly from M0");
    assert_eq!(core.config, restored.config);
    assert_eq!(
        core.scheduler.now_cpu_cycles(),
        restored.scheduler.now_cpu_cycles()
    );
    assert_eq!(core.cpu.pc(), restored.cpu.pc());
    assert_eq!(core.bus.rdram.len(), restored.bus.rdram.len());
}

#[test]
fn unimplemented_cpu_step_traps_instead_of_stubbing() {
    // Cold reset vector fetches from unimplemented PIF ROM (must trap, never return 0).
    let mut core = N64Core::new(N64Config::accurate());
    let cold_outcome = core.run_cycles(10);
    assert!(matches!(
        cold_outcome,
        RunOutcome::Trapped {
            trap: Trap::UnimplementedRegister {
                device: "PIF_ROM",
                ..
            },
            ..
        }
    ));

    // Unimplemented instruction word in RDRAM must trap with UnimplementedOpcode.
    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut soft_core = N64Core::new(cfg);
    soft_core
        .bus
        .write_u32(0x0000_1000, 0x7000_0000, CacheAttr::Uncached)
        .unwrap();
    soft_core.cpu.set_pc(0xFFFF_FFFF_8000_1000);
    let opcode_outcome = soft_core.run_cycles(10);
    assert!(matches!(
        opcode_outcome,
        RunOutcome::Trapped {
            trap: Trap::UnimplementedOpcode {
                pc: 0xFFFF_FFFF_8000_1000,
                word: 0x7000_0000
            },
            ..
        }
    ));
}

/// Build a minimal valid `.z64` cart whose entry (`0x8000_0400`) spins on an idle `BEQ r0, r0, -1` loop.
fn make_idle_cart() -> Cart {
    let mut rom = vec![0u8; 0x2000];
    rom[0..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    rom[0x08..0x0C].copy_from_slice(&0x8000_0400u32.to_be_bytes());
    // 0x1000_FFFF = BEQ r0, r0, -1 (followed by NOP delay slot 0x0000_0000)
    rom[0x1000..0x1004].copy_from_slice(&0x1000_FFFFu32.to_be_bytes());
    // Known DMA payload at cart offset 0x1800 (PA 0x1000_1800)
    rom[0x1800..0x1808].copy_from_slice(&0xCAFE_BABE_1234_5678u64.to_be_bytes());
    Cart::from_bytes(&rom).expect("valid synthetic .z64")
}

#[test]
fn m2_pi_dma_immediate_data_and_delayed_mi_interrupt() {
    use crate::mi::MI_INTR_PI;
    use crate::pi::Pi;

    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut core = N64Core::with_cart(make_idle_cart(), cfg);
    // Enable MI_INTR_MASK.PI (bit 9 = set PI mask) so MI IRQ propagates to COP0.Cause.IP2.
    core.bus
        .write_u32(0x0430_000C, 1 << 9, CacheAttr::Uncached)
        .unwrap();

    // Start 8-byte PI DMA: Cart 0x1000_1800 -> RDRAM 0x0000_2000.
    core.bus
        .write_u32(0x0460_0000, 0x0000_2000, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0460_0004, 0x1000_1800, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0460_000C, 7, CacheAttr::Uncached)
        .unwrap();

    // Immediate-data contract: payload is already readable in RDRAM while DMA_BUSY is set.
    assert_eq!(
        core.bus.read_u64(0x0000_2000, CacheAttr::Uncached),
        Ok(0xCAFE_BABE_1234_5678)
    );
    assert_ne!(
        core.bus.read_u32(0x0460_0010, CacheAttr::Uncached).unwrap() & Pi::DMA_BUSY,
        0
    );
    assert_eq!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_PI,
        0
    );

    // Run enough cycles for PiDmaComplete (len=8 -> 72 cycles).
    assert!(matches!(core.run_cycles(200), RunOutcome::Completed { .. }));
    assert_eq!(
        core.bus.read_u32(0x0460_0010, CacheAttr::Uncached).unwrap() & Pi::DMA_BUSY,
        0
    );
    assert_ne!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_PI,
        0
    );
    assert_ne!(core.cpu.cop0.cause & (1 << 10), 0);

    // Clear PI interrupt via PI_STATUS write bit 1.
    core.bus
        .write_u32(0x0460_0010, 0x02, CacheAttr::Uncached)
        .unwrap();
    assert_eq!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_PI,
        0
    );
}

#[test]
fn m2_si_dma_pif_joybus_roundtrip() {
    use crate::mi::MI_INTR_SI;
    use crate::si::Si;

    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut core = N64Core::with_cart(make_idle_cart(), cfg);

    // Prepare 64-byte Joybus command frame at RDRAM 0x0000_3000:
    // Port 0: tx=1, rx=3, cmd=0x00 (Info); followed by 0xFE end marker; byte 63 = 0x01.
    let mut frame = [0u8; 64];
    frame[0] = 0x01;
    frame[1] = 0x03;
    frame[2] = 0x00;
    frame[6] = 0xFE;
    frame[63] = 0x01;
    for (i, &b) in frame.iter().enumerate() {
        core.bus
            .write_u8(0x0000_3000 + i as u32, b, CacheAttr::Uncached)
            .unwrap();
    }

    // SI WR64B (RDRAM -> PIF RAM)
    core.bus
        .write_u32(0x0480_0000, 0x0000_3000, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0480_0010, 0x1FC0_07C0, CacheAttr::Uncached)
        .unwrap();
    assert_ne!(
        core.bus.read_u32(0x0480_0018, CacheAttr::Uncached).unwrap() & Si::DMA_BUSY,
        0
    );
    assert!(matches!(
        core.run_cycles(5000),
        RunOutcome::Completed { .. }
    ));
    assert_ne!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_SI,
        0
    );

    // Clear SI IRQ, then SI RD64B (PIF RAM -> RDRAM 0x0000_3100)
    core.bus
        .write_u32(0x0480_0018, 0, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0480_0000, 0x0000_3100, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0480_0004, 0x1FC0_07C0, CacheAttr::Uncached)
        .unwrap();
    assert!(matches!(
        core.run_cycles(5000),
        RunOutcome::Completed { .. }
    ));

    // Port 0 Info response: standard controller ID 0x05, 0x00, status 0x02 (no pak) at bytes 3..6.
    assert_eq!(core.bus.read_u8(0x0000_3103, CacheAttr::Uncached), Ok(0x05));
    assert_eq!(core.bus.read_u8(0x0000_3104, CacheAttr::Uncached), Ok(0x00));
    assert_eq!(core.bus.read_u8(0x0000_3105, CacheAttr::Uncached), Ok(0x02));
}

#[test]
fn m2_ri_select_nonzero_and_vi_v_intr_fires_mi_interrupt() {
    use crate::mi::MI_INTR_VI;

    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut core = N64Core::with_cart(make_idle_cart(), cfg);

    // RI_SELECT must default non-zero (IPL3 / n64-systemtest boot trap contract).
    assert_ne!(
        core.bus.read_u32(0x0470_000C, CacheAttr::Uncached).unwrap(),
        0
    );

    // Program NTSC VI_V_SYNC = 525 half-lines (~5963 cycles/line) and VI_V_INTR = 4 (line 2).
    core.bus
        .write_u32(0x0440_0018, 525, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0440_000C, 4, CacheAttr::Uncached)
        .unwrap();

    // Run enough cycles for > 2 scanlines (~12,000 cycles).
    assert!(matches!(
        core.run_cycles(25_000),
        RunOutcome::Completed { .. }
    ));
    assert_ne!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_VI,
        0
    );

    // Writing VI_V_CURRENT clears MI_INTR_VI.
    core.bus
        .write_u32(0x0440_0010, 0, CacheAttr::Uncached)
        .unwrap();
    assert_eq!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_VI,
        0
    );
}

#[test]
fn m3_rsp_scalar_program_dma_and_break_interrupt_end_to_end() {
    use crate::mi::MI_INTR_SP;
    use crate::rsp::Sp;

    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut core = N64Core::with_cart(make_idle_cart(), cfg);

    // Enable MI_INTR_MASK.SP (bit 1 = set SP mask) so BREAK -> MI_INTR_SP -> COP0.Cause.IP2.
    core.bus
        .write_u32(0x0430_000C, 1 << 1, CacheAttr::Uncached)
        .unwrap();

    // Place a real RSP scalar program in RDRAM at 0x0000_4000 (10 instructions = 40 bytes):
    //   0x00: ADDIU r1, r0, 5        ; loop counter = 5
    //   0x04: ADDIU r2, r0, 0        ; accumulator = 0
    //   0x08: ADD   r2, r2, r1       ; sum += i
    //   0x0C: BNE   r1, r0, -2       ; loop back to 0x08 if r1 != 0
    //   0x10: ADDIU r1, r1, -1       ; (delay slot) i--
    //   0x14: SW    r2, 0(r0)        ; DMEM[0..4] = 15 (5+4+3+2+1)
    //   0x18: MTC0  r0, c0           ; SP_MEM_ADDR = 0x0000 (DMEM)
    //   0x1C: ADDIU r3, r0, 0x5000   ; RDRAM dest = 0x0000_5000
    //   0x20: MTC0  r3, c1           ; SP_DRAM_ADDR = 0x5000
    //   0x24: ADDIU r4, r0, 7        ; len = 8 bytes
    //   0x28: MTC0  r4, c3           ; SP_WR_LEN -> DMA DMEM[0..8] -> RDRAM[0x5000..0x5008]
    //   0x2C: BREAK                  ; halt + set BROKE + assert MI_INTR_SP
    let program: [u32; 12] = [
        0x2401_0005, // ADDIU r1, r0, 5
        0x2402_0000, // ADDIU r2, r0, 0
        0x0041_1020, // ADD   r2, r2, r1
        0x1420_FFFE, // BNE   r1, r0, -2 (target = 0x10 + (-8) = 0x08)
        0x2421_FFFF, // ADDIU r1, r1, -1 (delay slot)
        0xAC02_0000, // SW    r2, 0(r0)
        0x4080_0000, // MTC0  r0, c0 (SP_MEM_ADDR)
        0x2403_5000, // ADDIU r3, r0, 0x5000
        0x4083_0800, // MTC0  r3, c1 (SP_DRAM_ADDR)
        0x2404_0007, // ADDIU r4, r0, 7
        0x4084_1800, // MTC0  r4, c3 (SP_WR_LEN)
        0x0000_000D, // BREAK
    ];
    for (i, &word) in program.iter().enumerate() {
        core.bus
            .write_u32(0x0000_4000 + (i as u32) * 4, word, CacheAttr::Uncached)
            .unwrap();
    }

    // CPU DMAs the program from RDRAM 0x4000 -> RSP IMEM (0x1000), 48 bytes (len = 47).
    core.bus
        .write_u32(0x0404_0000, 0x1000, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0404_0004, 0x4000, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0404_0008, 47, CacheAttr::Uncached)
        .unwrap();

    // Set SP_PC = 0, then clear HALT (bit 0) + set INTR_ON_BREAK (bit 8).
    core.bus
        .write_u32(0x0408_0000, 0, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0404_0010, (1 << 0) | (1 << 8), CacheAttr::Uncached)
        .unwrap();
    assert!(!core.bus.sp.is_halted());

    // Step N64Core so scheduler drives RSP via exact 2/3 RCP clock ratio.
    assert!(matches!(core.run_cycles(200), RunOutcome::Completed { .. }));

    // Verify RSP finished, halted with BROKE, DMA'd sum(1..=5)=15 to RDRAM[0x5000], and raised SP IRQ!
    assert!(core.bus.sp.is_halted());
    assert_ne!(core.bus.sp.status & Sp::STATUS_BROKE, 0);
    assert_eq!(core.bus.read_u32(0x0000_5000, CacheAttr::Uncached), Ok(15));
    assert_ne!(
        core.bus.read_u32(0x0430_0008, CacheAttr::Uncached).unwrap() & MI_INTR_SP,
        0
    );
    assert_ne!(core.cpu.cop0.cause & (1 << 10), 0);
}

#[test]
fn m4_rsp_vector_program_lqv_vadd_sqv_end_to_end() {
    use crate::rsp::Sp;

    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut core = N64Core::with_cart(make_idle_cart(), cfg);

    // Seed two 16-byte vectors in DMEM at 0x000 and 0x010:
    //   vecA = [100, 200, 300, 400, 0x7FFF, -10, 0, 1]
    //   vecB = [  1,   2,   3,   4,      5, -20, 7, 9]
    let vec_a: [u16; 8] = [100, 200, 300, 400, 0x7FFF, (-10i16) as u16, 0, 1];
    let vec_b: [u16; 8] = [1, 2, 3, 4, 5, (-20i16) as u16, 7, 9];
    for i in 0..8 {
        let a_b = vec_a[i].to_be_bytes();
        let b_b = vec_b[i].to_be_bytes();
        core.bus.sp_dmem[i * 2] = a_b[0];
        core.bus.sp_dmem[i * 2 + 1] = a_b[1];
        core.bus.sp_dmem[0x10 + i * 2] = b_b[0];
        core.bus.sp_dmem[0x10 + i * 2 + 1] = b_b[1];
    }

    // RSP IMEM program:
    //   0x00: LQV v1[0], 0x00(r0)   ; sub=4, base=0, vt=1, e=0, off=0
    //   0x04: LQV v2[0], 0x10(r0)   ; sub=4, base=0, vt=2, e=0, off=1 (1 << 4 = 0x10)
    //   0x08: VADD v3, v1, v2       ; op=0x12, bit25=1, e=0, vt=2, vs=1, vd=3, funct=0x10
    //   0x0C: SQV v3[0], 0x20(r0)   ; sub=4, base=0, vt=3, e=0, off=2 (2 << 4 = 0x20)
    //   0x10: BREAK
    let prog: [u32; 5] = [
        (0x32 << 26) | (1 << 16) | (4 << 11),
        (0x32 << 26) | (2 << 16) | (4 << 11) | 1,
        (0x12 << 26) | (1 << 25) | (2 << 16) | (1 << 11) | (3 << 6) | 0x10,
        (0x3A << 26) | (3 << 16) | (4 << 11) | 2,
        0x0000_000D,
    ];
    for (i, &w) in prog.iter().enumerate() {
        let bytes = w.to_be_bytes();
        core.bus.sp_imem[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }

    // Clear HALT and run N64Core
    core.bus
        .write_u32(0x0408_0000, 0, CacheAttr::Uncached)
        .unwrap();
    core.bus
        .write_u32(0x0404_0010, 1, CacheAttr::Uncached)
        .unwrap();
    assert!(matches!(core.run_cycles(100), RunOutcome::Completed { .. }));
    assert!(core.bus.sp.is_halted());
    assert_ne!(core.bus.sp.status & Sp::STATUS_BROKE, 0);

    // Check clamped VADD result stored by SQV into DMEM[0x20..0x30]
    let expected: [u16; 8] = [101, 202, 303, 404, 0x7FFF, (-30i16) as u16, 7, 10];
    for (i, &exp) in expected.iter().enumerate() {
        let got = u16::from_be_bytes([
            core.bus.sp_dmem[0x20 + i * 2],
            core.bus.sp_dmem[0x20 + i * 2 + 1],
        ]);
        assert_eq!(got, exp, "lane {i} mismatch");
    }
}
