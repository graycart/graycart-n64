<!--
Cited: lemmy-64/n64-systemtest; Dillonb/n64-tests; PeterLemon/N64; libdragon;
  graycart-gba 11-test-apparatus posture
URLs: see §1 tables
Note: suite inventory + harness plan; no implementation this research turn. Retrieved 2026-09-13.
-->

# graycart-n64 — Test apparatus (suite inventory + harness)

**Audience:** Dave + implementers.  
**Does not supersede:** [07-test-strategy.md](./07-test-strategy.md)  
**Phase numbers:** [PHASES.md](./PHASES.md) / [08](./08-implementation-plan.md) win.  
**Provenance:** [provenance/tests/](./provenance/tests/)

---

## Relation to doc 07

| Doc | Owns |
|-----|------|
| **07** | Layers, CI culture, anti-patterns |
| **11 (this)** | Named suites, licenses, phase→suite map, vendoring, harness API sketch |
| **12** | Merge freeze enforcement |

---

## 1. Canonical Suites (N64 Equivalents of Game Boy Blargg & Mooneye)

For Game Boy (`graycart-gb`), **Blargg** tests individual CPU instruction/subsystem correctness and **Mooneye** tests strict hardware quirks and cycle-exact edge cases. The N64 ecosystem has direct equivalents across every subsystem:

| Suite | GB Analogy | Upstream | License | Acquisition (Windows & POSIX) | Oracle Mechanism | Primary Milestones |
|---|---|---|---|---|---|---|
| **Dillonb/n64-tests** (26 CPU ROMs + `simpleboot` set) | **Blargg `cpu_instrs`** (per-instruction early CPU suite) | https://github.com/Dillonb/n64-tests | **None** (fetch-only; NEVER commit) | Prebuilt release zips (`dillon-n64-tests.zip`, `dillon-n64-tests-simpleboot.zip`) via `fetch-suites.ps1` / `.sh` | `r30 == -1` (`0xFFFF_FFFF_FFFF_FFFF`) pass; `r30 > 0` fail ID; `simpleboot` jumps straight to `0x8000_1000` | **M1** |
| **lemmy-64/n64-systemtest** | **Mooneye** (frozen comprehensive hardware-quirk suite) | https://github.com/lemmy-64/n64-systemtest | **MIT** | Source build via Rust + `nust64` (no GitHub Release binary); open IPL3 | Self-judging ISViewer log (`Done! Tests: N. Failed: M`) per category: CPU, COP0, TLB, COP1, Bus/Mem, RSP | **M1–M4, M10** |
| **thelemmy/nemu64-test** | **Mooneye (active successor, Jul 2026+)** | https://github.com/thelemmy/nemu64-test | **MIT** | Source build via Rust + `nust64 0.4.1` (includes Windows `run-base.bat`, `run-timing.bat`) | Self-judging ISViewer log + opt-in feature ROMs (`timing`, `cop0hazard`, `cycle`, `vmulf`/`vmulu` RSP stress) | **M1–M4, M10** |
| **PeterLemon/N64 (krom)** | **Blargg micro-tests + visual demos** | https://github.com/PeterLemon/N64 | **Unlicense** (exclude bundled bootcode) | Curated manifest (`tests/fixtures/peterlemon-manifest.json`); builds with `bass` | Visual / framebuffer hashes across `CPUTest`, `FPU`, `RSPTest`, `RDP` primitives | **M1, M3–M5** |
| **rasky/n64memtest** (`v0.9`) | **Memory/bus stress fixture** | https://github.com/rasky/n64memtest | MIT / libdragon | Prebuilt release `n64memtest.z64` via `fetch-suites.ps1` / `.sh` | 18 RDRAM March/walking-bit/checkerboard diagnostics + headless burn-in mode | **M2, M5** |
| **rasky/n64-systemcrash** | **Hardware freeze/crash oracle** | https://github.com/rasky/n64-systemcrash | None declared (fetch-only) | Source clone (`libdragon`); triggers real retail N64 hard freezes | Verifies Graycart's "Trap, don't stub" hardware-freeze semantics (cached MMIO, unmapped windows, RDP lockups) | **M2, M5, M10** |
| **rsp-ruination** & RSP vector stress | **RSP torture tests** | Community / `nemu64-test` `vmulf`/`vmulu` features | Check upstream | `nemu64-test` stress features + community RSP torture ROMs | Exact 48-bit VU accumulator carry & clamping verification | **M4** |
| **DragonMinded/libdragon** examples & open IPL3 | **Homebrew SDK integration & audio/RDP smoke** | https://github.com/DragonMinded/libdragon | **Unlicense** | Built examples / open IPL3 | Real open-IPL3 boot path, `rdpq` 2D/3D frames, RSP audio mixer PCM hash | **M2, M4–M7** |
| **240p Test Suite (N64)** | **Display/VI calibration oracle** | https://github.com/ArtemioUrbina/240pTestSuite | **GPL-2.0+** (fetch-only; NEVER commit) | Official release download into `tests/fixtures/fetched/240p/` | VI resolution, de-interlace, overscan, color/gamma, and scanline checks | **M5, M10** |
| **RDP Golden Corpus (offline)** | **PPU golden frame hashes** | Hand-built DLs cross-checked offline vs `angrylion-rdp-plus` / `paraLLEl-RDP` | Goldens/scripts only (NEVER port MAME/GPL code) | Offline reference renders | Pre-VI-filter and post-VI-filter FNV-1a/SHA-256 frame hashes (`CpuRdpLle` == `EdgeWalker`) | **M5–M6** |
| **Local Commercial Corpus** (`..\carts\n64\`) | **Real-world smoke & regression probe** | User's local `carts/n64/` (gitignored, outside repo) | Proprietary (**NEVER** commit or require in CI) | Local disk only (covers CIC-6102 & CIC-6105, 4/8 MiB, EEPROM/SRAM/FlashRAM) | Step-by-step boot progress probe (boot stage, cycles, trap PC, framebuffer hash); M9 title-screen hashes | **M1–M9 (progress probe)** |

### 1.1 Dillonb/n64-tests — young CPU gate (verified 26-ROM inventory)

| | |
|--|--|
| **URL** | https://github.com/Dillonb/n64-tests |
| **License** | **None declared** — strictly fetch-and-run only (`THIRD_PARTY.md`) |
| **Prebuilt Release Assets** | `dillon-n64-tests.zip` and `dillon-n64-tests-simpleboot.zip` under release tag `latest` (26 `.z64` ROMs each: `basic`, `addiu`, `addu`, `and`, `andi`, `daddiu`, `dsll`, `dsll32`, `dsllv`, `dsra32`, `nor`, `or`, `ori`, `sll`, `sllv`, `slt`, `slti`, `sltiu`, `sltu`, `sra`, `srav`, `srl`, `srlv`, `subu`, `xor`, `xori`) |
| **Oracle** | `r30 == -1` (`u64::MAX`) pass; `r30 > 0` fail test ID; `r30 == 0` still running |
| **Soft-boot** | PC `0x8000_1000`; copy up to `0x10_0000` bytes from cart `0x1000` → RDRAM `0x0000_1000` |
| **Provenance** | [tests/dillonb-n64-tests.md](./provenance/tests/dillonb-n64-tests.md) |

### 1.2 lemmy-64/n64-systemtest & thelemmy/nemu64-test — broad Mooneye-class boards

| | |
|--|--|
| **URLs** | https://github.com/lemmy-64/n64-systemtest (frozen) · https://github.com/thelemmy/nemu64-test (active successor) |
| **License** | **MIT** |
| **Covers** | VR4300 64-bit/32-bit ISA, COP0 wide regs, LL/SC, exceptions, TLB, COP1 FPU, mem sizes/bus quirks, RSP SU+VU, RDP; optional `timing`/`cycle`/`cop0hazard`/`vmulf`/`vmulu` feature ROMs |
| **Build & Boot** | Built locally with Rust + `nust64`; boots via real libdragon open IPL3 and self-reports over ISViewer 64 |
| **Provenance** | [tests/n64-systemtest.md](./provenance/tests/n64-systemtest.md) |

### 1.3 PeterLemon/N64 — bare-metal micro-tests & visuals

| | |
|--|--|
| **URL** | https://github.com/PeterLemon/N64 |
| **License** | **Unlicense** (curated manifest only; exclude bundled bootcode) |
| **Covers** | Bare-metal CPU/FPU/RSP/RDP micro-tests and demos |
| **Oracle** | Frame hashes / visual smoke |
| **Provenance** | [tests/peterlemon-n64.md](./provenance/tests/peterlemon-n64.md) |

### 1.4 rasky/n64memtest & rasky/n64-systemcrash — RDRAM & hardware-freeze oracles

| | |
|--|--|
| **URLs** | https://github.com/rasky/n64memtest (`v0.9` prebuilt `n64memtest.z64`) · https://github.com/rasky/n64-systemcrash (source, fetch-only) |
| **Covers** | 18 RDRAM March/walking-bit tests (headless burn-in supported) and deliberate retail N64 hardware freeze/crash triggers |

### 1.5 RDP golden corpus & libdragon examples

| | |
|--|--|
| **Source** | Hand-built DLs compared offline to `angrylion-rdp-plus` / `paraLLEl-RDP`, plus [DragonMinded/libdragon](https://github.com/DragonMinded/libdragon) (Unlicense) examples & open IPL3 |
| **Vendor** | Goldens + scripts only (zero third-party RDP code) |
| **Provenance** | [tests/rdp-goldens.md](./provenance/tests/rdp-goldens.md) |

---

## 2. Phase → must suites

| Phase | Must | Stretch |
|-------|------|---------|
| P0 | units + fixture stubs | — |
| P1 | Dillonb CPU | systemtest CPU |
| P2 | systemtest mem subset / units | PI quirk ROMs |
| P3 | RSP DMA+SU ROM/unit | — |
| P4 | VU unit + task | — |
| P5 | ≥3 RDP demo hashes | 1 angrylion DL |
| P6 | AI beep | soft WAV |
| P7 | save roundtrip | Flash full |
| P8 | systemtest threshold T₀ | timing feature |
| P9+ | per PHASES | — |

---

## 3. Vendoring rules

| Artifact | Policy |
|----------|--------|
| MIT/Apache homebrew prebuilts | Vendor with LICENSE + pin SHA |
| GPL suites | Document fetch script; avoid mixing into MIT crate without legal review |
| Commercial | `carts/` gitignore only |
| PIF/CIC dumps | **Never** |

---

## 4. Harness API sketch

```rust
// tests/roms/harness.rs
fn run_rom(path: &Path, opts: BootOpts) -> Outcome { … }

struct BootOpts {
    soft_boot: bool,
    entry: Option<u32>,
    expansion_pak: bool,
    max_cycles: u64,
}
```

Matrices behind `#[ignore]` until fixtures present. Path/LICENSE stubs always compile.

---

## 5. No apparatus implementation in this research turn

Design only — coding waits on [08 §7](./08-implementation-plan.md#7-acceptance-criteria-dave-review-gate).
