# Agent guidelines — Graycart (`graycart-n64`)

Canonical norms for changing this emulator. Aligned with [`docs/AGENTS.md`](./docs/AGENTS.md), [`ATTRIBUTION.md`](./ATTRIBUTION.md), [`THIRD_PARTY.md`](./THIRD_PARTY.md), [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md), [`docs/ORCHESTRATOR-HANDOFF.md`](./docs/ORCHESTRATOR-HANDOFF.md), and [`docs/13-debugger-and-ui-architecture.md`](./docs/13-debugger-and-ui-architecture.md).

---

## 1. Standing Rules (Non-Negotiable)

1. **Trap, don't stub.** Unimplemented opcodes, RDP commands, COP0 registers, MMIO offsets, and illegal SysAD accesses MUST return an explicit `Trap` (or panic in strict mode) with PC/address/word. Never NOP-and-continue or silently return `0`.
2. **Done means a receipt.** A task is complete only when its named validation command output (counts, hashes) is pasted into the result and recorded in [`STATUS.md`](./STATUS.md).
3. **One family per task.** One opcode family, one RDP command group, or one DMA channel per task, with hardware pseudocode, edge vectors, a named fail→pass gate, no regressions on previously green categories, and an explicit file scope.
4. **Hypothesis before code on debugging.** Write the failing log/diff hypothesis before editing code.
5. **License hygiene is mechanical.** Check [`THIRD_PARTY.md`](./THIRD_PARTY.md) before reading or citing external projects. GPL, MAME/Angrylion, and unlicensed reference trees NEVER enter `src/` or agent-copyable paths (`ref/` is gitignored and study-only). No third-party emulator/RDP forks or C++ FFI plugins (`EdgeWalker`, our M6 Vulkan compute LLE RDP, is 100% bespoke).
6. **Mandatory file-top attribution.** Every Rust or Markdown file relying on external documentation or code MUST carry a top-of-file credit header per [`ATTRIBUTION.md`](./ATTRIBUTION.md).
7. **Maintain the ruled-out ledger.** Read and update [`STATUS.md`](./STATUS.md) every session so dead ends are never retried.
8. **Human gates stay human.** Never re-bless a golden from our own output without an external oracle, and never declare a milestone complete without Dave's review of the receipt.

---

## 2. Session Startup Checklist

Before writing code or spawning subagents, every session MUST execute:
1. Read [`STATUS.md`](./STATUS.md) to inspect the current active milestone and ruled-out ledger.
2. Read [`AGENTS.md`](./AGENTS.md) and [`docs/ORCHESTRATOR-HANDOFF.md`](./docs/ORCHESTRATOR-HANDOFF.md).
3. Verify clean branch and git status.
4. Run the validation gate (see §6) to ensure the baseline is 100% green.

---

## 3. Architecture & Module Map

- **Library-first core:** `graycart-n64` lib has **zero** GUI dependencies (`winit`/`wgpu`/`egui`/`cpal` belong only in a future binary frontend).
- **Single source of truth Bus:** Owns RDRAM + bit-packed 9th-bit side array (`hidden_bits`), `sp_dmem`, `sp_imem`, `sp`, MI / RI / PI / SI / PIF / VI / AI, and ISViewer (`0x13FF_0000..=0x13FF_FFFF`).
- **CPU & RSP cores:** `CpuBackend` / `InterpCpu`, `RspBackend` / `InterpRsp`.
- **RDP & `EdgeWalker`:** `RdpBackend` / `CpuRdpLle` + future bespoke Vulkan compute **`EdgeWalker`** (100% bespoke in Rust).
- **Scheduler:** Exact `2/3` RCP rational scheduler, zero floats, `serde` determinism enabled from M0.
- **Debugger & UI Standard:** [`docs/13-debugger-and-ui-architecture.md`](./docs/13-debugger-and-ui-architecture.md) (Departure Mono + `btop`-inspired Machine Monitor).
- **Two-tier accuracy contract:** Default `N64Config::accurate()` is strictly hardware-faithful LLE (and what CI tests). Opt-in enhancements (`EnhancementConfig`, M11) live strictly behind explicit flags.
- **Tests layout:** Unit tests live in `src/<module>/tests.rs` (or dedicated test files) via `#[cfg(test)] mod tests;` — never inline in production files. Integration tests live under `tests/`.
- **Forbidden in git:** Never commit `carts/`, `ref/`, `tests/fixtures/fetched/`, `.z64`/`.n64`/`.v64` commercial ROMs, `.sav`/`.eep`/`.sra`/`.fla` saves, or PIF/CIC/IPL dumps.

---

## 4. Orchestrator + Maximally Parallel Cheap Sub-Agent Playbook

Per Dave's explicit directive:
- **Orchestrator Role ("The Boss"):** The top-level agent acts strictly as Orchestrator. Design thin interface seams, fan out implementation across **maximally parallel subagents using the cheapest capable model (`Model: "flash_lite"`)**, enforce **strictly disjoint file scopes** per worker so parallel workers never collide, perform **line-by-line hardware-spec diff review** on every worker's changes before merging, run the full validation + fixture gates, update `STATUS.md`, and report a **Progress Board + copy-paste Windows PowerShell commands** after every wave/milestone.
- **When to use `flash_lite` vs. Orchestrator Model:**
  - `flash_lite`: Parallel well-scoped implementation tasks (single opcode families, peripheral register blocks, unit test suites, docs, mechanical refactors).
  - Orchestrator Model: Seam design, subtle hardware debugging, final diff audit, integration wiring, release verification.

### Reusable Sub-Agent Brief Template

Every subagent task dispatch MUST provide this contract:

```markdown
## Task: <Milestone & Family Name>

### Standing Rules
- Read AGENTS.md, ATTRIBUTION.md, THIRD_PARTY.md.
- Trap, don't stub: any unimplemented path must return Err(Trap::...). Never return 0 or NOP.

### Allowed Files (STRICT)
- `src/<mod>/<file>.rs` (only these files may be touched)

### Hardware Specification & Pseudocode
<NEC VR4300 / RCP hardware pseudocode & spec citations>

### Edge Test Vectors
1. <Input -> Expected output / trap>

### Validation Gate
Run and paste exact output:
- cargo fmt --check
- cargo clippy --all-targets --all-features -- -D warnings
- RUST_MIN_STACK=16777216 cargo test <filter>
```

---

## 5. Adding, Removing, or Refactoring Features Safely

- **Adding features:** Ensure accurate LLE default behavior; place opt-in enhancements strictly behind `EnhancementConfig`; add top-of-file credit headers (`ATTRIBUTION.md`); check license hygiene (`THIRD_PARTY.md`); put unit tests in `src/<mod>/tests.rs` (or `*_tests.rs`); maintain `serde` round-trip compatibility; follow trap-don't-stub; record receipt in `STATUS.md`.
- **Removing or refactoring features:** Never silently degrade `N64Config::accurate()`; replace removed hardware stubs with explicit `Trap`; update `STATUS.md` Ruled-Out ledger so dead ends are never retried; verify no regressions on CI + Dillonb + systemtest + local carts.

---

## 6. Required Verification Gates (Windows PowerShell & POSIX)

Before finishing any task, run and paste verbatim output counts for:

```powershell
$env:RUST_MIN_STACK = "16777216"

# 1. ROM-free workspace CI gate
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# 2. Dillonb 26-ROM CPU conformance matrix (requires `.\scripts\fetch-suites.ps1 -Suite dillonb`)
cargo test --release --test roms matrix_dillonb_all_26 -- --ignored --nocapture

# 3. Local-only commercial cart smoke probe (never commit ROMs or `..\carts\n64`)
cargo test --release --test roms probe_local_carts -- --ignored --nocapture

# 4. Forbidden-binary git hygiene check
git ls-files | Select-String -Pattern '\.(z64|n64|v64|sav|eep|sra|fla|bin)$|^carts/|^ref/|tests/fixtures/fetched/'
```

*(Note: `graycart-n64` is a git submodule under the parent `graycart` repo, so `git` commands and probes reading `..\carts\n64` touch paths outside the submodule root. Keep forbidden binaries out of `git ls-files` at all times.)*
