# Graycart N64 — Accuracy & Ruled-Out Ledger (`STATUS.md`)

Loaded into every implementation session per [`AGENTS.md`](./AGENTS.md) and [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) §5.6.
Milestones advance only on pasted, verifiable test receipts reviewed by Dave.

## Active Milestone

- **Current:** `M6 — EdgeWalker` kickoff (W0–W2) landed **uncommitted** on `feat/m1-cpu` after M5 wave commit `92ba55e`. **`EdgeWalker` is the bespoke Vulkan compute LLE RDP** (the M6 GPU LLE itself, not a sidecar and not a plugin). W0 seam: `RdpBackend`; `submit` / `sync` run the CPU parity pipe until real shaders. W1: bit-exact parity harness vs `CpuRdpLle`. W2: compute stage stubs (`ingest` / `tile_bin` / `span` / `vi` WGSL) behind `vulkan-rdp`; dispatch traps until real shaders. Gate notes: `cargo fmt --check` green; `cargo clippy --all-targets --all-features -- -D warnings` green; W2 report ~**243** library tests passed. **M6 is not Done** — GPU LLE is not verified, the games-running bar is not met, and the final commit / push / squash / next-branch sequence is **PARKED**. Worker model `grok-4.7-high`. M5 wave is `92ba55e` (not pushed, not human-blessed Done; still trapping `SetConvert`/`SetKey*`, COPY/AA and other exotic blender modes). M5.0 seam remains `d89d417` (**179 passed**). M4 RSP vector receipts remain ready for Dave's review.
- **Branch / HEAD:** `feat/m1-cpu` at `92ba55e` (local feature branch, 8 commits ahead of `origin/main` `28a507a`, not yet pushed/PR'd). M6 W0–W2 is a dirty working tree on top of `92ba55e` — **no M6 commit hash**.
- **Bespoke Vulkan Compute LLE RDP Name:** **`EdgeWalker`** (zero third-party RDP forks/FFI). `EdgeWalker` **is** the M6 bespoke Vulkan compute LLE RDP. The CPU pipe is the temporary parity path until real shaders replace it.
- **M6 kickoff (uncommitted, on `92ba55e`):** W0 `RdpBackend` seam + CPU parity pipe. W1 bit-exact harness vs `CpuRdpLle` (VI scanout frame hash and raw framebuffer FNV, including the 9th-bit window; seed cases: empty submit, SyncFull only, FillRectangle smoke). W2 `ingest` / `tile_bin` / `span` / `vi` WGSL stubs behind `vulkan-rdp` (wgpu planned, not ash; crate not linked; default features stay GPU-free). `dispatch_compute` / `Pipeline::dispatch` trap until real shaders. **Not Done.** Commit / push / squash / next-branch **PARKED**.
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
- **M6 EdgeWalker kickoff (uncommitted, on `92ba55e`):** worker model `grok-4.7-high`. **`EdgeWalker` is the bespoke Vulkan compute LLE RDP**, not a sidecar. W0: `RdpBackend` seam; `submit` / `sync` stay on the CPU parity pipe until real shaders. W1: bit-exact parity harness vs `CpuRdpLle`. W2: `ingest` / `tile_bin` / `span` / `vi` WGSL stubs behind `vulkan-rdp`; dispatch traps until real shaders. fmt/clippy green; W2 report ~**243** library tests passed. **Not Done** — GPU LLE not verified; games-running bar not met. Commit / push / squash / next-branch **PARKED**. Dirty paths: `Cargo.toml`, `src/rdp/mod.rs`, `src/tests.rs`, `src/rdp/edgewalker/` (`mod.rs`, `gpu.rs`, `tests.rs`, `parity.rs`, `parity_tests.rs`, `pipeline.rs`, `shaders/{ingest,tile_bin,span,vi}.wgsl`), `STATUS.md`.

## Milestone Ledger (M0–M12)

| Milestone | Status | Gate Receipt Summary |
|---|---|---|
| **M0 — Scaffold** | **Done** (PR #2 merged, CI green 3/3 OS) | 21 passed (15 unit, 3 CLI smoke, 3 ROM harness), 0 failed, 3 ignored matrix stubs compiled; fmt/clippy `-D warnings` clean; macOS/Ubuntu/Windows CI green; zero forbidden binaries tracked |
| **M1 — CPU functional** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | Full VR4300 ISA + COP0/Exceptions + 32-entry TLB + COP1 IEEE-754 FPU + L1 I/D Caches + VR4300 `SRA`/`SRAV` 64-bit hardware quirk; **26/26 Dillonb `n64-tests` PASS** (0 FAIL, 0 TRAP, 0 TIMEOUT) |
| **M2 — Bus + boot** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | MI/RI/PI/SI/PIF-HLE/VI/AI registers & scheduled DMA/IRQ wired; **26/26 Dillonb PASS**; **8/8 local commercial carts running 200k cycles with 0 traps** |
| **M3 — RSP scalar + DMA** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | SP MMIO + 2D skip/count DMA + full 32-bit RSP Scalar Unit + `BREAK`/`COP0` + exact 2/3 RCP scheduling; **26/26 Dillonb PASS**; **7/8 local commercial carts running 2,000,000 cycles cleanly** |
| **M4 — RSP vector unit** | **Receipts ready (`feat/m1-cpu`, awaiting Dave's review)** | 160 passed (153 unit, 3 CLI smoke, 4 ROM harness), 0 failed, 5 ignored; full 32×128-bit VU (`LWC2`/`SWC2` + 8×8 transpose, 22 arith/MAC ops w/ 48-bit acc, 17 compare/clip/logic ops, `MFC2`/`MTC2`/`CFC2`/`CTC2`, zero-float integer `RCP`/`RSQ` 512-entry ROMs); **26/26 Dillonb PASS** |
| **M5 — Bespoke Software LLE RDP (`CpuRdpLle`) + VI** | Wave committed `92ba55e` on `feat/m1-cpu` (not pushed; not human-blessed Done; final CI reconfirm PASS) | W1 FIFO/decoder/setters hooked to TMEM + `draw_primitive`; W2 4 KiB TMEM + 8 tiles + sampler (**13** tests); W3 edge-walker (**18** tests); W4 combiner/blender + VI RGBA8888 (**15+9** tests). Final reconfirm: `cargo fmt --check` PASS; `cargo clippy --all-targets --all-features -- -D warnings` PASS; `cargo test` **241 passed; 0 failed; 5 ignored** (234 lib + 3 CLI + 4 roms); Dillonb **26/26**; carts **7/8**; hygiene clean. Still trapping: `SetConvert`/`SetKey*`, COPY/AA exotic blender modes. M5.0 seam commit remains `d89d417` (**179 passed**) |
| **M6 — Bespoke Vulkan Compute LLE RDP (`EdgeWalker`)** | Kickoff W0–W2 uncommitted on `92ba55e` — **not Done** (GPU LLE unverified; games-running bar not met; commit/push/squash/next-branch **PARKED**) | `EdgeWalker` **is** the Vulkan compute LLE RDP (not a sidecar). W0 `RdpBackend` + CPU parity pipe; W1 bit-exact harness vs `CpuRdpLle`; W2 `ingest`/`tile_bin`/`span`/`vi` WGSL stubs behind `vulkan-rdp`; dispatch traps until real shaders. fmt/clippy green; W2 report ~**243** lib tests passed. Worker model `grok-4.7-high` |
| **M7 — Audio (AI + RSP mixer)** | Not started | — |
| **M8 — PIF/Joybus/Saves/4-port Transfer Pak** | Not started | — |
| **M9 — Commercial smoke (local-only)** | Not started | — |
| **M10 — Timing hardening (`timing`/`cop0hazard`/`cycle`)** | Not started | — |
| **M11 — Modern Enhancements & Player Experience Tier** | Not started | — |
| **M12 — Optional dynarec + Family API extract** | Not started | — |

## Receipts

### M6 — `EdgeWalker` kickoff (`feat/m1-cpu`, uncommitted on `92ba55e`, 2026-11-27)

Kickoff receipt only. **No M6 commit.** **`EdgeWalker` is Graycart's bespoke Vulkan compute LLE RDP** — the M6 GPU LLE itself, not a sidecar and not a plugin. Zero third-party RDP forks/FFI. Worker model `grok-4.7-high`.

**M6 is not Done.** The GPU LLE is not verified. The games-running bar is not met. The final commit / push / squash / next-branch sequence is **PARKED**.

- **W0 — seam.** `EdgeWalker` implements `RdpBackend`. `submit` / `sync` execute the CPU parity pipe so frame bytes and latched state match `CpuRdpLle` until real shaders exist. `dispatch_compute` returns `Trap::UnimplementedRegister { device: "EdgeWalker", offset: 0x0001, is_write: true }`. Feature `vulkan-rdp` compiles `ComputeDevice::open`, which traps the same way. wgpu is the planned pure-Rust compute API; the crate is not a dependency yet. ash is not used. Default features stay GPU-free.
- **W1 — bit-exact parity harness** vs `CpuRdpLle` (VI scanout frame hash and raw framebuffer FNV, including the 9th-bit window). Seed cases: empty submit, SyncFull only, FillRectangle smoke.
- **W2 — compute stage stubs.** `ingest` / `tile_bin` / `span` / `vi` WGSL live behind `vulkan-rdp` (`pipeline.rs` + `shaders/`). Sources are embedded when the feature is on. They are not submitted to a device. `Pipeline::dispatch` traps until real shaders. `vulkan-rdp` still links no GPU crate.
- **Files (dirty, no commit):** `Cargo.toml`, `src/rdp/mod.rs`, `src/tests.rs`, `src/rdp/edgewalker/mod.rs`, `src/rdp/edgewalker/gpu.rs`, `src/rdp/edgewalker/tests.rs`, `src/rdp/edgewalker/parity.rs`, `src/rdp/edgewalker/parity_tests.rs`, `src/rdp/edgewalker/pipeline.rs`, `src/rdp/edgewalker/shaders/ingest.wgsl`, `src/rdp/edgewalker/shaders/tile_bin.wgsl`, `src/rdp/edgewalker/shaders/span.wgsl`, `src/rdp/edgewalker/shaders/vi.wgsl`, `STATUS.md`.
- **Earlier wave notes** (kept; W2 is the latest reported gate):
  - W0: `cargo test rdp::` → lib **78 passed; 0 failed; 0 ignored; 160 filtered out**. `cargo test rdp::edgewalker --features vulkan-rdp` → **5 passed; 0 failed**.
  - W1: `cargo test m6_edgewalker` → **1 passed; 0 failed** (242 filtered); `cargo test rdp::edgewalker` → **8 passed; 0 failed** (235 filtered).
- **W2 gate notes** (Windows PowerShell, `RUST_MIN_STACK=16777216`):
  - `cargo fmt --check` → green
  - `cargo clippy --all-targets --all-features -- -D warnings` → green
  - W2 report: library tests ~**243 passed**
  - GPU LLE **not** verified. Games-running bar **not** met. Do not commit, push, squash, or cut the next branch until Dave unparks that sequence.

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
