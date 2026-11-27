//! Headless CLI runner for `graycart-n64` (zero GUI dependencies).
//!
//! Supports `--frames`, `--cycles`, `--hash-out`, `--isviewer-out`, `--soft-boot`,
//! `--expansion-pak` / `--no-expansion-pak`, `--single-step`, `--version`, and `--help`.
//!
//! Cited: Graycart N64 Development Plan §4 M0 & §7.5 · docs/PHASES.md P0

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use graycart_n64::{Cart, N64Config, N64Core, RunOutcome};

#[derive(Debug, Default)]
struct CliArgs {
    rom_path: Option<PathBuf>,
    frames: Option<u64>,
    cycles: Option<u64>,
    hash_out: Option<PathBuf>,
    isviewer_out: Option<PathBuf>,
    soft_boot: bool,
    expansion_pak: bool,
    single_step: bool,
    show_version: bool,
    show_help: bool,
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<CliArgs, String> {
    let mut parsed = CliArgs {
        expansion_pak: true,
        ..CliArgs::default()
    };
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--version" | "-V" => parsed.show_version = true,
            "--help" | "-h" => parsed.show_help = true,
            "--soft-boot" => parsed.soft_boot = true,
            "--expansion-pak" => parsed.expansion_pak = true,
            "--no-expansion-pak" => parsed.expansion_pak = false,
            "--single-step" => parsed.single_step = true,
            "--frames" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--frames requires an integer argument".to_string())?;
                let n = raw
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --frames value: {raw}"))?;
                parsed.frames = Some(n);
            }
            "--cycles" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--cycles requires an integer argument".to_string())?;
                let n = raw
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --cycles value: {raw}"))?;
                parsed.cycles = Some(n);
            }
            "--hash-out" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--hash-out requires a file path".to_string())?;
                parsed.hash_out = Some(PathBuf::from(raw));
            }
            "--isviewer-out" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--isviewer-out requires a file path".to_string())?;
                parsed.isviewer_out = Some(PathBuf::from(raw));
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag: {other}"));
            }
            positional => {
                if parsed.rom_path.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument: {positional}"
                    ));
                }
                parsed.rom_path = Some(PathBuf::from(positional));
            }
        }
    }
    Ok(parsed)
}

fn print_usage() {
    println!(
        "graycart-n64 {} (headless runner)\n\n\
         USAGE:\n\
         \tgraycart-n64 [OPTIONS] <ROM_PATH>\n\n\
         OPTIONS:\n\
         \t--frames <N>         Run for N frames (default: 1)\n\
         \t--cycles <N>         Run for N CPU cycles (overrides --frames)\n\
         \t--hash-out <PATH>    Write deterministic framebuffer hash hex to PATH\n\
         \t--isviewer-out <PATH> Write captured ISViewer output bytes to PATH\n\
         \t--soft-boot          Enable HLE soft-boot stub (PC = header entry)\n\
         \t--expansion-pak      Enable 8 MiB RDRAM (default)\n\
         \t--no-expansion-pak   Use base 4 MiB RDRAM\n\
         \t--single-step        Clamp scheduler slices to 1 instruction\n\
         \t-V, --version        Print version and exit\n\
         \t-h, --help           Print this help message and exit",
        env!("CARGO_PKG_VERSION")
    );
}

fn run(args: CliArgs) -> ExitCode {
    if args.show_version {
        println!("graycart-n64 {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.show_help {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let Some(rom_path) = args.rom_path else {
        eprintln!("error: missing <ROM_PATH> (pass --help for usage)");
        return ExitCode::from(1);
    };

    let cart = match Cart::from_file(&rom_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error loading ROM '{}': {e}", rom_path.display());
            return ExitCode::from(1);
        }
    };

    println!(
        "rom: title=\"{}\" format={:?} cic={:?} entry=0x{:08X} size={}B",
        cart.header().title,
        cart.source_format(),
        cart.cic(),
        cart.header().entry_pc,
        cart.rom().len()
    );

    let mut config = N64Config::accurate();
    config.expansion_pak = args.expansion_pak;
    config.soft_boot = args.soft_boot;
    config.single_step_mode = args.single_step;

    let mut core = N64Core::with_cart(cart, config);
    let outcome = if let Some(cycles) = args.cycles {
        core.run_cycles(cycles)
    } else {
        core.run_frames(args.frames.unwrap_or(1))
    };

    let fb_hash = core.framebuffer_hash();
    if let Some(hash_path) = &args.hash_out {
        if let Err(e) = fs::write(hash_path, format!("{fb_hash:016x}\n")) {
            eprintln!("error writing hash-out '{}': {e}", hash_path.display());
            return ExitCode::from(1);
        }
    }

    let isv = core.isviewer_output();
    if let Some(isv_path) = &args.isviewer_out {
        if let Err(e) = fs::write(isv_path, isv) {
            eprintln!("error writing isviewer-out '{}': {e}", isv_path.display());
            return ExitCode::from(1);
        }
    }
    if !isv.is_empty() {
        print!("{}", String::from_utf8_lossy(isv));
    }

    match outcome {
        RunOutcome::Completed { cycles, frames } => {
            println!(
                "outcome: completed cycles={cycles} frames={frames} fb_hash={fb_hash:016x} isviewer_bytes={}",
                isv.len()
            );
            ExitCode::SUCCESS
        }
        RunOutcome::Trapped { cycles, trap } => {
            eprintln!(
                "outcome: trapped cycles={cycles} fb_hash={fb_hash:016x} isviewer_bytes={} trap=\"{trap}\"",
                isv.len()
            );
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    match parse_args(env::args()) {
        Ok(args) => run(args),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(1)
        }
    }
}
