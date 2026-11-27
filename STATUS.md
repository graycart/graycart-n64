# Graycart N64 — Accuracy & Ruled-Out Ledger (`STATUS.md`)

Loaded into every implementation session per [`AGENTS.md`](./AGENTS.md) and [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) §5.6.
Milestones advance only on pasted, verifiable test receipts reviewed by Dave.

## Active Milestone

- **Current:** `M2 — Bus + boot` receipts complete on `feat/m1-cpu` (ready for Dave's review & M3 RSP scalar + DMA kickoff)
- **Bespoke Vulkan Compute LLE RDP Name:** **`EdgeWalker`** (zero third-party RDP forks/FFI)
- **Debugger & UI Standard:** [`docs/13-debugger-and-ui-architecture.md`](./docs/13-debugger-and-ui-architecture.md) (Departure Mono + `btop`-inspired Machine Monitor with live CPU/Memory/GPU/Audio graphs matching `graycart-gb` & `graycart-gba`)
- **Pre-code gate:** Accepted by Dave on 2026-11-27

## Milestone Ledger (M0–M12)

| Milestone | Status | Gate Receipt Summary |
|---|---|---|
| **M0 — Scaffold** | **Done** (PR #2 merged, CI green 3/3 OS) | 21 passed (15 unit, 3 CLI smoke, 3 ROM harness), 0 failed, 3 ignored matrix stubs compiled; fmt/clippy `-D warnings` clean; macOS/Ubuntu/Windows CI green; zero forbidden binaries tracked |
| **M1 — CPU functional** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | Full VR4300 ISA + COP0/Exceptions + 32-entry TLB + COP1 IEEE-754 FPU + L1 I/D Caches + VR4300 `SRA`/`SRAV` 64-bit hardware quirk; **26/26 Dillonb `n64-tests` PASS** (0 FAIL, 0 TRAP, 0 TIMEOUT) |
| **M2 — Bus + boot** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | 125 passed (118 unit, 3 CLI smoke, 4 ROM harness), 0 failed, 5 ignored; MI/RI/PI/SI/PIF-HLE/VI/AI registers & scheduled DMA/IRQ wired; **26/26 Dillonb PASS**; **8/8 local commercial carts running 200k cycles with 0 traps** |
| **M3 — RSP scalar + DMA** | Next | — |
| **M4 — RSP vector unit** | Not started | — |
| **M5 — Bespoke Software LLE RDP (`CpuRdpLle`) + VI** | Not started | — |
| **M6 — Bespoke Vulkan Compute LLE RDP (`EdgeWalker`)** | Not started | — |
| **M7 — Audio (AI + RSP mixer)** | Not started | — |
| **M8 — PIF/Joybus/Saves/4-port Transfer Pak** | Not started | — |
| **M9 — Commercial smoke (local-only)** | Not started | — |
| **M10 — Timing hardening (`timing`/`cop0hazard`/`cycle`)** | Not started | — |
| **M11 — Modern Enhancements & Player Experience Tier** | Not started | — |
| **M12 — Optional dynarec + Family API extract** | Not started | — |

## Receipts

### M2 — Bus, Peripherals (MI/RI/PI/SI/PIF/VI/AI) & 26/26 Dillonb CPU Conformance (`feat/m1-cpu`, 2026-11-27)

- 4 parallel `flash_lite` workers (`mi`+`ri`, `pi`, `si`+`pif`, `vi`+`ai`) + Orchestrator wiring in `src/bus/mod.rs` and `src/lib.rs` (immediate-data / delayed-MI-interrupt PI DMA, SI 64B PIF<->RDRAM DMA + Joybus HLE port 0, CIC boot seeds `6101/6102/6103/6105/6106`, `RI_SELECT != 0` IPL3 boot contract, register-driven `VI` scanlines + `VI_V_INTR`, `AI` 2-slot FIFO skeleton, continuous `MI` -> `COP0.Cause.IP2` propagation).
- Orchestrator review & hardware conformance catches fixed before commit:
  - `src/pif/mod.rs`: Joybus RX response offset fixed from `i + 3` to `i + 2 + tx`.
  - `src/cpu/shift.rs`: Dillonb `sra_simpleboot.z64` (test 1501) and `srav_simpleboot.z64` (test 201) caught the NEC VR4300 32-bit `SRA`/`SRAV` 64-bit right-shift hardware quirk (N64brew: shifts full 64-bit `rt` before sign-extending bit 31, so dirty upper 32 bits shift down into bits `31..0`). Fixed in `src/cpu/shift.rs` and locked with unit tests in `src/cpu/shift_tests.rs`.
- Local validation (Windows PowerShell, `RUST_MIN_STACK=16777216`):
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 lint warnings)
  - `cargo test` → exit `0`:
    - `src/lib.rs` unit tests: **118 passed; 0 failed; 0 ignored** (+25 new unit & end-to-end M2 bus/DMA/IRQ integration tests)
    - `tests/cli_smoke.rs`: **3 passed; 0 failed; 0 ignored**
    - `tests/roms/main.rs`: **4 passed; 0 failed; 5 ignored**
  - `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → exit `0` (**4.45s**):
    - **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`** across all 26 `Dillonb/n64-tests` simpleboot ROMs (`addiu`, `addu`, `and`, `andi`, `basic`, `daddiu`, `dsll`, `dsll32`, `dsllv`, `dsra32`, `nor`, `or`, `ori`, `sll`, `sllv`, `slt`, `slti`, `sltiu`, `sltu`, `sra`, `srav`, `srl`, `srlv`, `subu`, `xor`, `xori`).
  - `cargo test --release --test roms probe_local_carts -- --ignored --nocapture` (local-only `..\carts\n64`, **0.14s**):
    - **8/8 commercial carts ran 200,000 cycles cleanly with ZERO traps** across `Cic6102` (SM64, MK64, GoldenEye 007 in TLB KUSEG `0x70000584`, Star Wars Ep. I Racer), `Cic6103` (Super Smash Bros.), and `Cic6105` (Ocarina of Time, Majora's Mask, Donkey Kong 64).
  - Forbidden-binary git check → `Clean: no forbidden binaries tracked`.

### M1 Wave B — COP0/Exceptions, 32-Entry TLB, COP1 FPU & L1 Caches (`feat/m1-cpu`, 2026-11-27)

- Orchestrator M1.B0 seam (`src/cpu/{mod,mmu,exceptions,decode}.rs`, `CountClock` `CompareMatch` dispatch in `src/lib.rs`) + 4 parallel `flash_lite` Wave B workers (`cop0.rs`, `tlb.rs`, `cop1.rs`, `cache.rs`).
- Orchestrator review caught & fixed 6 hardware edge bugs before merge: `REGIMM` trap sign-extension before unsigned compare, `XContext.R` bit position (`<< 31`), `EntryHi.VPN2` 64-bit mask, `ERET` preserving `EPC`/`ErrorEPC` untouched, uninitialized TLB entries using KSEG0 sentinel `0x8000_0000` (so VA `0x0000_0000` raises TLB Refill instead of Invalid), `ROUND.W`/`ROUND.L` always ties-to-even independent of `FCR31.RM`, and `CACHE` `0x0D` (`Create Dirty Exclusive`) translating VA via MMU + calling `raise_exception`.
- Local validation (Windows PowerShell, `RUST_MIN_STACK=16777216`):
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 lint warnings)
  - `cargo test` → exit `0`:
    - `src/lib.rs` unit tests: **93 passed; 0 failed; 0 ignored** (+34 new unit tests across COP0/Exceptions, TLB, COP1 FPU, and L1 I/D Caches)
    - `tests/cli_smoke.rs`: **3 passed; 0 failed; 0 ignored**
    - `tests/roms/main.rs`: **4 passed; 0 failed; 5 ignored**
- Local commercial cart progress board (`..\carts\n64`, local-only):
  - **Zero unimplemented CPU opcodes across all 8 carts and all 26 Dillonb ROMs.**
  - **GoldenEye 007:** programmed the TLB, jumped into TLB-mapped KUSEG (`PC 0x0000000070000584`), and executed **200,000+ instructions** cleanly.
  - **Super Mario 64** (`115,069` insts), **Donkey Kong 64** (`15,258` insts), and **Zelda: Majora's Mask** (`16,311` insts): cleared `MFC0 Status` + CPU/cache init and reached `SI_STATUS` (`0x0018`, M2).
  - **Ocarina of Time**, **Mario Kart 64**, and **Super Smash Bros.**: running cleanly past `200,000` instructions.
  - **Star Wars Ep. I Racer** (`17` insts) & **Dillonb 26 ROMs**: waiting strictly on M2 `PI`/`VI` registers.

### M1.0 + Wave A — VR4300 Integer ISA (`feat/m1-cpu`, 2026-11-27)

- Orchestrator M1.0 seam (`src/cpu/{mod,mmu,exceptions,decode}.rs`, `src/bus/mod.rs` 16-bit + entry-PC soft-boot, `tests/roms/{harness,main}.rs` Dillonb `r30` sentinel + 26-ROM matrix + local cart probe) + 5 parallel `flash_lite` Wave A workers (`alu.rs`, `shift.rs`, `mult_div.rs`, `branch.rs`, `load_store.rs`).
- Orchestrator review caught & fixed 3 hardware bugs before merge (`SH` misaligned `AdES`, `SDL`/`SDR` `shift == 0` full-overwrite mask) plus 2 test-file assertion fixes.
- Local validation (Windows PowerShell, `RUST_MIN_STACK=16777216`):
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 lint warnings)
  - `cargo test` → exit `0`:
    - `src/lib.rs` unit tests: **59 passed; 0 failed; 0 ignored** (+44 new CPU unit tests across ALU, Shift, Mult/Div, Branch, Load/Store)
    - `tests/cli_smoke.rs`: **3 passed; 0 failed; 0 ignored**
    - `tests/roms/main.rs`: **4 passed; 0 failed; 5 ignored**
- Fixture & local-cart progress probe:
  - Dillonb `dillonb-simpleboot` (26 ROMs): `basic_simpleboot.z64` executed **66 instructions** to `VI` init trap (`0x0000`); remaining 25 ROMs executed **9 instructions** to `PI` DMA trap (`0x0000`) — zero CPU ISA failures.
  - Local commercial smoke probe (8 carts, local-only): SM64 `115,041` insts → `MFC0 Status`; GoldenEye `125,537` insts → `MTC0 Index`; DK64 `15,230` insts → `MFC0 Status`; Majora's Mask `16,277` insts → `MFC0 Status`; OoT / MK64 / SSB ran `200,000` insts cleanly; Star Wars Racer `17` insts → `VI` write. Next: **M1 Wave B** (COP0/Exceptions, TLB, COP1, Cache).

### M0 — Scaffold (`feat/m0-scaffold` / PR #2, 2026-11-27)

- Local validation:
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 warnings)
  - `RUST_MIN_STACK=16777216 cargo test` → exit `0`:
    - `src/lib.rs` unit tests: **15 passed; 0 failed; 0 ignored** (core serde round-trip, anti-stub traps, full physical decode map, cached-MMIO freeze, unmapped & 64-bit RCP freezes, RDRAM 4/8 MiB + bit-packed 9th-bit array, ISViewer ring buffer + wrap, 2/3 RCP rational accumulator, deterministic event queue + single-step mode, Count/Compare mid-slice & 32-bit wrap-safe scheduling, `.z64`/`.v64`/`.n64` normalization, header parse, CIC CRC32 table)
    - `tests/cli_smoke.rs`: **3 passed; 0 failed; 0 ignored** (`--version`/`--help`, synthetic ROM run + `--hash-out` + explicit M0 CPU trap at soft-boot entry PC, ISViewer public API stream capture)
    - `tests/roms/main.rs`: **3 passed; 0 failed; 3 ignored** (`tests/fixtures/README.md` + `peterlemon-manifest.json` validation, `n64-systemtest` / `nemu64-test` ISViewer log parser, missing-fixture `BootFail`; ignored matrix stubs `matrix_dillonb_basic`, `matrix_n64_systemtest_base`, `matrix_nemu64_test_base`)
  - `cargo test --tests --no-run` → exit `0` (all ignored ROM harness targets compile cleanly)
  - `cargo run -- --version` → `graycart-n64 0.0.1`
  - `bash -n scripts/fetch-suites.sh && bash scripts/fetch-suites.sh --dry-run` → exit `0`
  - Forbidden-binary git check → `Clean: no forbidden binaries tracked`
- GitHub Actions CI (PR #2):
  - `macos-latest`: **pass** (31s)
  - `ubuntu-latest`: **pass** (1m1s)
  - `windows-latest`: **pass** (1m47s)

## Ruled Out

Approaches, claims, or hypotheses already checked and rejected so no session retries them:

| Date | Area | Ruled Out | Why / Replacement |
|---|---|---|---|
| 2026-11-27 | Audio (`docs/05-audio.md`) | "AI delayed-carry bug" as a single named hardware bug | No primary source uses that name; conflated distinct behaviors. Track separately: (1) ares v129 queued-DMA length latching bug, (2) AI 8 KiB (`0x2000`) boundary DMA address quirk (libdragon/n64brew), and (3) RSP `VADDC`/accumulator carry semantics. |
| 2026-11-27 | Scheduler | Cooperative coroutines (`libco`-style) & per-cycle pipeline at M0 | Coroutines fight Rust `&mut Bus` ownership; full pipeline is an M10 hardening target. Use single-threaded deterministic event-queue slice scheduler with exact 2/3 RCP rational accumulator. |
| 2026-11-27 | GPU RDP | Linking `paraLLEl-RDP` via C++ FFI or forking third-party RDP code | Dave's architectural call: zero third-party RDP forks/FFI. Build bespoke `CpuRdpLle` (M5 oracle) + bespoke Vulkan compute LLE RDP **`EdgeWalker`** (M6 parity → M11 enhancements). |
| 2026-11-27 | Test vendoring | Committing `Dillonb/n64-tests` or `240p Test Suite` ROMs in git | Dillonb has no license (never redistribute); 240p Suite is GPL-2.0+ (incompatible with MIT crate). Fetch-and-run only via `scripts/fetch-suites.sh` into gitignored `tests/fixtures/fetched/`. |
