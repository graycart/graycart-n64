# Agent guidelines -- Graycart (`graycart-n64`)

How to change this emulator. Product name **Graycart**; crate/binary target **`graycart-n64`**. Family: [graycart](https://github.com/graycart/graycart). Research gold standard: [graycart-gba](https://github.com/graycart/graycart-gba) `docs/` (provenance discipline).

> **Sync:** Canonical research pack lives in this repo under [`docs/`](./README.md). Keep norms here aligned with graycart-gba / graycart-gb `AGENTS.md` posture.

## Project goal

1. Emulate **Nintendo 64** hardware: NEC **VR4300** (MIPS III / R4300i-compatible) + SGI **Reality Coprocessor** (**RSP** + **RDP**) + peripherals (**MI**, **PI**, **SI**, **AI**, **VI**, **RI**).
2. Ship a **library-first** core for desktop hosts and future graycart-linux via the family core API.
3. Prefer **hardware-faithful** paths; when using HLE (RSP microcodes, RDP, PIF/CIC boot), **label it** and keep an LLE path on the roadmap.

### How agents should navigate

1. Read **this file** + [`ATTRIBUTION.md`](./ATTRIBUTION.md) before editing sources.
2. Research pack (this directory): start with [`PHASES.md`](./PHASES.md) and [`08-implementation-plan.md`](./08-implementation-plan.md).
3. Family-standard core API: Project store [`docs/graycart-family/01-core-api.md`](../graycart-family/01-core-api.md) when available — align host/core seams.
4. Own only your Phase partition paths; do **not** touch sibling repos (`graycart-snes`, etc.) unless assigned.

## Hardware-first

Commercial progress is a **smoke test**. Conformance / torture ROMs are the **accuracy test**.

- Fix the **owning subsystem** (CPU, bus/RI, PI, SI, MI, VI, AI, RSP, RDP, cart/PIF). Trace to the earliest wrong hardware-visible state.
- **No title hacks.** No game-specific special cases when a real program path after a bad observation would explain it.
- **Primary sources first** (NEC VR4300 UM, community reconstructions of N64 Programming Manual / RSP Programmer’s Guide that are legally citable, n64brew wiki as curated reconstruction). Then test-ROM expectations. Then trusted **secondary** notes (ares, cen64, mupen64plus, parallel-RDP, angrylion) — never as sole authority.
- Unsupported cart features fail clearly — never silent wrong-save-type or pretend-ROM-only.
- **Never** commit PIF ROM, CIC dumps, IPL binaries, commercial `.z64`/`.n64`/`.v64`, or large copyrighted manuals.

## Attribution (file headers -- mandatory)

Full policy + examples: [`ATTRIBUTION.md`](./ATTRIBUTION.md).

If a source file (Rust **or** Markdown in this repo) references someone else's **code** or **internet documentation**, put **credit at the top of that file**:

- what was used
- URL and/or name
- brief note (inspired by / ported from / cited)

Provenance folders and README link lists alone are **not** enough.

Example (Rust):

```rust
//! RDRAM physical map + RI refresh stubs.
//!
//! Cited: N64brew — Memory map
//!   https://n64brew.dev/wiki/Memory_map
//! Cross-check: NEC VR4300 User's Manual (COP0/TLB) — cite section, do not paste.
```

Example (Markdown):

```markdown
<!--
Cited: N64brew — Reality Signal Processor
URL: https://n64brew.dev/wiki/Reality_Signal_Processor
Note: IMEM/DMEM sizes and dual-issue SU+VU summary; not a full reprint.
-->
```

## Core vs host

The **lib** is the machine. The **binary** (`frontend/`, when present) owns winit/wgpu/egui/cpal. Core speaks framebuffer (VI present), PCM (AI), and controller state only. Headless `--frames` must not depend on the window stack.

- VI filtering / shade→RGB presentation details may live in the host, but RDP framebuffer bytes belong to the core.
- UI: no disabled "coming soon" items for unfinished core features.
- Release builds are the supported play mode once a frontend exists.

## Modules

If something has its own state, rules, tests, or lifecycle, it gets its own module. Keep orchestration thin (`lib.rs` / machine glue, `bus`, `main.rs`). Do not dump RSP/RDP logic into the CPU execute loop.

Suggested nouns (scaffold):

```text
cpu/        # VR4300 interpreter (+ later dynarec)
bus/        # physical map, SysAD quirks
rcp/        # shared RCP glue
rsp/        # SU + VU + DMA + tasks
rdp/        # command processor + pipe (LLE) / plugin seam
mi/ vi/ ai/ pi/ si/ ri/
pif/        # HLE joybus + boot stubs (no dumps)
cart/       # ROM endian, saves
frontend/   # binary-only host
```

Tests: `src/<module>/tests.rs` via `#[cfg(test)] mod tests;` — not inline in production files. Integration under `tests/` uses the public API. ROM harnesses under `tests/roms/` with fixtures in `tests/fixtures/` (licenses + README per suite).

## Parallelism

Independent tasks: disjoint files, no shared unfinished types. Interface producers before consumers. RSP LLE and RDP LLE are **coupled** for accuracy modes — coordinate plugins/seams in [09](./09-graphics-plugin-accuracy.md).

## SemVer

Same posture as graycart-gb / graycart-gba:

- `Cargo.toml` `[package].version` is the only product version.
- Scaffold starts at **`0.0.1`**; first tagged runnable milestone targets **`0.1.0`**.
- **Do not** ship `1.0.0` until play + save compatibility + basic cross-platform trust exist.
- Git tag **`vX.Y.Z` must equal** the crate version.

## Validation (required)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Default CI must stay green **without** PIF/CIC dumps, commercial ROMs, or ignored ROM matrices. Never `cargo test -- --ignored` in the default workflow.

Never commit `carts/*`, `.sav`, state dumps, firmware dumps, secrets, or skip hooks. Never force-push `main`.

Do not weaken tests (drop asserts, skip without a reason, title-specific expected values).

## Docs and research

- In-repo: this file, [`ATTRIBUTION.md`](./ATTRIBUTION.md), root `README.md`, and (when added) `docs/conformance.md` + obtain-your-own firmware notes.
- Research: this `docs/` tree — especially `08-implementation-plan.md`, `PHASES.md`, `07-test-strategy.md`, `09-graphics-plugin-accuracy.md`, `10-family-api-and-reuse.md`, and `provenance/`.
- Family API: Project store `docs/graycart-family/`.

No emulator core implementation until the research review gate in `08-implementation-plan.md` section 7 is accepted.
