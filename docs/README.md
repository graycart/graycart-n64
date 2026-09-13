# graycart-n64 research docs

Phased research pack for a Nintendo 64 emulator in the [Graycart](https://github.com/graycart/graycart) family. **Review gate before coding** ([08 §7](./08-implementation-plan.md#7-acceptance-criteria-dave-review-gate), [PHASES.md](./PHASES.md)).

**Agent norms + credits:** [`AGENTS.md`](./AGENTS.md) · [`ATTRIBUTION.md`](./ATTRIBUTION.md) — file-header attribution is **mandatory** before any in-repo sources that cite external docs or code (provenance folders alone are not enough).

| | |
|--|--|
| **Target repo** | [graycart/graycart-n64](https://github.com/graycart/graycart-n64) — placeholder LICENSE + README as of research start |
| **Family peers** | [graycart-gba](https://github.com/graycart/graycart-gba) (research-pack gold standard) · [graycart-gb](https://github.com/graycart/graycart-gb) · [graycart-nes](https://github.com/graycart/graycart-nes) · [graycart-snes](https://github.com/graycart/graycart-snes) (sibling-owned; do not edit) |
| **Umbrella** | [graycart/graycart](https://github.com/graycart/graycart) — submodule pin only; not a Cargo workspace |
| **Store partition** | `docs/graycart-n64/` (Project Agent Store mirror) |

### Product goal

1. Build a **library-first N64 core** that models the VR4300 + Reality Coprocessor (RSP + RDP) + MI/PI/SI/AI/VI/RI peripherals well enough for homebrew, torture tests, and eventually commercial titles as smoke.
2. Prefer **honest accuracy tradeoffs**: document HLE vs LLE for RSP/RDP; do not pretend HLE microcode shortcuts are LLE.
3. Align host seams with the Graycart family core API ([`docs/graycart-family/`](../graycart-family/) when present in the Project store).

Semver policy (once code exists): match graycart-gb / graycart-gba — user-visible fix → patch bump + `vX.Y.Z` tag; hold `main` quality bar.

**Hard non-goals for this pack / repo:** commercial ROMs, PIF/CIC/IPL dumps, large copyrighted manual paste. Cite titles + sections; archive **minimal** attributed excerpts under [`provenance/`](./provenance/README.md).

---

## Reading order

0. **[AGENTS.md](./AGENTS.md)** + **[ATTRIBUTION.md](./ATTRIBUTION.md)** — norms and mandatory file-header credits (before any coding).
1. **[00 — Architecture overview](./00-architecture-overview.md)** — RCP (RSP+RDP), VR4300, peripherals, clocks, UMA.
2. Subsystem deep-dives **01–06** (any order after 00; cross-link, don’t duplicate).
3. **[07 — Test strategy](./07-test-strategy.md)** — accuracy / CI contract (aspirational gates).
4. **[11 — Test apparatus](./11-test-apparatus.md)** — concrete suite inventory, phase must-gates, vendoring, harness shape.
5. **[12 — Test gates](./12-test-gates.md)** — phase freeze / merge rules.
6. **[09 — Graphics plugin / accuracy tradeoffs](./09-graphics-plugin-accuracy.md)** — LLE RDP vs HLE microcodes.
7. **[10 — Family API / reuse](./10-family-api-and-reuse.md)** — crate boundary vs gba/gb; host API.
8. **[08 — Implementation plan](./08-implementation-plan.md)** + **[PHASES.md](./PHASES.md)** — phased build after the research gate.

Provenance archives (excerpts, licenses, retrieved dates): **[provenance/](./provenance/README.md)**.

---

## Document index

| Doc | Topic | Status |
|-----|--------|--------|
| [AGENTS.md](./AGENTS.md) | Agent norms | Written |
| [ATTRIBUTION.md](./ATTRIBUTION.md) | File-header credit policy + examples | Written |
| [00-architecture-overview.md](./00-architecture-overview.md) | System architecture map | Written |
| [01-cpu-r4300i.md](./01-cpu-r4300i.md) | VR4300 / MIPS III / TLB / cache / COP0 | Written |
| [02-bus-memory-dma.md](./02-bus-memory-dma.md) | Memory map, DMA, PI/SI/RI | Written |
| [03-rsp.md](./03-rsp.md) | Reality Signal Processor — VU, IMEM/DMEM, tasks | Written |
| [04-rdp.md](./04-rdp.md) | Reality Display Processor — rasterizer, cmds, VI path | Written |
| [05-audio.md](./05-audio.md) | AI + RSP audio microcodes | Written |
| [06-cart-cic-pif-saves.md](./06-cart-cic-pif-saves.md) | Cart, CIC, PIF, EEPROM/SRAM/Flash, controllers | Written |
| [07-test-strategy.md](./07-test-strategy.md) | Suites, traces, CI acceptance | Written |
| [08-implementation-plan.md](./08-implementation-plan.md) | Phased implement/test plan | Written |
| [PHASES.md](./PHASES.md) | Phase checklist / milestones | Written |
| [09-graphics-plugin-accuracy.md](./09-graphics-plugin-accuracy.md) | LLE RDP vs HLE microcode tradeoffs | Written |
| [10-family-api-and-reuse.md](./10-family-api-and-reuse.md) | Graycart family API + reuse notes | Written |
| [11-test-apparatus.md](./11-test-apparatus.md) | Suite inventory, vendoring, harness API | Written |
| [12-test-gates.md](./12-test-gates.md) | Phase freeze + CI gate map | Written |

Broken links here mean a sibling page has not landed yet — keep the href stable.

---

## Provenance areas

| Area | Path | Owning doc |
|------|------|------------|
| Index | [provenance/README.md](./provenance/README.md) | Overview |
| CPU | [provenance/cpu/](./provenance/cpu/) | 01 |
| Bus / DMA / PI·SI·RI | [provenance/bus/](./provenance/bus/) | 02 |
| RSP | [provenance/rsp/](./provenance/rsp/) | 03 |
| RDP | [provenance/rdp/](./provenance/rdp/) | 04 |
| Audio | [provenance/audio/](./provenance/audio/) | 05 |
| Cart / CIC / PIF / saves | [provenance/cart/](./provenance/cart/) | 06 |
| I/O / MI / VI / IRQ | [provenance/io/](./provenance/io/) | 00 · 02 · 05 |
| Tests | [provenance/tests/](./provenance/tests/) | 07 · 11 |

---

## Hardest parts (honest preview)

| Area | Why it hurts |
|------|----------------|
| **RSP LLE** | Dual-issue SU+VU, 4 KiB IMEM/DMEM, task DMA, microcode variety; HLE must name which microcodes |
| **RDP LLE** | Fixed-function pipeline hazards, TMEM tiling, coverage/AA/z 9th-bit metadata; angrylion / paraLLEl-RDP as references |
| **Timing / UMA** | VR4300 stalled behind RCP; RDRAM latency; PI/SI async writes; multi-master contention |
| **TLB + cache** | KSEG0/1 vs TLB maps; cached access only valid for RDRAM; wrong cache → hard freeze on HW |
| **PIF / CIC** | Boot IPL1–3, joybus, anti-piracy challenge — **no dumps in repo**; HLE boot path for CI |

---

## Internal status stubs

Agents may drop short completion notes under `internal/graycart-n64/status-<area>.md` in the Project store when that partition exists.
