//! Integration smoke tests for the headless CLI runner and ISViewer capture.
//! Uses synthetic in-memory/tempdir ROMs only (zero commercial or third-party ROMs).

use std::fs;
use std::process::Command;

use graycart_n64::bus::ISVIEWER_BASE;
use graycart_n64::{CacheAttr, Cart, N64Config, N64Core};

fn synthetic_z64(title: &str, entry_pc: u32) -> Vec<u8> {
    let mut rom = vec![0u8; 0x2000];
    rom[0..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    rom[8..12].copy_from_slice(&entry_pc.to_be_bytes());
    let tb = title.as_bytes();
    rom[0x20..0x20 + tb.len().min(20)].copy_from_slice(&tb[..tb.len().min(20)]);
    // Place an explicitly reserved/unimplemented MIPS III opcode (0x7000_0000, op 0x1C) at
    // entry offset 0x1000 so the CLI smoke test reliably verifies trap exit code 2.
    rom[0x1000..0x1004].copy_from_slice(&0x7000_0000u32.to_be_bytes());
    rom
}

#[test]
fn cli_version_and_help_succeed() {
    let bin = env!("CARGO_BIN_EXE_graycart-n64");

    let ver = Command::new(bin)
        .arg("--version")
        .output()
        .expect("run --version");
    assert!(ver.status.success());
    let stdout = String::from_utf8_lossy(&ver.stdout);
    assert!(stdout.contains("graycart-n64 0.0.1"), "got: {stdout}");

    let help = Command::new(bin)
        .arg("--help")
        .output()
        .expect("run --help");
    assert!(help.status.success());
    let help_out = String::from_utf8_lossy(&help.stdout);
    assert!(help_out.contains("--frames"));
    assert!(help_out.contains("--hash-out"));
}

#[test]
fn cli_runs_synthetic_rom_writes_hash_and_traps_on_m0_cpu_stub() {
    let bin = env!("CARGO_BIN_EXE_graycart-n64");
    let dir = std::env::temp_dir().join(format!("graycart_n64_cli_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let rom_path = dir.join("synthetic.z64");
    let hash_path = dir.join("frame.hash");
    fs::write(&rom_path, synthetic_z64("SMOKE ROM", 0x8000_1000)).unwrap();

    let out = Command::new(bin)
        .args([
            "--frames",
            "1",
            "--soft-boot",
            "--hash-out",
            hash_path.to_str().unwrap(),
            rom_path.to_str().unwrap(),
        ])
        .output()
        .expect("run synthetic rom");

    // Exit code 2 confirms explicit Trap on unimplemented M0 CPU opcode (not silent NOP).
    assert_eq!(out.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("SMOKE ROM"), "stdout: {stdout}");
    assert!(
        stderr.contains("PC 0xFFFFFFFF80001000"),
        "expected soft-boot entry PC in trap, stderr: {stderr}"
    );

    let hash_str = fs::read_to_string(&hash_path).expect("hash-out file must exist");
    let trimmed = hash_str.trim();
    assert_eq!(trimmed.len(), 16, "expected 64-bit hex hash, got {trimmed}");
    assert!(trimmed.chars().all(|c| c.is_ascii_hexdigit()));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn core_captures_isviewer_stream_from_public_api() {
    let cart = Cart::from_bytes(&synthetic_z64("ISV TEST", 0x8000_1000)).unwrap();
    let mut cfg = N64Config::accurate();
    cfg.soft_boot = true;
    let mut core = N64Core::with_cart(cart, cfg);

    let log = b"Hello ISViewer\n";
    for (i, &b) in log.iter().enumerate() {
        core.bus
            .write_u8(ISVIEWER_BASE + 0x20 + i as u32, b, CacheAttr::Uncached)
            .unwrap();
    }
    core.bus
        .write_u32(ISVIEWER_BASE + 0x14, log.len() as u32, CacheAttr::Uncached)
        .unwrap();

    assert_eq!(core.isviewer_output(), log);
}
