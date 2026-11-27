//! ROM conformance test suite entrypoint (`cargo test --test roms`).
//!
//! Default `cargo test` runs only the ROM-free harness/manifest/parser checks.
//! Conformance ROM matrix rows are marked `#[ignore]` and require running
//! `scripts/fetch-suites.sh` first (see `tests/fixtures/README.md`).

mod harness;

use std::path::Path;

use harness::{parse_systemtest_log, run_rom, BootOpts, Outcome, SystemtestReport};

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
fn run_rom_reports_boot_fail_when_fixture_is_missing() {
    let outcome = run_rom(
        Path::new("tests/fixtures/fetched/does_not_exist.z64"),
        &BootOpts::default(),
    );
    assert!(matches!(outcome, Outcome::BootFail(_)));
}

#[test]
#[ignore = "requires fetched suite; run `bash scripts/fetch-suites.sh --suite dillonb`"]
fn matrix_dillonb_basic() {
    let rom = Path::new("tests/fixtures/fetched/dillonb/basic.z64");
    let opts = BootOpts {
        soft_boot: true,
        entry: Some(0x8000_1000),
        expansion_pak: true,
        max_cycles: 20_000_000,
    };
    let outcome = run_rom(rom, &opts);
    assert!(matches!(outcome, Outcome::Pass { .. }), "got {outcome:?}");
}

#[test]
#[ignore = "requires fetched suite; run `bash scripts/fetch-suites.sh --suite systemtest`"]
fn matrix_n64_systemtest_base() {
    let rom = Path::new("tests/fixtures/fetched/systemtest/n64-systemtest.z64");
    let outcome = run_rom(rom, &BootOpts::default());
    assert!(matches!(outcome, Outcome::Pass { .. }), "got {outcome:?}");
}

#[test]
#[ignore = "requires fetched suite; run `bash scripts/fetch-suites.sh --suite nemu64`"]
fn matrix_nemu64_test_base() {
    let rom = Path::new("tests/fixtures/fetched/nemu64/nemu64-test.z64");
    let outcome = run_rom(rom, &BootOpts::default());
    assert!(matches!(outcome, Outcome::Pass { .. }), "got {outcome:?}");
}
