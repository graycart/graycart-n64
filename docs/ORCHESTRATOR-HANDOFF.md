<!--
Cited: docs/DEVELOPMENT-PLAN.md; docs/00-architecture-overview.md through docs/12-test-gates.md;
  STATUS.md; THIRD_PARTY.md; AGENTS.md
Note: Complete local-development handoff braindump & multi-subagent orchestration playbook
  prepared at the M0 -> M1 transition (2026-11-27).
-->

# Graycart N64 — Local Continuation & Multi-Subagent Orchestrator Handoff

This document is the complete **braindump + orchestration playbook** for pulling `graycart-n64` locally and driving Milestones **M1–M12** with a high-reasoning **Orchestrator** agent coordinating multiple **cheaper/faster worker subagents** in parallel.

---

## 1. Ground-Truth Project State (Post-M0, 2026-11-27)

- **Branch / Status:** Milestone **M0 (Scaffold)** is complete, verified locally and on GitHub Actions (`macos-latest`, `ubuntu-latest`, `windows-latest` all green), and merged to `main`.
- **Next Active Milestone:** **M1 — VR4300 CPU Functional**.
- **Crate:** `graycart-n64` (`v0.0.1`, MIT, Rust 2021 edition, zero GUI dependencies, `serde` enabled from day one for whole-machine serialization).
- **Bespoke Vulkan Compute LLE RDP Name:** **`EdgeWalker`** (100% from scratch in Rust + our own compute shaders in M6; **zero** third-party emulator/RDP/RSP forks or C++ FFI plugins).
- **Two-Tier Accuracy Contract:**
  - **Tier 1 (Default / CI Gate — `N64Config::accurate()`):** Strictly hardware-faithful LLE passing every test cart and golden. Never compromised by hacks or enhancements.
  - **Tier 2 (Opt-In — `EnhancementConfig`, M11):** Layered cleanly on top of **`EdgeWalker`** and our deterministic core (upscaling, SSAA, widescreen, VI/CRT filters, overclocking, pak auto-swap, Oracle Lens, Live Frame Inspector).

### Current Codebase Inventory (`src/`, `tests/`, `scripts/`)

| Path | Role & Current State | Next Touch |
|---|---|---|
| [`src/lib.rs`](../src/lib.rs) | `N64Core`, `N64Config::accurate()`, `EnhancementConfig`, `RunOutcome`, serde round-trip | M1 (wire CPU + CountClock + interrupts) |
| [`src/trap.rs`](../src/trap.rs) | Structured `Trap` enum (`UnmappedFreeze`, `CachedMmioFreeze`, `Rcp64BitReadFreeze`, `MisalignedAccess`, `UnimplementedRegister`, `UnimplementedOpcode`, `UnimplementedCop0Reg`, `UnimplementedRdpCommand`) | M1+ (keep expanding, NEVER weaken) |
| [`src/bus/mod.rs`](../src/bus/mod.rs) | Full physical decode table (`docs/02` §3), freeze windows, cached-MMIO trap, 64-bit RCP read freeze, DMEM/IMEM 8 KiB mirrors, ISViewer routing | M2 (MMIO regs, PI/SI DMA, PIF HLE) |
| [`src/bus/rdram.rs`](../src/bus/rdram.rs) | 4/8 MiB RDRAM + **bit-packed 9th-bit side array** (`hidden_bits`), open-bank `>installed` reads return `0` | M5/M6 (RDP coverage/Z 9th-bit consumers) |
| [`src/bus/isviewer.rs`](../src/bus/isviewer.rs) | ISViewer 64 (`0x13FF_0000..=0x13FF_FFFF`) ring buffer + capture log | Ready for M1/M2 `n64-systemtest` & `libdragon` |
| [`src/schedule/mod.rs`](../src/schedule/mod.rs) | Single-threaded deterministic event-queue slice scheduler, exact `2/3` RCP rational accumulator (zero floats), `single_step_mode`, sync-aware wrap-safe `CountClock` | M1 (wire `CountClock` to COP0 `Count`/`Compare`), M2 (VI/PI/SI/AI events) |
| [`src/cart/mod.rs`](../src/cart/mod.rs) | `.z64`/`.v64`/`.n64` detection & big-endian canonicalization, 64-byte header parse, IEEE CRC32 CIC classification (`6101/6102/6103/6105/6106/7102` + `Unknown`) | M2 (CIC HLE checksums/seeds), M8 (saves) |
| [`src/cpu/mod.rs`](../src/cpu/mod.rs) | `CpuBackend` trait + `InterpCpu` shell (`step` traps `UnimplementedOpcode`) | **M1 (active target)** |
| [`src/rsp/mod.rs`](../src/rsp/mod.rs) | `RspBackend` trait + `InterpRsp` shell (4 KiB DMEM/IMEM) | M3 (SU + DMA), M4 (VU) |
| [`src/rdp/mod.rs`](../src/rdp/mod.rs) | `RdpBackend` trait (`submit` + explicit `RdpSyncKind::{Pipe, Tile, Load, Full}`), `CpuRdpLle` shell + **`EdgeWalker`** architectural seam | M5 (`CpuRdpLle`), M6 (`EdgeWalker`) |
| [`src/mi/`](../src/mi/mod.rs), `vi/`, `ai/`, `pi/`, `ri/`, `si/`, `pif/` | Peripheral shells trapping `UnimplementedRegister` | M2 (MI/PI/SI/RI/PIF boot + AI/VI skeletons) |
| [`src/main.rs`](../src/main.rs) | Zero-dep headless CLI (`--frames`, `--cycles`, `--hash-out`, `--isviewer-out`, `--soft-boot`, `--no-expansion-pak`, `--single-step`, `--version`, `--help`) | M1/M5 (add trace/dump flags as needed) |
| [`tests/cli_smoke.rs`](../tests/cli_smoke.rs) | Synthetic ROM CLI & ISViewer integration tests | Extend as CLI grows |
| [`tests/roms/harness.rs`](../tests/roms/harness.rs) & [`main.rs`](../tests/roms/main.rs) | Shared `run_rom`, `BootOpts`, `Outcome`, `parse_systemtest_log`, compiled `#[ignore]` matrix stubs | M1 (add Dillonb `r30` sentinel & per-category systemtest assertions) |
| [`scripts/fetch-suites.sh`](../scripts/fetch-suites.sh) | Non-interactive suite fetcher targeting gitignored `tests/fixtures/fetched/` | Use locally to pull Dillonb / systemtest / nemu64 |

---

## 2. Local Machine Quickstart (First 5 Minutes)

```bash
git clone https://github.com/graycart/graycart-n64.git   # or: git pull origin main
cd graycart-n64

# Linux audio/input/windowing dev libs (skip on macOS/Windows):
# sudo apt-get install -y libasound2-dev libudev-dev pkg-config libxkbcommon-dev libwayland-dev

# Verify clean baseline (must pass 100% ROM-free):
export RUST_MIN_STACK=16777216
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# Fetch M1/M2 conformance suites into gitignored tests/fixtures/fetched/:
bash scripts/fetch-suites.sh --suite dillonb
bash scripts/fetch-suites.sh --suite systemtest
bash scripts/fetch-suites.sh --suite nemu64

# Run ignored ROM conformance matrix locally (NEVER in default CI):
RUST_MIN_STACK=16777216 cargo test --test roms -- --ignored --nocapture
```

---

## 3. Orchestrator + Cheap-Subagent Operating Model

### 3.1 Role Split (Maximize Speed & Minimize Cost Without Sacrificing Accuracy)

| Role | Recommended Model Tier | Owns | Must NEVER Do |
|---|---|---|---|
| **Orchestrator** | High-reasoning flagship model | 1. Interface & type contracts (`mod.rs`, shared traits, dispatch skeletons)<br/>2. Writing tight worker specs (pseudocode + 5–20 edge vectors + disjoint file list)<br/>3. Dispatching parallel worker subagents on **disjoint files**<br/>4. Reviewing every worker diff for silent stubs, attribution headers, and license hygiene<br/>5. Running integration/ROM gates, diagnosing failures with **hypothesis-before-code**, updating `STATUS.md`, committing & PRs | Never accept "looks good" without running the exact gate command; never delegate open-ended "fix rendering / fix timing" debugging blindly to a cheap worker. |
| **Worker Subagents** | Default worker model (`Model: "grok-4.7-high"`) | 1. Single-family implementations with explicit hardware pseudocode & test vectors (e.g., `alu.rs`, `shift.rs`, `mult_div.rs`, `branch.rs`, `load_store.rs`, `cop1.rs`, `mi/mod.rs`, `pi/mod.rs`, RDP command decoders)<br/>2. Unit test suites in `src/<mod>/tests.rs`<br/>3. Disassemblers, fixture/manifest tooling, mechanical refactors | Never edit files outside their assigned list; never add silent `0`/NOP stubs; never touch `Cargo.toml` deps or `ref/`; never skip `cargo fmt` / `clippy` / `test`. |

### 3.2 Mandatory Worker Subagent Prompt Template (Copy-Paste Contract)

Every time the Orchestrator spawns a cheaper worker subagent, fill in this exact contract:

```markdown
## Task: <M1.X — Single Family Name>

### Standing Rules (Hard)
1. Read `AGENTS.md`, `ATTRIBUTION.md`, and `THIRD_PARTY.md` first.
2. **Trap, don't stub:** Any unimplemented encoding/offset MUST return `Err(Trap::...)`. Never NOP or return `0`.
3. **File-top attribution header** required on every touched `.rs` file citing primary spec (e.g., NEC VR4300 UM chapter/section or N64brew URL).
4. **Tests go in a dedicated `tests.rs` file** (`#[cfg(test)] mod tests;`), not inline in production files.
5. **License hygiene:** NEVER copy from GPL, MAME/Angrylion, or unlicensed trees.

### Allowed Files (STRICT — do not touch any other file)
- `src/cpu/<family>.rs` (create/edit)
- `src/cpu/<family>_tests.rs` or `src/cpu/tests/<family>.rs` (create/edit)

### Hardware Specification & Pseudocode
<Paste exact MIPS III / VR4300 / RCP bit-level pseudocode, sign-extension rules, exception conditions, and edge quirks here.>

### Mandatory Edge Test Vectors (5–20 cases)
1. <Input -> Exact expected output / exception / register state>
2. ...

### Gate & Receipt
Run and paste exact verbatim output into your final report:
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `RUST_MIN_STACK=16777216 cargo test <module_filter>`
```

### 3.3 Orchestrator Review Checklist (Before Accepting Any Worker Output)

1. **Scope check:** `git diff --stat` touches ONLY the worker's allowed files.
2. **Anti-stub check:** Grep worker diff for `_ => Ok(`, `return Ok(0)`, `// TODO`, `todo!()` masquerading as no-op. Unimplemented paths must return `Err(Trap::...)`.
3. **Attribution check:** Every new file has a top-of-file credit header (`//! Cited: ...`).
4. **License check:** No suspicious GPL/MAME identifiers or copied comments.
5. **Receipt check:** Orchestrator re-runs the full workspace gate (`cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && RUST_MIN_STACK=16777216 cargo test`) plus the relevant ignored ROM suite gate, and records exact counts in [`STATUS.md`](../STATUS.md).

---

## 4. Parallel Execution Blueprint by Milestone (M1–M12)

### Milestone M1 — VR4300 CPU Functional *(Next Up!)*

**Gate:** Dillonb `basic.z64` + `cpu` tests PASS (fetch-only); `n64-systemtest` & `nemu64-test` CPU, COP0, TLB categories at `Failed: 0`; COP1 category gated separately.

1. **M1.0 — Orchestrator Serial Prerequisite (do this BEFORE spawning workers):**
   - Expand `src/cpu/mod.rs` to hold complete `InterpCpu` state: `gpr: [u64; 32]`, `hi`, `lo`, `pc`, `next_pc` (delay-slot pipeline state: `BranchState` tracking delay slot + branch-likely nullify!), `ll_bit`, `ll_addr`, `cop0: Cop0`, `cop1: Cop1`, `tlb: Tlb`, `caches: Caches`.
   - Create `src/cpu/mmu.rs`: 32-bit KSEG0 (`PA = VA - 0x8000_0000`, `CacheAttr::Cached`) and KSEG1 (`PA = VA - 0xA000_0000`, `CacheAttr::Uncached`) fast path + TLB hook for KUSEG/KSSEG/KSEG3.
   - Create `src/cpu/decode.rs`: clean opcode/funct/rt decoder dispatching into disjoint family functions (`alu::exec_*`, `shift::exec_*`, `mult_div::exec_*`, `branch::exec_*`, `load_store::exec_*`, `cop0::exec_*`, `cop1::exec_*`), defaulting unknown encodings to `Err(Trap::UnimplementedOpcode { pc, word })`.
   - Wire `tests/roms/harness.rs` to also support Dillonb's `r30` pass/fail sentinel (`r30 == u64::MAX` / `-1i64` pass, `r30 > 0` fail ID) alongside ISViewer log parsing.

2. **M1 Parallel Wave A — 5 Concurrent Cheap Worker Subagents (100% Disjoint Files):**
   - **Worker A1 (`src/cpu/alu.rs`):** `ADD`, `ADDI`, `ADDU`, `ADDIU`, `SUB`, `SUBU`, `DADD`, `DADDI`, `DADDU`, `DADDIU`, `DSUB`, `DSUBU`, `SLT`, `SLTI`, `SLTU`, `SLTIU`, `AND`, `ANDI`, `OR`, `ORI`, `XOR`, `XORI`, `NOR`, `LUI`.
     - *Critical edge vectors:* 32-bit ops sign-extend result from bit 31 into 64-bit GPR; `ADD`/`ADDI`/`SUB`/`DADD`/`DADDI`/`DSUB` raise `ArithmeticOverflow` without writing destination; `ADDU`/`SUBU`/`DADDU`/`DSUBU` wrap silently; writes to `r0` are discarded.
   - **Worker A2 (`src/cpu/shift.rs`):** `SLL`, `SRL`, `SRA`, `SLLV`, `SRLV`, `SRAV`, `DSLL`, `DSRL`, `DSRA`, `DSLL32`, `DSRL32`, `DSRA32`, `DSLLV`, `DSRLV`, `DSRAV`.
     - *Critical edge vectors:* 32-bit shifts take `sa` (5 bits) and sign-extend bit 31 to 64 bits (including `SLL r, r, 0` sign-extending!); `DSLL32`/`DSRL32`/`DSRA32` shift by `sa + 32`; variable shifts mask shift amount to 5 bits (32-bit) or 6 bits (64-bit).
   - **Worker A3 (`src/cpu/mult_div.rs`):** `MULT`, `MULTU`, `DIV`, `DIVU`, `DMULT`, `DMULTU`, `DDIV`, `DDIVU`, `MFHI`, `MFLO`, `MTHI`, `MTLO`.
     - *Critical edge vectors:* `DIV` / `DIVU` / `DDIV` / `DDIVU` by zero NEVER panic or raise exception on VR4300 — they write deterministic hardware `LO`/`HI` values (verify exact NEC VR4300 UM / ares vectors before writing!); `i32::MIN / -1` and `i64::MIN / -1` overflow without trapping; 32-bit results sign-extend into 64-bit `HI`/`LO`.
   - **Worker A4 (`src/cpu/branch.rs`):** `J`, `JAL`, `JR`, `JALR`, `BEQ`, `BNE`, `BLEZ`, `BGTZ`, `BLTZ`, `BGEZ`, `BLTZAL`, `BGEZAL`, plus all 8 Likely variants (`BEQL`, `BNEL`, `BLEZL`, `BGTZL`, `BLTZL`, `BGEZL`, `BLTZALL`, `BGEZALL`).
     - *Critical edge vectors:* Branch delay slot executes before target; `Likely` branches nullify delay slot when NOT taken; `BLTZAL`/`BGEZAL` write link register `r31` even when branch is not taken (and link before evaluating `rs` if `rs == 31`!); `J`/`JAL` take upper bits of the **delay slot PC** (`pc + 4`).
   - **Worker A5 (`src/cpu/load_store.rs`):** `LB`, `LBU`, `LH`, `LHU`, `LW`, `LWU`, `LD`, `SB`, `SH`, `SW`, `SD`, `LWL`, `LWR`, `LDL`, `LDR`, `SWL`, `SWR`, `SDL`, `SDR`, `LL`, `SC`, `LLD`, `SCD`, `SYNC`.
     - *Critical edge vectors:* Big-endian byte offsets for `LWL`/`LWR`/`SWL`/`SWR` and 64-bit `LDL`/`LDR`/`SDL`/`SDR`; `LW` sign-extends to 64 bits while `LWU` zero-extends; misaligned `LH`/`LW`/`LD`/`SH`/`SW`/`SD` raise `AdEL`/`AdES` with `BadVAddr` set; `LL` sets `ll_bit` + physical `ll_addr`, `SC` writes `1` or `0` to `rt` and clears `ll_bit` on exception/`ERET`.
   - **Orchestrator Gate after Wave A:** Merge A1–A5, run `cargo test` + Dillonb `basic.z64`!

3. **M1 Parallel Wave B — 4 Disjoint Subsystems (Workers after Orchestrator defines `Exception` enum):**
   - **Worker B1 (`src/cpu/cop0.rs` + `src/cpu/exceptions.rs`):** All COP0 regs (`Index`, `Random`, `EntryLo0/1`, `Context`, `PageMask`, `Wired`, `BadVAddr`, `Count`, `EntryHi`, `Compare`, `Status`, `Cause`, `EPC`, `PRId`=`0x0000_0B22`, `Config`, `LLAddr`, `WatchLo/Hi`, `XContext`, `ParityError`, `CacheError`, `TagLo/Hi`, `ErrorEPC`), `MFC0`/`DMFC0`/`MTC0`/`DMTC0`/`ERET`/`SYSCALL`/`BREAK`/`TGE`/`TLT`/`TEQ`/`TNE`, `Count`/`Compare` wired directly to `schedule::CountClock`, delay-slot `Cause.BD` + `EPC = pc - 4`, vector bases (`BEV=0` vs `BEV=1`, TLB refill vs general exception).
   - **Worker B2 (`src/cpu/tlb.rs`):** 32-entry fully associative joint TLB, `TLBR`/`TLBWI`/`TLBWR`/`TLBP`, variable `PageMask` (4 KiB–16 MiB), ASID + Global bit (`G = lo0.G && lo1.G`), odd/even page selection, Refill vs Invalid vs Mod exceptions, `Context`/`XContext`/`BadVAddr`/`EntryHi` population on miss.
   - **Worker B3 (`src/cpu/cop1.rs`):** VR4300 COP1 FPU (`FR=0` 32-bit paired regs & `FR=1` 64-bit regs, `LWC1`/`LDC1`/`SWC1`/`SDC1`, `MFC1`/`DMFC1`/`MTC1`/`DMTC1`/`CFC1`/`CTC1`, `ADD`/`SUB`/`MUL`/`DIV`/`SQRT`/`ABS`/`MOV`/`NEG`/`ROUND`/`TRUNC`/`CEIL`/`FLOOR`/`CVT`/`C.<cond>` in `.S`/`.D`/`.W`/`.L`, `BC1F`/`BC1T`/`BC1FL`/`BC1TL`, FCR31 rounding modes + exception cause/flag/enable bits, `CpU` exception when `Status.CU1 == 0`).
   - **Worker B4 (`src/cpu/cache.rs`):** Functional 16 KiB I-cache (32 B lines, 512 lines) and 8 KiB write-back D-cache (16 B lines, 512 lines) + `CACHE` instruction opcodes (Index Invalidate, Index Load/Store Tag, Hit Invalidate, Hit Writeback, Fill, Create Dirty Exclusive) — DMA intentionally does NOT snoop D-cache!

### Milestone M2 — Bus + Boot *(High Parallelism)*
- **Orchestrator:** Wire `Scheduler` event dispatch loop in `N64Core` (VI line, PI DMA complete, SI DMA complete, AI DMA step, SP DMA complete) + MI interrupt line aggregation (`MI_INTR & MI_INTR_MASK` → CPU `Cause.IP2`).
- **Parallel Workers:**
  - **M2-W1 (`src/mi/mod.rs` + `src/ri/mod.rs`):** Full MI mode/version/intr/mask regs + RI mode/config/current_load/select/refresh regs (`RI_SELECT` non-zero trap for `n64-systemtest`!).
  - **M2-W2 (`src/pi/mod.rs`):** PI regs, BSD domain 1/2 timing regs, **immediate-data / delayed-MI-interrupt** PI DMA (`ares` v129 pattern), PI status busy/IO bits, 16-bit halfword alignment quirk.
  - **M2-W3 (`src/si/mod.rs` + `src/pif/mod.rs`):** SI 64-byte PIF RAM DMA, boot-critical CIC HLE seeds (6101/6102/6103/6105/6106), PIF terminate-boot command (`0x08` / `0x10` / `0x20` bits), real **libdragon open-IPL3** execution path (verifying all 3 systemtest boot traps: `RI_SELECT`, SP DMA >8 MiB → 0, SP DMA IMEM 4 KiB wrap).
  - **M2-W4 (`src/vi/mod.rs` + `src/ai/mod.rs`):** Register-driven VI field period (`VI_V_SYNC` / `VI_V_INTR` / `VI_V_CURRENT` — never hard-coded 60 Hz!) + AI register/DMA event skeleton in scheduler.

### Milestones M3–M4 — RSP Scalar + Vector Unit
- **M3 (2 workers):** `src/rsp/dma.rs` (SP DMA double-buffered + IMEM 4 KiB wrap + >8 MiB reads returning 0 + SP status/halt/broke/MI IRQ) and `src/rsp/su.rs` (cut-down MIPS scalar interpreter + COP0 SP/DP reg forwarding).
- **M4 (Orchestrator-led specs + focused VU family workers):** Split VU into `src/rsp/vu/load_store.rs` (LBV..LTV/SBV..STV with 16-byte wrap & element broadcast), `src/rsp/vu/arith.rs` (VADD/VSUB/VADDC/VSUBC/VABS/VAND/VOR/VXOR/VSAR), `src/rsp/vu/mul.rs` (VMULF/VMULU/VMUDL/VMUDM/VMUDN/VMUDH/VMACF/VMACU/VMADL/VMADM/VMADN/VMADH — **exact 48-bit accumulator carry & clamping asymmetry vectors required in every worker spec!**), `src/rsp/vu/clip.rs` (VCL/VCH/VCR/VMRG/VLT/VEQ/VNE/VGE), `src/rsp/vu/rcp.rs` (VRCP/VRCPL/VRCPH/VRSQ/VRSQL/VRSQH lookup tables). Gate strictly on `n64-systemtest` RSP + `vmulf`/`vmulu` stress tests.

### Milestone M5 — Bespoke Software LLE RDP (`CpuRdpLle`) + VI
- Built slice-by-slice in `src/rdp/lle/` (`decode.rs`, `tmem.rs`, `edge.rs`, `combiner.rs`, `blender.rs`, `vi_filter.rs`):
  1. `FILL` + `COPY` + scissor + `SyncFull` (boots libdragon console/2D).
  2. Shaded triangles + 1-cycle Color Combiner.
  3. TMEM (4 KiB, `LoadBlock`/`LoadTile`/`LoadTLUT`, DXT, odd/even swizzle, TLUT high half) + textured triangles/rects.
  4. Blender + 3-bit coverage + RDRAM **9th-bit** read/write + Z-buffer + deterministic per-span dither/noise seeding.
  5. 2-cycle mode + VI anti-alias/divot/gamma/dither scan-out (separate pre-VI-filter and post-VI-filter hash gates).

### Milestone M6 — **`EdgeWalker`** (Bespoke Vulkan Compute LLE RDP)
- **Zero third-party RDP code/FFI.** Pure Rust (`ash` or `wgpu` compute, behind opt-in `vulkan-rdp` feature so core stays zero-GUI) + our own compute shaders living in `src/rdp/edgewalker/`:
  - Mirrors our verified M5 `CpuRdpLle` pipeline on the GPU:
    1. Command stream ingestion & state snapshot ring (`SyncPipe`/`SyncTile`/`SyncLoad`/`SyncFull` barriers respected).
    2. Tile binning compute pass (primitive → screen-tile command lists).
    3. Span rasterization + TMEM + Color Combiner + Blender + 9th-bit coverage + Z compute pass writing directly into GPU-mirrored RDRAM + 9th-bit SSBOs (with synchronous readback on CPU/VI sync barriers).
    4. Bespoke GPU VI scan-out & presentation pass + persistent on-disk pipeline cache.
  - **Every slice gated on bit-exact 1× frame-hash parity (`EdgeWalker` hash == `CpuRdpLle` hash)** before moving to the next slice!

### Milestones M7–M12 Summary
- **M7 Audio:** AI double-buffered DMA + documented ares v129 queued-DMA length latching & 8 KiB boundary quirks + libdragon RSP audio mixer golden PCM hash.
- **M8 PIF/Saves/Transfer Pak:** EEPROM 4k/16k, SRAM, FlashRAM FSM, Controller Pak, **4-port Transfer Pak** (with future `graycart-gb` save bridge), Joybus timing, 6105 challenge, Rumble↔MemPak hot-swap (`0x00` listener + hotkey).
- **M9 Commercial Smoke:** Local-only named corpus (1× 6102, 1× 6103, 1× 6105) title-screen hashes.
- **M10 Timing Hardening:** `timing` → `cop0hazard` → `cycle` feature ROMs, PI BSD domain timing, `240p Test Suite` VI checks.
- **M11 Modern Enhancements & Player Experience Tier (Opt-In via `EnhancementConfig`):**
  - `EdgeWalker` 2×/4×/8× LLE internal upscaling + ordered SSAA downsampling
  - Three-tier Widescreen (16:9 / 21:9): anamorphic + auto-letterbox crop, opt-in extended-frustum geometry widescreen with unstretched HUD `TEXRECT` anchoring, `.bps`/`.ips`/multi-line GameShark patch engine
  - Granular VI controls (de-blur, divot, dedither, gamma, **True 1:1 raw pixel scanout**), per-primitive 3D texture vs. 2D sprite filtering, Force LOD 0, built-in band-limited NTSC Composite & Aperture-Grille CRT shader, persistent pipeline cache
  - Voluntary VI-locked VR4300/RCP overclocking (1.5×/2×/3×)
  - Auto-swap Rumble↔MemPak, 4-port Transfer Pak, modern dual-stick FPS presets + mouse-look + multi-bind triggers + true N64 ~80-unit octagonal stick calibration
  - Deterministic ring-buffer Rewind + 1-frame Run-Ahead
  - **Oracle Lens** (live split-screen / pixel-diff between `EdgeWalker` vs. `CpuRdpLle` or Native 1× vs. Enhanced 4× Widescreen) & **Live RDP Frame Inspector** (draw-call scrubber, live TMEM viewer, 9th-bit coverage visualizer, Z heatmap, combiner equation decoder)
- **M12 Optional Performance Backends & Family API:** Cached interpreter / dynarec (only if profiling demands) + `GraycartCore` family API extract & umbrella submodule pin.

---

## 5. High-Risk Gotchas Ledger (Read Before Writing M1–M6 Specs)

1. **Three `n64-systemtest` / libdragon open-IPL3 boot traps (§3.8):**
   - `RI_SELECT` (`0x0470_000C`) must be non-zero or IPL3 tries full RDRAM controller init.
   - SP DMA reading RDRAM above 8 MiB MUST return `0` (already guaranteed by `Rdram::read_u*` open-bank bounds!).
   - SP DMA reading/writing IMEM longer than 4 KiB MUST wrap inside 4 KiB IMEM.
2. **COP0 `Count`/`Compare` 91-second wrap bug (§3.3):** Already solved in [`src/schedule/mod.rs`](../src/schedule/mod.rs) `CountClock` — wire COP0 `MFC0`/`MTC0` directly to `CountClock`, never maintain a duplicate counter.
3. **PI DMA timing (§3.3):** Copy data immediately on trigger, keep `PI_STATUS` DMA-busy set until scheduled `EventKind::PiDmaComplete`, then clear busy and assert `MI_INTR_PI`.
4. **D-cache vs. DMA coherence:** Real N64 DMA does NOT snoop CPU D-cache. Games intentionally rely on stale cache lines vs. explicit `CACHE` writeback/invalidate.
5. **VU fixed-point precision:** F3DEX2 triangles render fine even when `VMADL` accumulator carry is off by 1 bit, while audio PCM hashes break. Always gate VU on `vmulf`/`vmulu` stress tests + `n64-systemtest` RSP category.
6. **RDP sync barriers & deterministic noise:** Never collapse `SyncPipe`/`SyncTile`/`SyncLoad` into `SyncFull`. Seed dither/noise deterministically per-span in both `CpuRdpLle` and `EdgeWalker` so parallel GPU workgroups match serial CPU hashes bit-for-bit.
7. **License landmines (`THIRD_PARTY.md`):** `Dillonb/n64-tests` has **no license** (fetch-only, never commit); `240p Test Suite` is **GPL-2.0+** (fetch-only); `Angrylion` is **MAME non-commercial** (compare outputs only, never port); `Gopher64`/`Mupen`/`Project64`/`RMG` are **GPL** (study-only in gitignored `ref/`).

---

## 6. First Prompt to Give Your Local Orchestrator Session

When you open this repo locally, paste this to bootstrap your Orchestrator session immediately:

> Read `AGENTS.md`, `STATUS.md`, `THIRD_PARTY.md`, `docs/DEVELOPMENT-PLAN.md`, and `docs/ORCHESTRATOR-HANDOFF.md`. You are the **Orchestrator** for `graycart-n64`. We are starting **Milestone M1 (VR4300 CPU functional)**. Follow §4 of `docs/ORCHESTRATOR-HANDOFF.md`: first build the M1.0 CPU state/MMU/decoder seam yourself, then dispatch parallel worker subagents (using cheaper models) on disjoint files for Wave A (`alu.rs`, `shift.rs`, `mult_div.rs`, `branch.rs`, `load_store.rs`) using the §3.2 task-spec template with full hardware pseudocode and edge vectors. Review every worker diff against our trap-don't-stub, attribution, and license rules, verify with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `RUST_MIN_STACK=16777216 cargo test`, and fetched `Dillonb/n64-tests`, and record exact receipts in `STATUS.md`.
