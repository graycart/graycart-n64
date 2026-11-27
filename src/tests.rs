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
    let mut core = N64Core::new(N64Config::accurate());
    let outcome = core.run_cycles(10);
    assert!(matches!(
        outcome,
        RunOutcome::Trapped {
            trap: Trap::UnimplementedOpcode { .. },
            ..
        }
    ));
}
