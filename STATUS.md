# Graycart N64 — Accuracy & Ruled-Out Ledger (`STATUS.md`)

Loaded into every implementation session per [`AGENTS.md`](./AGENTS.md) and [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) §5.6.
Milestones advance only on pasted, verifiable test receipts reviewed by Dave.

## Active Milestone

- **Current:** `M1 — CPU functional` (ready to begin locally on Windows using [`docs/ORCHESTRATOR-HANDOFF.md`](./docs/ORCHESTRATOR-HANDOFF.md))
- **Bespoke Vulkan Compute LLE RDP Name:** **`EdgeWalker`** (zero third-party RDP forks/FFI)
- **Debugger & UI Standard:** [`docs/13-debugger-and-ui-architecture.md`](./docs/13-debugger-and-ui-architecture.md) (Departure Mono + `btop`-inspired Machine Monitor with live CPU/Memory/GPU/Audio graphs matching `graycart-gb` & `graycart-gba`)
- **Pre-code gate:** Accepted by Dave on 2026-11-27

## Milestone Ledger (M0–M12)

| Milestone | Status | Gate Receipt Summary |
|---|---|---|
| **M0 — Scaffold** | **Done** (PR #2 merged, CI green 3/3 OS) | 21 passed (15 unit, 3 CLI smoke, 3 ROM harness), 0 failed, 3 ignored matrix stubs compiled; fmt/clippy `-D warnings` clean; macOS/Ubuntu/Windows CI green; zero forbidden binaries tracked |
| **M1 — CPU functional** | **In Progress (`feat/m1-cpu` — M1.0 + Wave A done)** | 66 passed (59 unit, 3 CLI smoke, 4 ROM harness), 0 failed, 5 ignored; Wave A 5/5 integer ISA families implemented by parallel `flash_lite` workers & reviewed; next: Wave B (COP0/Exceptions, TLB, COP1 FPU, Caches) |
| **M2 — Bus + boot** | Not started | — |
| **M3 — RSP scalar + DMA** | Not started | — |
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
