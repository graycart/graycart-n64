# Agent guidelines — Graycart (`graycart-n64`)

Canonical norms for changing this emulator. Aligned with [`docs/AGENTS.md`](./docs/AGENTS.md), [`ATTRIBUTION.md`](./ATTRIBUTION.md), [`THIRD_PARTY.md`](./THIRD_PARTY.md), [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md), and [`docs/ORCHESTRATOR-HANDOFF.md`](./docs/ORCHESTRATOR-HANDOFF.md) (multi-subagent local playbook).

## Standing rules (non-negotiable)

1. **Trap, don't stub.** Unimplemented opcodes, RDP commands, COP0 registers, MMIO offsets, and illegal SysAD accesses MUST return an explicit `Trap` (or panic in strict mode) with PC/address/word. Never NOP-and-continue or silently return `0`.
2. **Done means a receipt.** A task is complete only when its named validation command output (counts, hashes) is pasted into the result and recorded in [`STATUS.md`](./STATUS.md).
3. **One family per task.** One opcode family, one RDP command group, or one DMA channel per task, with hardware pseudocode, edge vectors, a named fail→pass gate, no regressions on previously green categories, and an explicit file scope.
4. **Hypothesis before code on debugging.** Write the failing log/diff hypothesis before editing code.
5. **License hygiene is mechanical.** Check [`THIRD_PARTY.md`](./THIRD_PARTY.md) before reading or citing external projects. GPL, MAME/Angrylion, and unlicensed reference trees NEVER enter `src/` or agent-copyable paths (`ref/` is gitignored and study-only). No third-party emulator/RDP forks or C++ FFI plugins (`EdgeWalker`, our M6 Vulkan compute LLE RDP, is 100% bespoke).
6. **Mandatory file-top attribution.** Every Rust or Markdown file relying on external documentation or code MUST carry a top-of-file credit header per [`ATTRIBUTION.md`](./ATTRIBUTION.md).
7. **Maintain the ruled-out ledger.** Read and update [`STATUS.md`](./STATUS.md) every session so dead ends are never retried.
8. **Human gates stay human.** Never re-bless a golden from our own output without an external oracle, and never declare a milestone complete without Dave's review of the receipt.

## Architecture & separation

- **Library-first core:** `graycart-n64` lib has **zero** GUI dependencies (`winit`/`wgpu`/`egui`/`cpal` belong only in a future binary frontend).
- **Two-tier accuracy contract:** Default `N64Config::accurate()` is strictly hardware-faithful LLE (and is what CI tests). Opt-in enhancements (`EnhancementConfig`, M11) live strictly behind explicit flags.
- **Tests layout:** Unit tests live in `src/<module>/tests.rs` via `#[cfg(test)] mod tests;` — not inline in production files. Integration tests live under `tests/`.
- **Forbidden in git:** Never commit `carts/`, `ref/`, `tests/fixtures/fetched/`, `.z64`/`.n64`/`.v64` commercial ROMs, `.sav`/`.eep`/`.sra`/`.fla` saves, or PIF/CIC/IPL dumps.

## Required validation

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
RUST_MIN_STACK=16777216 cargo test
```

Default CI MUST stay green without ROMs, PIF/CIC dumps, or `-- --ignored`.
