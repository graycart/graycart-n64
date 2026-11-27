//! ROM conformance test suite entrypoint (`cargo test --test roms`).
//!
//! Default `cargo test` runs only the ROM-free harness/manifest/parser checks.
//! Conformance ROM matrix rows are marked `#[ignore]` and require running
//! `scripts/fetch-suites.ps1` (Windows) or `scripts/fetch-suites.sh` (POSIX) first.

mod harness;

use std::path::Path;

use graycart_n64::{Cart, N64Config, N64Core, RunOutcome};
use harness::{
    evaluate_dillonb_r30, parse_systemtest_log, run_dillonb_rom, run_rom, BootOpts, Outcome,
    SystemtestReport,
};

#[test]
fn fixtures_readme_and_peterlemon_manifest_exist_and_parse() {
    let manifest_path = Path::new("tests/fixtures/peterlemon-manifest.json");
    let raw = std::fs::read_to_string(manifest_path)
        .expect("tests/fixtures/peterlemon-manifest.json must exist");
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).expect("peterlemon-manifest.json must be valid JSON");
    assert_eq!(parsed["license"], "Unlicense");
    assert!(parsed["entries"].as_array().is_some_and(|a| !a.is_empty()));

    let readme = std::fs::read_to_string("tests/fixtures/README.md")
        .expect("tests/fixtures/README.md must exist");
    assert!(readme.contains("Dillonb/n64-tests"));
    assert!(readme.contains("nemu64-test"));
}

#[test]
fn systemtest_log_parser_handles_pass_and_category_failures() {
    let pass_log = "Booting n64-systemtest...\nDone! Tests: 563. Failed: 0\n";
    assert_eq!(
        parse_systemtest_log(pass_log),
        Some(SystemtestReport {
            total: 563,
            failed: 0,
            failed_tests: vec![],
        })
    );

    let fail_log = "\
FAIL: cpu::alu::dmultu_overflow
[FAIL] cop0::tlb::probe_miss
Done! Tests: 120. Failed: 2
";
    assert_eq!(
        parse_systemtest_log(fail_log),
        Some(SystemtestReport {
            total: 120,
            failed: 2,
            failed_tests: vec![
                "cpu::alu::dmultu_overflow".to_string(),
                "cop0::tlb::probe_miss".to_string(),
            ],
        })
    );

    assert_eq!(parse_systemtest_log("still running..."), None);
}

#[test]
fn dillonb_r30_sentinel_decodes_running_pass_and_fail_id() {
    assert_eq!(evaluate_dillonb_r30(0), None);
    assert_eq!(
        evaluate_dillonb_r30(u64::MAX),
        Some(Outcome::Pass { total: 1 })
    );
    assert_eq!(
        evaluate_dillonb_r30(0x0000_0000_FFFF_FFFF),
        Some(Outcome::Pass { total: 1 })
    );
    assert_eq!(
        evaluate_dillonb_r30(42),
        Some(Outcome::Fail {
            total: 1,
            failed: 1,
            failed_names: vec!["dillonb_test_id=42".to_string()],
        })
    );
}

#[test]
fn run_rom_reports_boot_fail_when_fixture_is_missing() {
    let outcome = run_rom(
        Path::new("tests/fixtures/fetched/does_not_exist.z64"),
        &BootOpts::default(),
    );
    assert!(matches!(outcome, Outcome::BootFail(_)));
}

#[test]
#[ignore = "requires fetched suite; run `.\\scripts\\fetch-suites.ps1 -Suite dillonb`"]
fn matrix_dillonb_basic() {
    let simpleboot = Path::new("tests/fixtures/fetched/dillonb-simpleboot/basic_simpleboot.z64");
    let rom = if simpleboot.exists() {
        simpleboot
    } else {
        Path::new("tests/fixtures/fetched/dillonb/basic.z64")
    };
    let opts = BootOpts {
        soft_boot: true,
        entry: Some(0x8000_1000),
        expansion_pak: true,
        max_cycles: 20_000_000,
    };
    let (outcome, cycles, insts) = run_dillonb_rom(rom, &opts);
    println!(
        "dillonb basic ({}): outcome={outcome:?} cycles={cycles} insts={insts}",
        rom.display()
    );
    assert!(matches!(outcome, Outcome::Pass { .. }), "got {outcome:?}");
}

#[test]
#[ignore = "requires fetched suite; run `.\\scripts\\fetch-suites.ps1 -Suite dillonb`"]
fn matrix_dillonb_all_26() {
    let dir = Path::new("tests/fixtures/fetched/dillonb-simpleboot");
    let mut roms: Vec<_> = std::fs::read_dir(dir)
        .expect("run `.\\scripts\\fetch-suites.ps1 -Suite dillonb` first")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "z64"))
        .collect();
    roms.sort();
    assert!(!roms.is_empty(), "no .z64 files in {}", dir.display());

    let opts = BootOpts {
        soft_boot: true,
        entry: Some(0x8000_1000),
        expansion_pak: true,
        max_cycles: 40_000_000,
    };

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut trapped = 0usize;
    let mut timed_out = 0usize;

    println!(
        "\n{:<28} {:<10} {:>10} {:>10}  DETAIL",
        "DILLONB ROM", "STATUS", "CYCLES", "INSTS"
    );
    println!("{}", "-".repeat(92));

    for rom in &roms {
        let name = rom.file_name().unwrap().to_string_lossy();
        let (outcome, cycles, insts) = run_dillonb_rom(rom, &opts);
        let (tag, detail) = match &outcome {
            Outcome::Pass { .. } => {
                passed += 1;
                ("PASS", String::new())
            }
            Outcome::Fail { failed_names, .. } => {
                failed += 1;
                ("FAIL", failed_names.join(", "))
            }
            Outcome::Unimplemented(t) => {
                trapped += 1;
                ("TRAP", t.to_string())
            }
            Outcome::Timeout { .. } => {
                timed_out += 1;
                ("TIMEOUT", "cycle budget exhausted".to_string())
            }
            Outcome::BootFail(msg) => {
                failed += 1;
                ("BOOTFAIL", msg.clone())
            }
        };
        println!("{name:<28} {tag:<10} {cycles:>10} {insts:>10}  {detail}");
    }

    println!("{}", "-".repeat(92));
    println!(
        "Dillonb Summary: {passed}/{} PASS | {failed} FAIL | {trapped} TRAP | {timed_out} TIMEOUT",
        roms.len()
    );
    assert_eq!(passed, roms.len(), "not all Dillonb ROMs passed yet");
}

#[test]
#[ignore = "local-only smoke probe over ../carts/n64 (never required in CI)"]
fn probe_local_carts() {
    let dir = Path::new("../carts/n64");
    if !dir.exists() {
        println!("skip: local carts directory {} not present", dir.display());
        return;
    }

    let mut roms: Vec<_> = std::fs::read_dir(dir)
        .expect("read ../carts/n64")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|e| matches!(e.to_str(), Some("z64" | "n64" | "v64")))
        })
        .collect();
    roms.sort();

    println!(
        "\n{:<36} {:<8} {:<10} {:>10} {:>10} {:>7}  STATUS",
        "LOCAL CART", "CIC", "ENTRY", "CYCLES", "CPU_INST", "RSP_DMA"
    );
    println!("{}", "-".repeat(124));

    for rom in &roms {
        let name = rom.file_name().unwrap().to_string_lossy();
        let cart = match Cart::from_file(rom) {
            Ok(c) => c,
            Err(e) => {
                println!("{name:<36} ERR      {e}");
                continue;
            }
        };
        let cic = format!("{:?}", cart.cic());
        let entry = format!("0x{:08X}", cart.header().entry_pc);
        let mut cfg = N64Config::accurate();
        cfg.soft_boot = true;
        let mut core = N64Core::with_cart(cart, cfg);
        let status = match core.run_cycles(2_000_000) {
            RunOutcome::Completed { .. } => format!(
                "RUNNING (PC 0x{:016X}, rsp_inst {}, fb_hash {:016x})",
                core.cpu.pc(),
                core.rsp.instructions_executed,
                core.framebuffer_hash()
            ),
            RunOutcome::Trapped { trap, .. } => format!("TRAP: {trap}"),
        };
        println!(
            "{name:<36} {cic:<8} {entry:<10} {:>10} {:>10} {:>7}  {status}",
            core.scheduler.now_cpu_cycles(),
            core.cpu.instructions_executed(),
            core.bus.sp.dma_count
        );
    }
}

#[test]
#[ignore = "requires fetched suite; run `.\\scripts\\fetch-suites.ps1 -Suite systemtest`"]
fn matrix_n64_systemtest_base() {
    let rom = Path::new("tests/fixtures/fetched/systemtest/n64-systemtest.z64");
    let outcome = run_rom(rom, &BootOpts::default());
    assert!(matches!(outcome, Outcome::Pass { .. }), "got {outcome:?}");
}

#[test]
#[ignore = "requires fetched suite; run `.\\scripts\\fetch-suites.ps1 -Suite nemu64`"]
fn matrix_nemu64_test_base() {
    let rom = Path::new("tests/fixtures/fetched/nemu64/nemu64-test.z64");
    let outcome = run_rom(rom, &BootOpts::default());
    assert!(matches!(outcome, Outcome::Pass { .. }), "got {outcome:?}");
}
