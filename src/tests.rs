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
