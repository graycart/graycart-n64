//! Shared ROM conformance harness and self-judging ISViewer log parser.
//!
//! Cited: docs/07-test-strategy.md §4 · docs/11-test-apparatus.md §4 · docs/12-test-gates.md
//! Note: Keeps all ROM matrix rows behind `#[ignore]` so default CI stays 100% ROM-free.

use std::path::Path;

use graycart_n64::{Cart, N64Config, N64Core, RunOutcome, Trap};

/// Boot and execution options for conformance ROM runs.
#[derive(Debug, Clone)]
pub struct BootOpts {
    pub soft_boot: bool,
    pub entry: Option<u32>,
    pub expansion_pak: bool,
    pub max_cycles: u64,
}

impl Default for BootOpts {
    fn default() -> Self {
        Self {
            soft_boot: false,
            entry: None,
            expansion_pak: true,
            max_cycles: 50_000_000,
        }
    }
}

/// Structured conformance run outcome (distinguishes emulator trap vs. suite fail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass {
        total: usize,
    },
    Fail {
        total: usize,
        failed: usize,
        failed_names: Vec<String>,
    },
    Timeout {
        cycles: u64,
    },
    Unimplemented(Trap),
    BootFail(String),
}

/// Parsed summary from `n64-systemtest` / `nemu64-test` ISViewer logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemtestReport {
    pub total: usize,
    pub failed: usize,
    pub failed_tests: Vec<String>,
}

/// Parse `n64-systemtest` / `nemu64-test` ISViewer output.
///
/// Recognizes failure lines (`FAIL <name>` or `Failed: <name>`) and the final
/// self-judging summary line (`Done! Tests: N. Failed: M`).
#[must_use]
pub fn parse_systemtest_log(log: &str) -> Option<SystemtestReport> {
    let mut failed_tests = Vec::new();
    let mut summary: Option<(usize, usize)> = None;

    for raw_line in log.lines() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("Done! Tests:") {
            let mut parts = rest.split("Failed:");
            let total_str = parts.next()?.trim().trim_end_matches('.');
            let failed_str = parts.next()?.trim().trim_end_matches('.');
            let total = total_str.parse::<usize>().ok()?;
            let failed = failed_str.parse::<usize>().ok()?;
            summary = Some((total, failed));
        } else if let Some(name) = line
            .strip_prefix("FAIL:")
            .or_else(|| line.strip_prefix("FAIL "))
            .or_else(|| line.strip_prefix("[FAIL]"))
        {
            let cleaned = name.trim();
            if !cleaned.is_empty() {
                failed_tests.push(cleaned.to_string());
            }
        }
    }

    summary.map(|(total, failed)| SystemtestReport {
        total,
        failed,
        failed_tests,
    })
}

/// Run a ROM from disk under `opts` and return a structured [`Outcome`].
pub fn run_rom(path: &Path, opts: &BootOpts) -> Outcome {
    let cart = match Cart::from_file(path) {
        Ok(c) => c,
        Err(e) => return Outcome::BootFail(e.to_string()),
    };

    let mut cfg = N64Config::accurate();
    cfg.soft_boot = opts.soft_boot;
    cfg.expansion_pak = opts.expansion_pak;

    let mut core = N64Core::with_cart(cart, cfg);
    if let Some(entry) = opts.entry {
        core.cpu.set_pc(entry as i32 as i64 as u64);
    }

    match core.run_cycles(opts.max_cycles) {
        RunOutcome::Trapped { trap, .. } => Outcome::Unimplemented(trap),
        RunOutcome::Completed { cycles, .. } => {
            let log = String::from_utf8_lossy(core.isviewer_output());
            match parse_systemtest_log(&log) {
                Some(report) if report.failed == 0 => Outcome::Pass {
                    total: report.total,
                },
                Some(report) => Outcome::Fail {
                    total: report.total,
                    failed: report.failed,
                    failed_names: report.failed_tests,
                },
                None => Outcome::Timeout { cycles },
            }
        }
    }
}
