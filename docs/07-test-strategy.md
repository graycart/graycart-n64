<!--
Cited: lemmy-64/n64-systemtest; Dillonb/n64-tests; PeterLemon/N64; angrylion-plus;
  parallel-rdp; graycart-gba docs/07+11 posture
URLs: https://github.com/lemmy-64/n64-systemtest · https://github.com/Dillonb/n64-tests
Note: accuracy/CI contract; does not reprint suite sources. Retrieved 2026-09-13.
-->

# graycart-n64 — Test strategy

**Audience:** Dave + implementers. Accuracy / CI **contract**.  
**Concrete inventory:** [11-test-apparatus.md](./11-test-apparatus.md)  
**Phase freeze:** [12-test-gates.md](./12-test-gates.md)  
**Provenance:** [provenance/tests/](./provenance/tests/)

---

## 1. Goals

1. Every phase exits on a **named oracle**, not “clippy green.”  
2. Default CI stays **ROM-free / dump-free** (`cargo test` without `--ignored`).  
3. Prefer **homebrew / MIT-friendly** suites; commercial smoke is local-only.  
4. Separate **CPU**, **RSP**, **RDP**, **bus**, and **timing** evidence.  
5. Match graycart-gba culture: Outcome enum, ignored matrices, LICENSE tables.

---

## 2. Layers (L0–L6)

| Layer | What | When |
|-------|------|------|
| **L0** | `fmt` / `clippy -D warnings` / unit tests | P0+ |
| **L1** | CPU instruction / COP0 unit + young ROM CPU | P1 |
| **L2** | Bus map / PI / SI unit + memory suite | P2 |
| **L3** | RSP SU/VU + DMA | P3–P4 |
| **L4** | RDP command goldens vs angrylion dumps | P5+ |
| **L5** | systemtest base board thresholds | P6–P8 |
| **L6** | Timing / stress / commercial smoke | late |

---

## 3. Primary suites

### 3.1 lemmy-64/n64-systemtest

- Broad: COP0 64-bit, LL/SC, exceptions, TLB, multi-size mem, RSP, quirks.  
- Extra features: timing, cycle, cop0hazard (opt-in).  
- Stress targets separate.  
- Uses **libdragon open IPL3** — good for legal CI once soft-boot works.  
- License: check upstream LICENSE before vendoring binaries.

### 3.2 Dillonb/n64-tests

- Young-emulator friendly; can skip boot (`PC=0x80001000` + cart copy).  
- r30 sentinel: `-1` pass, `>0` fail id.  
- Ideal **first ROM gate**.

### 3.3 PeterLemon / krom N64 demos + CPUTest

- Bare-metal visuals / CPU corners.  
- Good RDP smoke + hashed frames.

### 3.4 RDP references

- Hand-crafted display lists compared to **angrylion** / **paraLLEl-RDP** bitmaps.  
- Vendor **golden PNMs/PNGs**, not engine source.

### 3.5 Secondary

ares test vectors, mupen TAS traces, cen64 — cross-check only.

---

## 4. Outcome model (aspirational)

```text
Pass | Fail { id, pc, … } | Timeout | Unimplemented | BootFail
```

Harness must distinguish **emulator crash** from **suite fail**.

---

## 5. CI policy

| Job | Runs | Notes |
|-----|------|-------|
| default | L0 units | No `--ignored` |
| `conformance` (manual/dispatch) | ignored ROM matrix | Optional artifacts |
| never | commercial carts; PIF dumps | |

Audio: **soft WAV gate** until AI owner hardens (gba precedent).

---

## 6. Phase ↔ gate sketch

Reconcile with [PHASES.md](./PHASES.md) (PHASES wins on numbering):

| Phase | Must oracle |
|-------|-------------|
| P0 | CI green, fixtures README |
| P1 | Dillonb CPU or systemtest CPU subset |
| P2 | Memory access tests |
| P3 | RSP DMA + SU smoke |
| P4 | VU subset / task boot |
| P5 | RDP fill/tri golden |
| P6 | AI beep + IRQ |
| P7 | EEPROM/SRAM roundtrip |
| P8 | systemtest base threshold |
| P9 | Frontend playable homebrew |
| P10+ | Timing / LLE polish |

---

## 7. Anti-patterns

- Claiming phase done on units alone ([12](./12-test-gates.md)).  
- Title hacks to pass one retail.  
- Vendoring dumps “just for CI.”  
- Silent skip of failing matrix rows.

---

## 8. TBD

- Exact systemtest pass counts for each phase.  
- Automation for interactive suites (if any).  
- RDP golden corpus license + generation scripts ownership.
