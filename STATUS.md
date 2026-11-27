# Graycart N64 — Accuracy & Ruled-Out Ledger (`STATUS.md`)

Loaded into every implementation session per [`AGENTS.md`](./AGENTS.md) and [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) §5.6.
Milestones advance only on pasted, verifiable test receipts reviewed by Dave.

## Active Milestone

- **Current:** `M0 — Scaffold` (in progress)
- **Bespoke Vulkan Compute LLE RDP Name:** **`EdgeWalker`** (zero third-party RDP forks/FFI)
- **Pre-code gate:** Accepted by Dave on 2026-11-27

## Milestone Ledger (M0–M12)

| Milestone | Status | Gate Receipt Summary |
|---|---|---|
| **M0 — Scaffold** | In progress | Crate bootstrap, license boundary, bus/9th-bit RDRAM, scheduler + Count wrap test, ROM loader, headless ISViewer runner, CI suite plumbing |
| **M1 — CPU functional** | Not started | — |
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

*(Populated at the end of each task with exact command outputs.)*

## Ruled Out

Approaches, claims, or hypotheses already checked and rejected so no session retries them:

| Date | Area | Ruled Out | Why / Replacement |
|---|---|---|---|
| 2026-11-27 | Audio (`docs/05-audio.md`) | "AI delayed-carry bug" as a single named hardware bug | No primary source uses that name; conflated distinct behaviors. Track separately: (1) ares v129 queued-DMA length latching bug, (2) AI 8 KiB (`0x2000`) boundary DMA address quirk (libdragon/n64brew), and (3) RSP `VADDC`/accumulator carry semantics. |
| 2026-11-27 | Scheduler | Cooperative coroutines (`libco`-style) & per-cycle pipeline at M0 | Coroutines fight Rust `&mut Bus` ownership; full pipeline is an M10 hardening target. Use single-threaded deterministic event-queue slice scheduler with exact 2/3 RCP rational accumulator. |
| 2026-11-27 | GPU RDP | Linking `paraLLEl-RDP` via C++ FFI or forking third-party RDP code | Dave's architectural call: zero third-party RDP forks/FFI. Build bespoke `CpuRdpLle` (M5 oracle) + bespoke Vulkan compute LLE RDP **`EdgeWalker`** (M6 parity → M11 enhancements). |
| 2026-11-27 | Test vendoring | Committing `Dillonb/n64-tests` or `240p Test Suite` ROMs in git | Dillonb has no license (never redistribute); 240p Suite is GPL-2.0+ (incompatible with MIT crate). Fetch-and-run only via `scripts/fetch-suites.sh` into gitignored `tests/fixtures/fetched/`. |
