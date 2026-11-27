<!--
Cited: Graycart N64 Research Pack (docs 00–12, PHASES.md); N64 Emulation Deep Dive (2026-11-27)
Note: Ratified working development plan (v0.2, 2026-11-27) amending the September 2026
  research pack with Dave's ratified gate decisions: bespoke Vulkan compute LLE RDP
  ("EdgeWalker", zero third-party RDP forks/FFI) and Two-Tier Accuracy + M11 Enhancement roadmap.
-->

# Graycart N64: Development Plan

**Status:** Ratified by Dave on 2026-11-27. Pre-code gate passed; Milestone M0 underway.
**Inputs:** the repo research pack (`docs/00`–`12` + `PHASES.md`) and the Nov 27, 2026 N64 emulation deep dive.
**Workflow:** Dave directs and reviews. Sylvie plans and keeps the ledger. Antigravity implements, in the task format of §5.

---

## 1. What we are building

A library-first Nintendo 64 core in Rust for the Graycart family:

- **Tier 1 (Default / CI Gate — Strictly Hardware-Faithful LLE):** VR4300 (MIPS III) interpreter first, LLE RSP (`InterpRsp`), bespoke LLE RDP (`CpuRdpLle` reference oracle + **`EdgeWalker`** bespoke Vulkan compute LLE RDP), deterministic event-queue scheduler and bus with RDRAM's 9th-bit side array, HLE boot + real open-IPL3 execution, and zero PIF/CIC/commercial ROM dumps in the repo ever. Accuracy is proven by named test suites at named gates, never by "it boots a game."
- **Tier 2 (Opt-In Enhancement Layer — M11):** Built cleanly on top of **`EdgeWalker`** and our deterministic core behind explicit `EnhancementConfig` flags — LLE 2×/4×/8× upscaling + SSAA, three-tier widescreen (16:9 / 21:9) with unstretched HUD anchoring, granular VI/de-blur/true 1:1 pixel scanout/built-in NTSC+CRT filters, voluntary VI-locked CPU/RCP overclocking, multi-line patch/GameShark loader, Rumble↔Controller Pak auto-swap, 4-port Transfer Pak (with future `graycart-gb` save bridge), modern dual-stick/mouse-look/octagonal stick calibration, deterministic Rewind + 1-frame Run-Ahead, **Oracle Lens** live diff mode, and **Live RDP Frame Inspector**.

## 2. Verdict on the research pack

The pack's gate discipline stands: phases exit on named oracles, doc 12 freezes RSP work until the CPU/bus test plumbing is merged, HLE is never passed off as LLE, and TBDs are marked instead of invented. The architecture (deterministic core, bus as the only memory authority, backend traits for CPU/RSP/RDP) stands. §3 records the amendments ratified on 2026-11-27.

## 3. Ratified amendments

- **3.1 Dual-gate suites & strict vendoring:** Gate on both frozen `lemmy-64/n64-systemtest` (MIT) and pinned `thelemmy/nemu64-test` (MIT). `Dillonb/n64-tests` has **no license** (fetch-and-run only, NEVER commit/mirror). `PeterLemon/N64` is Unlicense (curated hashed subset only, excluding bundled bootcode). `240p Test Suite` is GPL-2.0+ (fetch-only VI gate). Thresholds are per-category `Failed: 0`; never re-bless goldens without an external reference.
- **3.2 Boot moves to the front:** Boot-critical CIC HLE (6101/6102/6103/6105/6106), PIF RAM setup, minimal SI, terminate-boot, and AI register/DMA event skeleton land in M2 (Bus + Boot). Full Joybus, saves, 4-port Transfer Pak, and 6105 runtime challenge land in M8.
- **3.3 Scheduler ratified:** Single-threaded, deterministic **event-queue slice scheduler with a master CPU-cycle counter** and exact `2/3` rational RCP accumulator (no floating point). PI DMA copies data immediately and delays only the MI interrupt (ares v129 pattern). `MFC0 Count` includes cycles elapsed since the last sync and `Compare` scheduling is 32-bit wrap-safe (unit-tested in M0 before M1 CPU). `single_step_mode` shrinks slices to 1 instruction for debugging.
- **3.4 Timing vs. cycle hardening:** Count/Compare, VI field period, DMA completion interrupts, and RSP/DP sync land from the start. Pipeline/contention fidelity defers to M10 (`timing` → `cop0hazard` → `cycle` feature ROMs).
- **3.5 RDRAM 9th bit in M0/bus:** Every RDRAM byte's 9th bit (RDP coverage/Z metadata) lives in a bit-packed side array from day one.
- **3.6 Bespoke GPU LLE RDP — `EdgeWalker` (Dave's Nov 27 decision):** Zero third-party emulator/RDP forks or C++ FFI (`paraLLEl-RDP` is study/output-compare only, NEVER linked). Both RDP backends behind `RdpBackend` are 100% bespoke Graycart code:
  - **`CpuRdpLle` (M5):** Pure-Rust software LLE RDP serving as our deterministic reference oracle and headless CI renderer.
  - **`EdgeWalker` (M6 → M11):** Our own **bespoke Vulkan compute LLE RDP** (pure Rust + our own compute shaders: tile binning → span rasterization → TMEM/combiner/blender/coverage/9th-bit/Z → RDRAM writeback) + bespoke GPU VI presentation, ported slice-by-slice against `CpuRdpLle` and gated on bit-exact 1× frame-hash parity before layering M11 enhancements (upscaling, SSAA, widescreen, CRT, Oracle Lens, Live Frame Inspector).
  - VR4300 and RSP interpreters stay on the host CPU.
- **3.7 Mechanical license boundary (`THIRD_PARTY.md`):** Ships in M0 with Study / Link / Port columns and gitignored `ref/` + `tests/fixtures/fetched/`.
- **3.8 CIC and open IPL3:** Support both HLE soft-boot and real libdragon open-IPL3 execution (larger than 4 KiB, TOC past `0x1000`, ELF loader, RDRAM clear + cache reset) plus the three documented systemtest boot traps (`RI_SELECT` non-zero skips RDRAM init, SP DMA reads >8 MiB return 0, SP DMA IMEM wraps at 4 KiB).
- **3.9 Audio doc 05 correction:** Replace the unverified "AI delayed-carry bug" label with the distinct, verifiable hardware quirks: ares v129 queued-DMA length latching bug, AI 8 KiB (`0x2000`) boundary DMA address increment quirk (libdragon/n64brew), and RSP `VADDC`/accumulator carry semantics.

## 4. Working Milestone Sequence (M0–M12)

| Milestone | Scope | Gate (Receipt Required) |
|---|---|---|
| **M0 Scaffold** | Crate + module skeleton; bus + 9th-bit RDRAM + freeze traps; scheduler + Count wrap test; ROM loader (z64/v64/n64 + CIC); headless runner + ISViewer; `N64Config::accurate()` vs. `EnhancementConfig`; `THIRD_PARTY.md`; `STATUS.md`; CI + suite plumbing | `cargo fmt/clippy/test` green, ROM-free; runner boots stub + captures ISViewer; suite plumbing merged |
| **M1 CPU functional** | VR4300 integer + COP0 + exceptions → TLB → COP1 (FPU) → functional caches (`CACHE` semantics); scheduler active | Dillonb basic PASS (fetch-only); systemtest & nemu64-test CPU/COP0/TLB `Failed: 0`; COP1 gated separately |
| **M2 Bus + boot** | Full physical map; PI DMA (immediate data, delayed MI IRQ); boot-critical CIC/PIF/SI; real open-IPL3 execution + HLE boot; AI event skeleton | Suites boot end-to-end and report full counts; all 3 IPL3 boot traps unit-tested |
| **M3 RSP scalar + DMA** | SU interpreter; SP DMA (4 KiB IMEM wrap, >8 MiB reads return 0); SP IRQ | Dillonb RSP + systemtest SP categories PASS |
| **M4 RSP vector unit** | All VU ops (48-bit accum, VCO/VCC/VCE, exact clamp/carry) | Systemtest RSP + `vmulf`/`vmulu` stress tests PASS; libdragon ucode emits DL |
| **M5 Software RDP (`CpuRdpLle`) + VI** | Bespoke pure-Rust LLE RDP in slices (FILL/COPY → triangles/1-cycle → TMEM → blender/coverage/Z → 2-cycle); VI scan-out with separate pre/post-filter hash gates | PeterLemon RDP demos + libdragon examples frame-hash match |
| **M6 Bespoke Vulkan Compute LLE RDP (`EdgeWalker`)** | From-scratch Rust + Vulkan compute LLE RDP (zero third-party forks/FFI) + GPU VI presentation; sync test mode preserved | `EdgeWalker` bit-exact 1× frame-hash parity with `CpuRdpLle` & external goldens across every slice; interactive homebrew at full speed |
| **M7 Audio** | AI double-buffered DMA (with documented queued-length & 8 KiB quirks) + RSP audio mixer | Golden PCM hash match |
| **M8 PIF/saves complete** | EEPROM/SRAM/Flash/Controller Pak byte-exact round-trips; 4-port Transfer Pak; Rumble↔MemPak hot-swap; Joybus timing; 6105 challenge | Byte-exact round-trip per backend; multi-port Transfer Pak test |
| **M9 Commercial smoke** | Named local-only corpus (1× 6102, 1× 6103, 1× 6105) boots to title screen | Title-screen frame hash recorded locally (not CI-gated) |
| **M10 Timing hardening** | Optional feature ROMs in order: `timing` → `cop0hazard` → `cycle`; PI BSD domain timing; 240p Suite VI checks | Recorded thresholds per feature ROM |
| **M11 Modern Enhancements & Player Experience Tier** | Opt-in on top of `EdgeWalker` + core: 2×/4×/8× LLE upscaling + SSAA, 3-tier widescreen (16:9/21:9) + HUD anchoring, granular VI de-blur / true 1:1 scanout / built-in NTSC+CRT + persistent pipeline cache, voluntary VI-locked CPU/RCP overclock, `.bps`/`.ips`/GameShark patch engine, Rumble↔MemPak auto-swap, 4-port Transfer Pak + `graycart-gb` bridge, dual-stick/mouse-look/octagonal stick calibration, Rewind + 1-frame Run-Ahead, **Oracle Lens** live diff & **Live RDP Frame Inspector** | Tier 1 accuracy gates remain 100% untouched & green; each enhancement verified with opt-in tests |
| **M12 Optional performance & Family API** | Cached interpreter / dynarec (only if profiling demands) + `GraycartCore` family API extract & umbrella pin | Interpreter remains permanent CI accuracy reference |

## 5. Antigravity working agreement

Enforced verbatim in root [`AGENTS.md`](../AGENTS.md):
1. Trap, don't stub.
2. Done means a receipt (recorded in [`STATUS.md`](../STATUS.md)).
3. One family per task with hardware pseudocode, edge vectors, fail→pass gate, no regressions, and explicit file scope.
4. Hypothesis before code on debugging tasks.
5. License hygiene is mechanical (`THIRD_PARTY.md` + gitignored `ref/` + file-top attribution headers).
6. Maintain the `STATUS.md` accuracy & ruled-out ledger every session.
7. Human gates stay human (Dave reviews milestone receipts; never re-bless goldens without an external oracle).

## 6. Ratified gate decisions (2026-11-27)

- [x] **Working plan adopted:** §3 amendments + M0–M12 milestone sequence accepted; pre-code gate lifted.
- [x] **GPU posture decided:** Bespoke Vulkan compute LLE RDP (**`EdgeWalker`**) built from scratch behind `RdpBackend` + `CpuRdpLle` reference oracle; zero third-party RDP forks or C++ FFI.
- [x] **Enhancement posture decided:** Two-Tier Accuracy Contract — faithful hardware accuracy & test-cart compliance by default, opt-in M11 enhancement tier (`EnhancementConfig`) cleanly layered on top.
- [x] **Scheduler ratified:** Single-threaded deterministic event-queue slice scheduler (§3.3).
- [x] **Test & license policy ratified:** Dual-gate suites, fetch-only rules for Dillonb/240p, `THIRD_PARTY.md` (§3.1 / §3.7).
- [x] **Doc 05 audio correction:** Applied in [`05-audio.md`](./05-audio.md).
