# Graycart N64 — Accuracy & Ruled-Out Ledger (`STATUS.md`)

Loaded into every implementation session per [`AGENTS.md`](./AGENTS.md) and [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) §5.6.
Milestones advance only on pasted, verifiable test receipts reviewed by Dave.

## Active Milestone

- **Current:** `M7 — Audio (AI + RSP Mixer)` **PASS — Receipts ready alongside M6 Wave 2 (`feat/m6-edgewalker`, uncommitted atop `6f52687`, awaiting Dave's review)** (zero M6/M7 commit yet). M7 adds 2-slot double-buffered AI DMA (`src/ai/dma.rs`) with IRQ-on-start, live 8-byte `AI_LEN` countdown, zero-float NTSC/PAL/MPAL rational pacing, and documented Quirk #1 (queued length isolation) + Quirk #2 (8 KiB `0x2000` boundary delayed-carry), a deterministic PCM ring buffer + zero-float 32.32 fixed-point resampler + WAV encoder (`src/ai/pcm.rs`), bespoke LLE RSP stereo Q15 mixer & `VADDC`/`VADD` phase-accumulator microcodes + bit-exact DSP oracles (`src/ai/rsp_mixer.rs`), exhaustive VU audio stress tests (`src/rsp/vu_audio_tests.rs`), and full `N64Core` / `Bus` wiring (`drain_audio_i16`, `drain_audio_resampled_i16`, `audio_pcm_hash`, `audio_telemetry`). Verified at **274 passed** default (`267` lib + `3` CLI + `4` roms; `5` ignored), **31/31** live RTX 4090 `vulkan-rdp` EdgeWalker tests, **26/26** Dillonb, **7/8** local carts, **0** clippy warnings, and **CLEAN** git hygiene. **Awaiting Dave's review before Done / commit.**
- **Branch / HEAD:** `feat/m6-edgewalker` tracking `origin/feat/m6-edgewalker` @ base `6f52687` (squash of PR #3 onto `main`). M6 GPU wave is **uncommitted** dirty work on top.
- **Bespoke Vulkan Compute LLE RDP Name:** **`EdgeWalker`** (zero third-party RDP forks/FFI; direct deps `wgpu` 24.0.5 + `pollster` 0.4 behind `vulkan-rdp` only).
- **Debugger & UI Standard:** [`docs/13-debugger-and-ui-architecture.md`](./docs/13-debugger-and-ui-architecture.md) (Departure Mono + `btop`-inspired Machine Monitor with live CPU/Memory/GPU/Audio graphs matching `graycart-gb` & `graycart-gba`)
- **Pre-code gate:** Accepted by Dave on 2026-11-27

## Session Handoff Snapshot (For Fresh Agent Sessions)

- **Workspace & OS:** Windows (PowerShell 5.1; use `; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }` instead of `&&`). `graycart-n64` is a git submodule under the parent `graycart` repo (`.git` points to `../.git/modules/graycart-n64`). Always set `$env:RUST_MIN_STACK = "16777216"` for `cargo test`.
- **Governance & Execution Model:** Follow [`AGENTS.md`](./AGENTS.md) strictly. The top-level agent acts strictly as **Orchestrator ("the boss")**: design thin interface seams, fan out implementation across **maximally parallel `grok-4.7-high` subagents** on strictly disjoint files, audit every worker diff line-by-line against hardware specs before merging, run all verification gates, update `STATUS.md`, and report a **Progress Board + runnable PowerShell commands** after every milestone.
- **Commits on `feat/m1-cpu` (since `main` `28a507a`):**
  1. `e47f185` — M1.0 seam + Wave A VR4300 integer ISA (5 parallel `flash_lite` workers)
  2. `c7ba46b` — M1.B0 seam + Wave B COP0/Exceptions, 32-entry TLB, COP1 FPU, L1 Caches (4 parallel `flash_lite` workers)
  3. `c7ffeaa` — M2 MI/RI/PI/SI/PIF-HLE/VI/AI peripherals, DMA/IRQ scheduler, VR4300 `SRA`/`SRAV` 64-bit hardware quirk fix, **26/26 Dillonb PASS** (4 parallel `flash_lite` workers)
  4. `c3b5b15` — M3 RSP Scalar Unit, SP MMIO + 2D skip/count DMA, exact `2/3` RCP scheduling, and upgraded [`AGENTS.md`](./AGENTS.md) parallel playbook (3 parallel `flash_lite` workers)
  5. `724663c` — M4 RSP Vector Unit (`LWC2`/`SWC2` + 8×8 transpose, 22 arith/MAC ops w/ 48-bit acc, 17 compare/clip/logic ops, `MFC2`/`MTC2`/`CFC2`/`CTC2`, zero-float integer 512-entry `RCP`/`RSQ` ROMs; 4 parallel `flash_lite` workers)
  6. `48b4cd3` — M0–M4 session handoff snapshot and M5 parallel kickoff blueprint
  7. `d89d417` — M5.0 Bus.rdp single-source-of-truth DPC/DPS seam + `SyncFull` → `MI_INTR_DP`.
  8. `92ba55e` — M5 W1–W4 `CpuRdpLle` software RDP + VI RGBA8888 scanout (**not pushed**). M6 `EdgeWalker` kickoff (W0–W2) is **uncommitted** on top of this commit (no M6 hash). Commit / push / squash / next-branch is **PARKED**.
- **Already-Fetched Local Fixtures (gitignored, never commit):**
  - `tests/fixtures/fetched/dillonb/` (26 standard `.z64` ROMs)
  - `tests/fixtures/fetched/dillonb-simpleboot/` (26 `*_simpleboot.z64` ROMs — **26/26 PASS in ~5s `--release`**)
  - `tests/fixtures/fetched/n64memtest/n64memtest.z64` (v0.9)
  - Local commercial smoke ROMs live outside the submodule in `..\carts\n64` (**8 `.z64` carts**, local-only, NEVER commit; `probe_local_carts` runs 2M cycles in ~0.4s `--release` with 7/8 running cleanly and Mario Kart 64 hitting clean `N64DD_REGS 0x0508` presence-probe trap).
- **M5.0 Bus.rdp seam final gate (`M5 — Bespoke Software LLE RDP CpuRdpLle + VI Scanout`):** worker model `grok-4.7-high`. RDP state lives only on `Bus.rdp` (orphaned throwaway `CpuRdpLle::new()` / `N64Core.rdp` removed). `RaiseDpIntr` → `MI_INTR_DP`. End-to-end `m5_0_bus_rdp_persistent_mmio_and_syncfull_mi_intr_dp` passing; **18** RDP unit tests passing. Final gate: **179 passed** = **172** library + **3** CLI smoke + **4** ROM harness; **0 failed**; **5 ignored** (default roms matrix stubs). Dillonb **26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT**. Local carts **7/8** clean at 2M cycles; Mario Kart 64 same clean `N64DD_REGS` presence-probe trap at offset `0x0508`. Hygiene clean (no forbidden binaries). Zero failures; zero Orchestrator code fixes during the final gate. `regs_tests`: offset `0x20` is the `DPC_START` mirror via `& 0x1F`; unknown-offset trap uses `0x02`. Seam receipt only (commit `d89d417`). Follow-on W1–W4 is committed as `92ba55e` (not pushed) and is **not** human-blessed Done.
  - Orchestrator M5.0 seam: `CpuRdpLle` on `Bus` (`pub rdp: CpuRdpLle`) as single source of truth for `0x0410_0000..=0x042F_FFFF` (DPC/DPS regs + XBUS DMEM / RDRAM ring-buffer DMA + `MI_INTR_DP` on `SyncFull`). Planned disjoint files:
    1. `src/rdp/regs.rs` + `regs_tests.rs` — DPC/DPS MMIO regs, 10-bit paired `DPC_STATUS`, command FIFO decoder & dispatch.
    2. `src/rdp/tmem.rs` + `tmem_tests.rs` — 4 KiB TMEM + `SetTile`/`SetTileSize`/`LoadTile`/`LoadBlock`/`LoadTlut` + texel sampler.
    3. `src/rdp/raster.rs` + `raster_tests.rs` — Edge-walker triangle rasterizer, `FillRectangle`, `TextureRectangle`/`Flip`, scissor clip, color/Z-buffer + 9th-bit coverage R/W.
    4. `src/rdp/combine_blend.rs` + `combine_blend_tests.rs` + VI RGBA8888 scanout — 1-cycle/2-cycle Color Combiner, Blender, `SyncFull` → `MI_INTR_DP`, and VI RGBA16/RGBA32 framebuffer scanout for golden hash/PNG verification (the software oracle for M6 bespoke Vulkan compute **`EdgeWalker`**).
- **M5 W1–W4 CpuRdpLle (commit `92ba55e`, not pushed):** worker model `grok-4.7-high`. W1 FIFO fetch + multi-word decoder + state setters, hooked to TMEM + `draw_primitive`. W2 4 KiB TMEM, 8 tiles, loads, sampler (**13** tests; clippy args bundled). W3 edge-walker triangles/rects/scissor/Z/coverage (**18** tests). W4 combiner/blender + VI RGBA8888 scanout (**15+9** tests). Final reconfirm: `cargo fmt --check` PASS; `cargo clippy --all-targets --all-features -- -D warnings` PASS; `cargo test` **241 passed; 0 failed; 5 ignored** (234 lib + 3 CLI + 4 roms); Dillonb **26/26**; local carts **7/8**; hygiene clean. Still trapping: `SetConvert`/`SetKey*`, COPY/AA and other exotic blender modes. M5 is **not** Done. Paths in `92ba55e`: `src/bus/mod.rs`, `src/rdp/{mod,regs,regs_tests,tmem,tmem_tests,raster,raster_tests,combine_blend,combine_blend_tests}.rs`, `src/vi/{mod,tests}.rs`.
- **M6 EdgeWalker GPU Wave 1 + Wave 2 (uncommitted on `feat/m6-edgewalker` @ base `6f52687`):** Worker model `flash_lite` (`edgewalker_worker`) across Wave 2 parallel disjoint shards (`M6-W2-BUS`, `M6-W2-TMEM`, `M6-W2-COMBINE`, `M6-W2-RASTER`), audited & integrated by Orchestrator. Full bespoke wgpu/Vulkan compute LLE (RTX 4090) with zero CPU fallback and zero WGSL integer `/` or `%`. `N64Config::rdp_backend` (`CpuLle` default, `EdgeWalker` opt-in) wired through `Bus::with_rdp_backend` / `pump_dpc_fifo`.
- **M7 Audio Wave (uncommitted on `feat/m6-edgewalker` atop M6):** 4 parallel `flash_lite` (`m7_audio_worker`) shards on disjoint files (`M7-W1-AI-DMA`, `M7-W2-PCM`, `M7-W3-RSP-MIXER`, `M7-W4-VU-AUDIO-STRESS`), audited line-by-line by Orchestrator and wired into `Ai`, `Bus`, and `N64Core`. Combined verification gate green: `fmt` 0; `clippy --all-targets --all-features -- -D warnings` 0; default `cargo test` **274 passed** (`267` lib + `3` CLI + `4` roms; `5` ignored); `vulkan-rdp` `rdp::edgewalker` **31 passed; 0 failed** live on RTX 4090; Dillonb **26/26**; local carts **7/8** (MK64 `N64DD_REGS 0x0508`); hygiene CLEAN. **Awaiting Dave's review — do not commit/push or mark Done without sign-off.** Full details in Receipts § M7 Audio & § M6 Wave 2.

## Milestone Ledger (M0–M12)

| Milestone | Status | Gate Receipt Summary |
|---|---|---|
| **M0 — Scaffold** | **Done** (PR #2 merged, CI green 3/3 OS) | 21 passed (15 unit, 3 CLI smoke, 3 ROM harness), 0 failed, 3 ignored matrix stubs compiled; fmt/clippy `-D warnings` clean; macOS/Ubuntu/Windows CI green; zero forbidden binaries tracked |
| **M1 — CPU functional** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | Full VR4300 ISA + COP0/Exceptions + 32-entry TLB + COP1 IEEE-754 FPU + L1 I/D Caches + VR4300 `SRA`/`SRAV` 64-bit hardware quirk; **26/26 Dillonb `n64-tests` PASS** (0 FAIL, 0 TRAP, 0 TIMEOUT) |
| **M2 — Bus + boot** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | MI/RI/PI/SI/PIF-HLE/VI/AI registers & scheduled DMA/IRQ wired; **26/26 Dillonb PASS**; **8/8 local commercial carts running 200k cycles with 0 traps** |
| **M3 — RSP scalar + DMA** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | SP MMIO + 2D skip/count DMA + full 32-bit RSP Scalar Unit + `BREAK`/`COP0` + exact 2/3 RCP scheduling; **26/26 Dillonb PASS**; **7/8 local commercial carts running 2,000,000 cycles cleanly** |
| **M4 — RSP vector unit** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | 160 passed (153 unit, 3 CLI smoke, 4 ROM harness), 0 failed, 5 ignored; full 32×128-bit VU (`LWC2`/`SWC2` + 8×8 transpose, 22 arith/MAC ops w/ 48-bit acc, 17 compare/clip/logic ops, `MFC2`/`MTC2`/`CFC2`/`CTC2`, zero-float integer `RCP`/`RSQ` 512-entry ROMs); **26/26 Dillonb PASS** |
| **M5 — Bespoke Software LLE RDP (`CpuRdpLle`) + VI** | Wave committed `92ba55e` on `feat/m1-cpu` (not pushed; not human-blessed Done; final CI reconfirm PASS) | W1 FIFO/decoder/setters hooked to TMEM + `draw_primitive`; W2 4 KiB TMEM + 8 tiles + sampler (**13** tests); W3 edge-walker (**18** tests); W4 combiner/blender + VI RGBA8888 (**15+9** tests). Final reconfirm: `cargo fmt --check` PASS; `cargo clippy --all-targets --all-features -- -D warnings` PASS; `cargo test` **241 passed; 0 failed; 5 ignored** (234 lib + 3 CLI + 4 roms); Dillonb **26/26**; carts **7/8**; hygiene clean. Still trapping: `SetConvert`/`SetKey*`, COPY/AA exotic blender modes. M5.0 seam commit remains `d89d417` (**179 passed**) |
| **M6 — Bespoke Vulkan Compute LLE RDP (`EdgeWalker`)** | **Wave 2 Full GPU Compute PASS — Receipts ready (`feat/m6-edgewalker`, uncommitted atop `6f52687`, awaiting Dave's review)** | Real wgpu `ComputeDevice` (Vulkan **NVIDIA GeForce RTX 4090 Laptop GPU**); persistent `EdgeWalkerGpu` (`#[serde(skip)]`) + 4 KiB VRAM TMEM + WGSL `tmem_load`/`tmem_sample`/`combine_blend`/`raster_core` (zero `/` or `%`, CAS atomics, bit-packed 9th-bit hidden buffer, 1-/2-cycle CC & blender, FillRect/TexRect+Flip/triangles/Z/dither/AA coverage); `N64Config::rdp_backend` (`CpuLle` default / `EdgeWalker` opt-in) wired into `Bus::pump_dpc_fifo`; `fmt` & `clippy --all-targets --all-features -D warnings` **PASS**; default `cargo test` **254 passed** (247+3+4; 5 ignored); `vulkan-rdp` `rdp::edgewalker` **31 passed; 0 failed** live on RTX 4090; Dillonb **26/26**; local carts **7/8**; hygiene **CLEAN** |
| **M7 — Audio (AI + RSP mixer)** | **Receipts ready (`feat/m6-edgewalker`, uncommitted atop M6, awaiting Dave's review)** | Double-buffered AI DMA w/ IRQ-on-start, live 8-byte `AI_LEN` countdown, zero-float NTSC/PAL/MPAL rational pacing, Quirk #1 length isolation + Quirk #2 8 KiB `0x2000` boundary delayed-carry; deterministic `AudioRingBuffer` + zero-float 32.32 `PcmResampler` + WAV encoder; bespoke LLE RSP stereo Q15 mixer + `VADDC`/`VADD` phase-accumulator microcodes & bit-exact oracles; 5 VU audio stress tests; `N64Core` audio API (`drain_audio_i16`, `drain_audio_resampled_i16`, `audio_pcm_hash`, `audio_telemetry`) & end-to-end RSP→AI golden PCM hash + delayed-carry tests. Gate: `fmt`/`clippy` **0**; default `cargo test` **274 passed** (267+3+4; 5 ignored); `vulkan-rdp` **31/31**; Dillonb **26/26**; carts **7/8**; hygiene **CLEAN** |
| **M8 — PIF/Joybus/Saves/4-port Transfer Pak** | Not started | — |
| **M9 — Commercial smoke (local-only)** | Not started | — |
| **M10 — Timing hardening (`timing`/`cop0hazard`/`cycle`)** | Not started | — |
| **M11 — Modern Enhancements & Player Experience Tier** | Not started | — |
| **M12 — Optional dynarec + Family API extract** | Not started | — |

## Receipts

### M7 — Audio (AI + RSP Mixer) (`feat/m6-edgewalker`, uncommitted atop M6, 2026-11-27)

- **Seam & 4 Parallel Workers (`flash_lite` / `m7_audio_worker` on strictly disjoint files, Orchestrator audited & wired):**
  1. **`M7-W1-AI-DMA` (`317dcd97-6989-4424-9544-c8838b0455ce`) — `src/ai/dma.rs`, `src/ai/dma_tests.rs`:** 2-slot double-buffered `AiDma` with zero-float `u128` rational NTSC/PAL/MPAL pacing (`next_burst_cycles`), IRQ-on-start (`0->1` and `2->1`), live 8-byte `AI_LEN` countdown & zero on drain, `AI_CONTROL` 0→1 resume, Quirk #1 (queued length isolation / 0-len immunity), Quirk #2 (8 KiB `0x2000` end-boundary delayed-carry without mid-transfer false triggers), and `serde` persistence (**4** tests).
  2. **`M7-W2-PCM` (`c7eed338-950b-47e6-81d7-057b43032ad2`) — `src/ai/pcm.rs`, `src/ai/pcm_tests.rs`:** `AudioRingBuffer` (`16_384` stereo frames default), peak/clipping/drop/underrun telemetry, running FNV-1a 64-bit `pcm_hash` invariant across capacity overflows, zero-float 32.32 fixed-point `PcmResampler` (chunked vs monolithic bit-exact), and `encode_wav_pcm16_stereo` (**5** tests).
  3. **`M7-W3-RSP-MIXER` (`a2e4f90f-bb5d-4139-aef9-05bfb1131937`) — `src/ai/rsp_mixer.rs`, `src/ai/rsp_mixer_tests.rs`:** Bespoke pure-integer LLE RSP stereo Q15 gain mixer microcode (`VMULF`/`VMACF`/`SSV` interleave + SP DMA writeback) & 16.16 `VADDC`/`VADD` phase-accumulator + 48-bit linear predictor microcode executed live on `InterpRsp` + `Bus`, verified bit-exact against `oracle_stereo_mix` & `oracle_phase_and_predictor` (with Orchestrator fixes for `i32` wave scaling, `BNE` branch offset, zero-extended `ORI` DMA address, and shared encoder helpers; **2** tests).
  4. **`M7-W4-VU-AUDIO-STRESS` (`21ec877a-a87f-4d7b-b754-0515405d0f47`) — `src/rsp/vu_audio_tests.rs`:** Exhaustive VU precision stress tests covering `VMULF`/`VMULU`/`VMACF`/`VMACU` positive & negative saturation, `VMUDL`/`VMACL` 48-bit accumulator carry + `clamp_acc_low` boundaries (`-32769`..`32768`), 8-lane `VADDC`/`VADD`/`VSUBC`/`VSUB` carry/borrow chains, `VSAW` accumulator readout, and unaligned DMEM-wrapping `LSV`/`SSV`/`LQV`/`SQV` interleave (with Orchestrator opcode & boundary fixes; **5** tests).
  5. **Integration (`src/ai/mod.rs`, `src/ai/tests.rs`, `src/bus/mod.rs`, `src/lib.rs`, `src/tests.rs`):** Composed `Ai` (`AiDma` + `AudioRingBuffer` + `PcmResampler`), `Bus::step_ai_burst` + `PhysRegion::AiRegs` IRQ-on-start routing, `N64Core` public audio API (`audio_sample_rate_hz`, `drain_audio_i16`, `drain_audio_resampled_i16`, `audio_pcm_hash`, `audio_telemetry`), end-to-end LLE RSP mixer → double-buffered AI DMA → golden PCM hash test (`m7_rsp_audio_mixer_to_ai_double_buffer_golden_pcm_hash`), and end-to-end 8 KiB delayed-carry test (`m7_ai_8k_boundary_delayed_carry_end_to_end`).
- **Orchestrator M7 Full Verification Gate (Windows PowerShell, `$env:RUST_MIN_STACK = "16777216"`):**
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 warnings)
  - `cargo test` (default, GPU-free) → lib **267 passed; 0 failed** (1.31s) + `cli_smoke` **3 passed; 0 failed** (0.19s) + `roms` **4 passed; 0 failed; 5 ignored** (**274 passed** total)
  - `cargo test --features vulkan-rdp rdp::edgewalker -- --nocapture` → **31 passed; 0 failed** live on **NVIDIA GeForce RTX 4090 Laptop GPU** (3.13s)
  - `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`** (4.74s)
  - `cargo test --release --test roms probe_local_carts -- --ignored --nocapture` → **7/8 RUNNING** @ 2,000,000 cycles (0.29s); Mario Kart 64 clean `TRAP: unimplemented N64DD_REGS register read at offset 0x0508`
  - Forbidden-binary git hygiene → **`HYGIENE=CLEAN`**

### M6 — `EdgeWalker` Wave 2 Full GPU Compute LLE (`feat/m6-edgewalker`, uncommitted atop `6f52687`, 2026-11-27)

- **Seam & Parallel Workers (`flash_lite` / `edgewalker_worker` on strictly disjoint files, Orchestrator audited & wired):**
  1. **`M6.W2.0` Seam (`gpu_types.rs`):** Locked LE packing structs: `GpuTileDescPack` (32B, 8 tiles = 256B), `TmemLoadParamsPack` (64B), `CombineStatePack` (64B), `RasterParamsPack` (288B = 72×32-bit words).
  2. **`M6-W2-BUS` (`85e02909-01b0-4f9d-9d51-bd4eef17a0cf`) — `src/bus/rdram.rs`, `src/rdp/mod.rs`, `src/lib.rs`, `src/bus/mod.rs`:** Exposed bit-packed 9th-bit slice accessors (`hidden_bits_as_slice` / `hidden_bits_as_mut_slice`), `RdpBackendKind` (`CpuLle` / `EdgeWalker`), `N64Config::rdp_backend` (`CpuLle` default), and `Bus::with_rdp_backend` + `pump_dpc_fifo` routing (`SyncFull` raises `MI_INTR_DP`).
  3. **`M6-W2-TMEM` (`5c4122e3-652e-4969-b5c9-4833f1e80cd5`) — `shaders/{tmem_load,tmem_sample}.wgsl`, `tmem_gpu.rs`, `tmem_gpu_tests.rs`:** GPU `LoadTile`/`LoadBlock`/`LoadTlut` + point texel sampler (all 9 N64 formats RGBA16/32, CI4/8, IA4/8/16, I4/8; shift/mask/mirror/clamp; odd-row `^= 4` swap; RGBA32 `+0x800` split; concrete `1i << m` Naga-clean shift).
  4. **`M6-W2-COMBINE` (`cb9fc007-bcc7-4411-9786-5a0cd89dc526`) — `shaders/combine_blend.wgsl`, `combine_gpu.rs`, `combine_gpu_tests.rs`:** 1-/2-cycle `(A-B)*C+D` + `clamp9_cc` + blender `(P*A + M*B)/(A+B)` using zero-division 18-bit integer shift-subtract `div_u32_clamped255` (NVVM-safe) with alpha preserved across G/B RGB muxes.
  5. **`M6-W2-RASTER` (`7e14eec6-5e00-4df5-9ffa-d92412618aa1`) — `shaders/raster_core.wgsl`, `raster_gpu.rs`, `raster_gpu_tests.rs`:** Full Edge-Walker rasterizer (`FillRectangle` 8/16/32b, `TextureRectangle`/`Flip`, all 8 triangle opcodes `0x08..=0x0F`, 64-bit `I64` fixed-point math without struct `select()`, Gouraud shade, 18-bit RDRAM+hidden Z-buffer compare/update, AA/non-AA coverage, Bayer/Magic dither, atomic RDRAM + bit-packed hidden-bit stores).
  6. **Integration & Parity (`edgewalker/mod.rs`, `parity_tests.rs`):** Persistent `EdgeWalkerGpu` pipelines (`ingest`, `tmem_load`, `raster`) + buffers (`tmem_buf`, `tiles_buf`, `rdram_buf`, `hidden_buf`), single upload/readback batch per `submit_gpu`, and 5 new bit-exact `EdgeWalker == CpuRdpLle` parity + Bus DPC tests.
- **Orchestrator Wave 2 Verification Gate (Windows PowerShell, `$env:RUST_MIN_STACK = "16777216"`):**
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0`
  - `cargo test` (default, GPU-free) → lib **247 passed; 0 failed** + `cli_smoke` **3 passed; 0 failed** + `roms` **4 passed; 0 failed; 5 ignored** (**254 passed** total)
  - `cargo test --features vulkan-rdp rdp::edgewalker -- --nocapture` → **31 passed; 0 failed** live on **NVIDIA GeForce RTX 4090 Laptop GPU** (`Vulkan DiscreteGpu`)
  - `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`** (4.25s)
  - `cargo test --release --test roms probe_local_carts -- --ignored --nocapture` → **7/8 RUNNING** @ 2,000,000 cycles (`0.31s`); Mario Kart 64 clean `TRAP: unimplemented N64DD_REGS register read at offset 0x0508`
  - Forbidden-binary git hygiene (`git ls-files | Select-String ...`) → **`HYGIENE=CLEAN`**

### M6 — `EdgeWalker` Remediation Wave 1 (`feat/m6-edgewalker`, uncommitted atop `6f52687`, 2026-11-27)

- **WGSL NVVM & Atomics Hardening (`shaders/{tile_bin,vi,span}.wgsl`):**
  - Replaced integer division `0xFFFFFFFFu / b` in `u32_mul_overflows` with pure-multiply 16-bit split across `tile_bin.wgsl` and `vi.wgsl` (zero `/` or `%` across all 4 shaders), restored `replicate5` in `vi.wgsl`, upgraded `span.wgsl` `store_be_u16` to a race-free `atomicCompareExchangeWeak` CAS loop, and scrubbed CPU-reference wording so shaders cite N64brew RDP/VI specs directly.
- **Persistent GPU State & No Dead Host Patching (`mod.rs`, `pipeline.rs`, `tests.rs`):**
  - Added cached `Option<Arc<EdgeWalkerGpu>>` (`#[serde(skip)]`) to `EdgeWalker` reusing `ComputeDevice`, compiled `ingest_pipeline` + `span_pipeline`, and storage buffers (`params_buf`, `tmem_buf`, `coverage_buf`, `depth_buf`, lazily grown `color_buf`) across submits.
  - Added `Pipeline::dispatch_ingest_with_pipeline` so `submit_gpu` runs cached GPU ingest directly; deleted dead `span_shader_wgsl()` and `pack_cmd()`.
  - Restored `#[cfg(test)] mod tests;` in `src/rdp/edgewalker/mod.rs` and verified `edgewalker_reuses_persistent_gpu_state_across_submits` + `serde` round-trip.
- **Standalone GPU Pipeline Test Suite (`pipeline_tests.rs`):**
  - Extracted inline tests out of `pipeline.rs` (`AGENTS.md` §3) into `src/rdp/edgewalker/pipeline_tests.rs` with live **NVIDIA GeForce RTX 4090 Laptop GPU** dispatches covering **all 4 WGSL entry points** (`dispatch_ingest_classifies_all_four_sync_barriers_on_gpu`, `dispatch_tile_bin_bins_primitives_on_gpu`, `dispatch_span_fill_smoke_does_not_fault_device`, `dispatch_vi_rgba16_replicate_scanout_on_gpu`).
- **Orchestrator Wave 1 Verification Gate (Windows PowerShell, `RUST_MIN_STACK=16777216`):**
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0`
  - `cargo test` (default, GPU-free) → lib **239 passed; 0 failed** + cli_smoke **3 passed** + roms **4 passed; 5 ignored** (**246 passed** total)
  - `cargo test --features vulkan-rdp rdp::edgewalker -- --nocapture` → **16 passed; 0 failed** on **NVIDIA GeForce RTX 4090 Laptop GPU** (Vulkan)
  - `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`** (4.74s)
  - Forbidden-binary git check → CLEAN

### M6 — `EdgeWalker` GPU compute wave — full summary (`feat/m6-edgewalker`, uncommitted atop `6f52687`, 2026-11-27)

**`EdgeWalker` is Graycart's bespoke Vulkan/wgpu compute LLE RDP** — not a sidecar / not a plugin. Zero third-party RDP forks/FFI. Direct GPU deps only behind `vulkan-rdp` (`wgpu` 24.0.5, `pollster` 0.4). Default features stay GPU-free. `Bus.rdp` remains `CpuRdpLle` (DPC SSOT + cart/Dillonb path). Worker model `grok-4.7-high`. Session meta: **14/14 todos completed**; **35 project workers** across the broader graycart-n64 push; this wave's named EdgeWalker workers listed below.

#### Workers (chronological)

1. **W-GPU — ComputeDevice** (`433de11c-67c6-4f3a-85f8-47b52d7ca108`)
   - `vulkan-rdp = ["dep:wgpu", "dep:pollster"]`; real `ComputeDevice::open` (Vulkan preferred → DX12/Metal/WebGPU fallback).
   - API: `create_compute_pipeline`, storage buffers, `write_buffer`, bind groups, `dispatch_workgroups`, synchronous `readback`.
   - **Outcome:** `open` Ok on **NVIDIA GeForce RTX 4090 Laptop GPU** (Vulkan DiscreteGpu); 32-byte buffer roundtrip Ok. Gate: fmt clean; clippy `--features vulkan-rdp` clean; `cargo test --features vulkan-rdp rdp::edgewalker::gpu` → **2 passed**. Files: `Cargo.toml`, `Cargo.lock`, `gpu.rs`, `gpu_tests.rs`.

2. **W-SHADERS — real WGSL on disk** (`62d54a35-aa68-4342-8299-4ac37725d4e1`)
   - Replaced stub early-returns with writing shaders. Naga 24 validated every entry point.
   - `ingest_main`: barrier kinds 0/1 Pipe/2 Tile/3 Load/4 Full per command index.
   - `tile_bin_main`: flat `(bin_id, prim_id)` pairs + `prim_bin_counts` (32×32 bins default).
   - `span_main`: FILL-cycle RGBA16 FillRectangle (smoke writes `0x1111`/`0x2222`); non-FILL host-trapped.
   - `vi_main`: RGBA16 → RGBA8888 REPLICATE expand `(v<<3)|(v>>2)`.
   - Files: `shaders/{ingest,tile_bin,span,vi}.wgsl`.

3. **W-PIPE — live `Pipeline::dispatch_*`** (`2cecf067-59cc-4c75-8183-ddcfe7d9cbba` + Vulkan follow-up)
   - With feature: `dispatch_ingest` / `dispatch_tile_bin` / `dispatch_span` / `dispatch_vi` upload → dispatch → readback on `&ComputeDevice`.
   - Without feature: stage traps at offsets `0x10`–`0x13`, no pixel writes.
   - `SpanParamsPack` = 80-byte LE layout matching span WGSL.
   - **Vulkan int-div follow-up:** NVIDIA NVVM faulted on `index / rect_w`; temporary host rewrite before compile, then superseded by permanent WGSL fix. FILL smoke through `Pipeline` stored `11 11 22 22`. Gate after follow-up: **13 passed** edgewalker tests (incl. `dispatch_span_fill_smoke_does_not_fault_device`). File: `pipeline.rs`.

4. **W-HOT — remove CPU fallback** (`32f80256-9164-4923-82a7-90d6e0bdad98`)
   - Deleted `parity: CpuRdpLle` hot-path delegation. Without `vulkan-rdp`, `submit`/`sync` → `Trap::UnimplementedRegister` EdgeWalker `0x0001`. With feature: host latches SetColorImage / SetOtherModes / SetFillColor / SetScissor; FILL FillRectangle → GPU span → RDRAM readback; unimplemented opcodes → `UnimplementedRdpCommand`.
   - `dispatch_compute` opens device and runs ingest (Sync Full → kind 4).
   - Seed parity bit-exact vs `CpuRdpLle`: empty submit, SyncFull only, FillRectangle smoke.
   - Gate: fmt/clippy `--all-features` clean; default `cargo test` lib **239** + cli **3** + roms **4**/5 ignored; `cargo test --features vulkan-rdp rdp::edgewalker` → **12 passed** at handoff (became **13** after W-PIPE smoke test). Files: `mod.rs`, `tests.rs`, `parity_tests.rs`, `src/tests.rs` (`m6` cfg).

5. **M6 fix span Vulkan ÷** (`898466ab-5ec6-4988-a702-61fc9717cc80`)
   - Permanent `span.wgsl`: f32 `floor` index split + multiply overflow guards; removed integer `/` that faulted NVVM.
   - `pipeline_for` compiles raw WGSL (host rewrite removed). Reconfirm: **13 passed** edgewalker tests on RTX 4090.

#### Orchestrator final gate paste (Windows PowerShell, `RUST_MIN_STACK=16777216`)

- `cargo fmt --check` → exit `0`
- `cargo clippy --all-targets --all-features -- -D warnings` → exit `0`
- `cargo test` → lib **239 passed; 0 failed**; cli_smoke **3 passed**; roms **4 passed; 5 ignored**
- `cargo test --features vulkan-rdp rdp::edgewalker` → **13 passed; 0 failed** (ComputeDevice open Ok backend=Vulkan name=NVIDIA GeForce RTX 4090 Laptop GPU; fill-rectangle / empty / SyncFull parity matched `CpuRdpLle`; `Pipeline::dispatch_span` FILL smoke Ok)
- `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`** (~4.73s)
- `cargo test --release --test roms probe_local_carts -- --ignored --nocapture` → **7/8 RUNNING** at 2M cycles; Mario Kart 64 clean `N64DD_REGS` read trap at offset `0x0508`
- Forbidden-binary hygiene → `Clean: no forbidden binaries tracked`

#### Working-tree scope (measured)

- Tracked product (excl. lock noise): `Cargo.toml`, `STATUS.md`, `src/rdp/edgewalker/{mod,gpu,pipeline,tests,parity_tests}.rs`, `shaders/{ingest,tile_bin,span,vi}.wgsl`, `src/tests.rs` → **12 files, +2610 / −221** (`git diff --stat HEAD -- . ":!Cargo.lock"`).
- Plus untracked `gpu_tests.rs`; plus `Cargo.lock` wgpu tree. Planning scratch (`.superpowers/`, `task_plan.md`, `findings.md`, `progress.md`, `docs/superpowers/`) is session-local and not the LLE deliverable.
- Session UI roll-up cited for this wave: **17 changed files (+639/−86)**; todos **14/14** complete; **35** project workers.

#### Progress Board (end state)

| Item | Status |
|---|---|
| W-GPU ComputeDevice (wgpu/Vulkan, RTX 4090) | Done |
| W-SHADERS real WGSL | Done |
| W-PIPE live dispatch + Vulkan ÷ mitigation | Done |
| W-HOT no CPU fallback + seed parity | Done |
| Permanent `span.wgsl` Vulkan ÷ fix | Done |
| fmt / clippy / cargo test / Dillonb 26/26 / carts 7/8 / hygiene | Green |
| STATUS.md receipt + Progress Board | Updated |
| Dave Done bless / commit | **Awaiting** |

**Still open (trap-don't-stub):** EdgeWalker triangles / texture rectangles / TMEM loads / non-FILL cycles. **No M6 commit yet.**

**Dirty product paths:** `Cargo.toml`, `Cargo.lock`, `src/rdp/edgewalker/{mod,gpu,gpu_tests,pipeline,tests,parity,parity_tests}.rs`, `src/rdp/edgewalker/shaders/{ingest,tile_bin,span,vi}.wgsl`, `src/tests.rs`, `STATUS.md`.

### M6 — `EdgeWalker` kickoff (historical; superseded by GPU wave receipt above)

Kickoff W0–W2 had CPU parity fallback + trapping GPU stubs. That fallback is **gone**. Prior kickoff notes kept only for ledger archaeology.

### M5 W1–W4 — `CpuRdpLle` (`feat/m1-cpu`, commit `92ba55e`, 2026-11-27)

Wave committed as `92ba55e` on `feat/m1-cpu` (not pushed). M5 is **not** human-blessed Done. Final CI reconfirm PASS. M6 kickoff is uncommitted on top of this commit.

- **Worker model:** `grok-4.7-high`
- **W1:** FIFO fetch + multi-word decoder + state setters; hooked to TMEM + `draw_primitive`.
- **W2:** 4 KiB TMEM, 8 tiles, loads, sampler (**13** tests); clippy args bundled.
- **W3:** edge-walker triangles / rects / scissor / Z / coverage (**18** tests).
- **W4:** combiner/blender + VI RGBA8888 scanout (**15+9** tests).
- **Landed in `92ba55e`:** `src/bus/mod.rs`, `src/rdp/mod.rs`, `src/rdp/regs.rs`, `src/rdp/regs_tests.rs`, `src/rdp/tmem.rs`, `src/rdp/tmem_tests.rs`, `src/rdp/raster.rs`, `src/rdp/raster_tests.rs`, `src/rdp/combine_blend.rs`, `src/rdp/combine_blend_tests.rs`, `src/vi/mod.rs`, `src/vi/tests.rs`.
- **Final reconfirm:** `cargo fmt --check` PASS; `cargo clippy --all-targets --all-features -- -D warnings` PASS; `cargo test` **241 passed; 0 failed; 5 ignored** (234 lib + 3 CLI + 4 roms); Dillonb **26/26**; local carts **7/8**; hygiene clean.
- **Still trapping (not Done):** `SetConvert` / `SetKey*`, COPY/AA and other exotic blender modes, and related unimplemented RDP paths (trap, don't stub).

### M5.0 — Bus.rdp seam (`feat/m1-cpu`, commit `d89d417`, 2026-11-27)

Seam receipt only (this commit). W1–W4 follow-on is committed as `92ba55e` (receipt above; not pushed). M5 is **not** Done.

- **Worker model:** `grok-4.7-high`
- **Seam:** RDP state lives only on `Bus.rdp`. Orphaned throwaway `CpuRdpLle::new()` and `N64Core.rdp` removed. `RaiseDpIntr` → `MI_INTR_DP`.
- **Tests:** e2e `m5_0_bus_rdp_persistent_mmio_and_syncfull_mi_intr_dp` passing; **18** RDP unit tests passing.
- **`regs_tests`:** offset `0x20` is the `DPC_START` mirror (`& 0x1F`); the unknown-offset trap uses `0x02`.
- **Final gate** (Windows PowerShell, `RUST_MIN_STACK=16777216`): `cargo fmt` ok; clippy `-D warnings` ok; `cargo test` → **179 passed** = **172** library + **3** CLI smoke + **4** ROM harness; **0 failed**; **5 ignored** (default roms matrix stubs). Zero failures; zero Orchestrator code fixes during the final gate.
- **Dillonb:** `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → **26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT**.
- **Local carts:** `probe_local_carts` at 2M cycles → **7/8** clean; Mario Kart 64 same clean `N64DD_REGS` presence-probe trap at offset `0x0508`.
- **Hygiene:** clean (no forbidden binaries tracked).

### M4 — RSP Vector Unit (COP2 / LWC2 / SWC2) (`feat/m1-cpu`, 2026-11-27)

- Orchestrator M4.0 seam (`src/rsp/vu.rs` 32×128-bit VPR, 8×48-bit accumulator `acc_hi`/`acc_md`/`acc_lo`, `VCO`/`VCC`/`VCE`, element swizzle engine, signed/unsigned/low accumulator clampers, COP2/LWC2/SWC2 dispatch in `src/rsp/scalar.rs`) + 4 parallel `flash_lite` workers on strictly disjoint files:
  - **M4-W1** (`src/rsp/vu_lsu.rs` + `vu_lsu_tests.rs`): all 11 `LWC2` loads (`LBV`..`LTV`) and 12 `SWC2` stores (`SBV`..`STV` + `SWV`) with 4 KiB wrapping DMEM and 8×8 matrix transpose.
  - **M4-W2** (`src/rsp/vu_arith.rs` + `vu_arith_tests.rs`): all 22 multiply/MAC/add/sub/abs/saw VU arithmetic ops (`VMULF`, `VMULU`, `VRNDP`, `VMULQ`, `VMUDL/M/N/H`, `VMACF`, `VMACU`, `VRNDN`, `VMACQ`, `VMACL/M/N/H`, `VADD`, `VSUB`, `VABS`, `VADDC`, `VSUBC`, `VSAW`).
  - **M4-W3** (`src/rsp/vu_logic_cmp.rs` + `vu_logic_cmp_tests.rs`): all 17 compare/clip/merge/logic/move/nop ops (`VLT`, `VEQ`, `VNE`, `VGE`, `VCL`, `VCH`, `VCR`, `VMRG`, `VAND`/`VNAND`/`VOR`/`VNOR`/`VXOR`/`VNXOR`, `VMOV`, `VNOP`/`VNULL`).
  - **M4-W4** (`src/rsp/vu_recip.rs` + `vu_recip_tests.rs`): `MFC2`/`MTC2`/`CFC2`/`CTC2` + 512-entry integer `RCP_ROM`/`RSQ_ROM` lookup tables (`VRCP`/`VRCPL`/`VRCPH`/`VRSQ`/`VRSQL`/`VRSQH`).
- Orchestrator line-by-line review caught & fixed 5 bugs before commit:
  - `src/rsp/vu_lsu.rs`: `LTV` double-rotated source offsets and `STV` echoed `LTV` instead of reading the diagonal (`vpr[vr][i]`); fixed both to exact transpose equations and locked with an asymmetric 8×8 matrix transpose test.
  - `src/rsp/vu_logic_cmp.rs`: `VMOV` (`0x33`) was missing early `return Ok(())`, causing the trailing `rsp.vu.vpr[vd] = out` to overwrite all 8 lanes instead of only lane `vs & 7`; fixed and verified.
  - `src/rsp/vu_recip.rs`: replaced floating-point `get_rsq` and clamped `get_rcp` shift with zero-float 128-bit integer `isqrt` `RSQ_ROM` and exact N64 `msb` shift + 1's-complement negation.
  - Restored `#[cfg(test)] #[path = ...]` test module hooks dropped in `vu_arith.rs` & `vu_logic_cmp.rs`, fixed `MTC2` low-16-bit assertion, and added end-to-end `m4_rsp_vector_program_lqv_vadd_sqv_end_to_end` in `src/tests.rs`.
- Local validation (Windows PowerShell, `RUST_MIN_STACK=16777216`):
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 lint warnings)
  - `cargo test` → exit `0`:
    - `src/lib.rs` unit tests: **153 passed; 0 failed; 0 ignored** (+20 new M4 VU unit & end-to-end tests)
    - `tests/cli_smoke.rs`: **3 passed; 0 failed; 0 ignored**
    - `tests/roms/main.rs`: **4 passed; 0 failed; 5 ignored**
  - `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → exit `0` (**5.29s**): **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`**.
  - `cargo test --release --test roms probe_local_carts -- --ignored --nocapture` (**2M cycles**, **0.43s**): **7/8 commercial carts running 2,000,000 cycles cleanly** (zero regressions; MK64 at clean `N64DD_REGS` probe trap).
  - Forbidden-binary git check → `Clean: no forbidden binaries tracked`.

### M3 — RSP Scalar Unit, SP MMIO + 2D DMA & `AGENTS.md` Parallel Playbook (`feat/m1-cpu`, 2026-11-27)

- Orchestrator M3.0 seam (single-source-of-truth `Bus.sp` + `sp_dmem` + `sp_imem`, `pa - 0x0404_0000` SP offset routing separating `SP_MEM_ADDR` from `SP_PC` at `0x0408_0000`, atomic `Cell<bool>` `SP_SEMAPHORE`, exact `2/3` RCP cycle scheduling of `InterpRsp::run` in `src/lib.rs`) + 3 parallel `flash_lite` workers (`src/rsp/regs.rs`+`regs_tests.rs`, `src/rsp/scalar.rs`+`scalar_tests.rs`, and comprehensive `AGENTS.md`).
- Orchestrator line-by-line review caught & fixed before commit:
  - `src/rsp/regs.rs`: `SP_STATUS` write bits 3..24 were shifted by +1 bit because `CLEAR_BROKE` is a single bit at bit 2 (no `SET_BROKE`); fixed exact 25-bit paired layout (`INTR` bits 3/4, `SSTEP` 5/6, `INTR_ON_BREAK` 7/8, `SIG0..SIG7` 9..24) and fixed 2D DMA `skip` to apply only between rows (`row + 1 < rows`).
  - `src/rsp/scalar.rs` & `scalar_tests.rs`: link return address uses `normal_next_pc.wrapping_add(4) & 0x0FFC` (delay-slot safe); fixed `SLL`/`SRL` `sa=4` bit encodings in `test_shifts_and_logic`.
  - `AGENTS.md`: fixed typo (`Refamending` → `Refactoring`) and expanded §6 with full Windows PowerShell validation, Dillonb, local-cart, and forbidden-binary commands.
  - Added end-to-end M3 integration test `m3_rsp_scalar_program_dma_and_break_interrupt_end_to_end` in `src/tests.rs` (CPU DMAs 12-instruction RSP loop into IMEM, starts RSP with `INTR_ON_BREAK`, RSP loops `sum(1..=5)=15`, writes DMEM, executes `MTC0` SP DMA back to RDRAM, and `BREAK`s raising `MI_INTR_SP → COP0.Cause.IP2`).
- Local validation (Windows PowerShell, `RUST_MIN_STACK=16777216`):
  - `cargo fmt --check` → exit `0`
  - `cargo clippy --all-targets --all-features -- -D warnings` → exit `0` (0 lint warnings)
  - `cargo test` → exit `0`:
    - `src/lib.rs` unit tests: **133 passed; 0 failed; 0 ignored** (+15 new M3 SP/RSP unit & end-to-end tests)
    - `tests/cli_smoke.rs`: **3 passed; 0 failed; 0 ignored**
    - `tests/roms/main.rs`: **4 passed; 0 failed; 5 ignored**
  - `cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture` → exit `0` (**4.98s**): **`26/26 PASS | 0 FAIL | 0 TRAP | 0 TIMEOUT`** (zero regressions).
  - `cargo test --release --test roms probe_local_carts -- --ignored --nocapture` (**2,000,000 cycles**, local-only `..\carts\n64`, **0.30s**):
    - **7/8 commercial carts ran 2,000,000 cycles cleanly with ZERO traps** (SM64, GoldenEye 007 in TLB KUSEG `0x702009C8`, Majora's Mask, Ocarina of Time Rev 2, Donkey Kong 64, Star Wars Ep. I Racer, Super Smash Bros.; SM64/GoldenEye/MM actively mutated their RDRAM framebuffer hashes!).
    - **Mario Kart 64** ran `412,681` cycles cleanly and hit `Trap::UnimplementedRegister { device: "N64DD_REGS", offset: 0x0508, is_write: false }` (`libultra` 64DD presence probe `ASIC_STATUS` — properly trapped per Standing Rule #1).
  - Forbidden-binary git check → `Clean: no forbidden binaries tracked`.

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
| 2026-11-27 | M6 `EdgeWalker` | Treating `EdgeWalker` as a sidecar, plugin, or second RDP beside `CpuRdpLle` | `EdgeWalker` **is** the bespoke Vulkan compute LLE RDP. `CpuRdpLle` is the M5 software oracle and the temporary CPU parity pipe. Dispatch stays trapped until real shaders are bit-exact. |
| 2026-11-27 | Test vendoring | Committing `Dillonb/n64-tests` or `240p Test Suite` ROMs in git | Dillonb has no license (never redistribute); 240p Suite is GPL-2.0+ (incompatible with MIT crate). Fetch-and-run only via `scripts/fetch-suites.sh` into gitignored `tests/fixtures/fetched/`. |
