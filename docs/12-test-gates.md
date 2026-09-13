<!--
Cited: PHASES.md exit gates; 07-test-strategy.md; 11-test-apparatus.md
Note: operational freeze + CI gate map — amends PHASES, does not replace 07/11.
-->

# graycart-n64 — Test gates (phase freeze)

**Audience:** Dave (enforce) + implementers.  
**Does not replace:** [07](./07-test-strategy.md) · [11](./11-test-apparatus.md) · [PHASES.md](./PHASES.md)

---

## Hard rule

**No P3+ merge (RSP or later) until:**

1. Named suite gates for **P1 CPU** and **P2 bus** are written here and in PHASES, **and**  
2. **Harness + fixture plumbing is merged** on `main`: shared `tests/roms/` runner, ignored matrices, LICENSE/path stubs — even if `.z64` binaries are still absent.

**Anti-pattern:** `clippy` + default `cargo test` green is **not** a phase gate beyond P0 hygiene.

---

## Gate table

Legend — **Blocker:** `OPEN` = blocks P3+; `PLUMB` = apparatus must exist; `ROM` = PASS once fixtures present; `LATER` = not a P3 freeze item.

| Phase | Suite(s) / oracle | Pass criteria | CI job | Blocker |
|-------|-------------------|---------------|--------|---------|
| **P0** | fmt/clippy/units; fixture README | All green; no `--ignored` | default | — |
| **P1** | Dillonb CPU (or agreed) | Matrix **PASS** | stubs default; ROM ignored | **OPEN** until plumbing |
| **P2** | Mem/PI units + suite subset | PASS | same | **OPEN** until plumbing |
| **P3** | RSP DMA+SU | Per PHASES | ignored ROM OK | **Frozen** until P1+P2 plumbing |
| **P4–P7** | PHASES | As written | ignored | **LATER** |
| **P8** | systemtest threshold | Recorded board | optional conformance | **LATER** |
| **P9–P12** | PHASES | As written | — | **LATER** |

---

## Ordered workstream

| Step | Work | Done when |
|------|------|-----------|
| **(A)** | Docs 07/11/12 + PHASES accepted | Dave OK |
| **(B)** | Harness + LICENSE stubs PR | Merged; CI ROM-free |
| **(C)** | Vendor/fetch allowed homebrew | Binaries obtainable; still ignored in default CI |
| **(D)** | Enable CPU matrix | Real PASS/FAIL when ROM present |
| **(E)** | Enable bus/mem matrix | P2 enforceable |
| **(F)** | Resume **P3** | Only after **(B)**; prefer D+E runnable |

**Minimum to lift freeze:** **(A)+(B)**.

---

## Dave checklist

- [ ] Freeze: no P3+ until **(B)** merges  
- [ ] Reject “units green ⇒ next phase”  
- [ ] After D/E green, tick PHASES P1/P2  

---

## Source index

| Doc | Role |
|-----|------|
| [PHASES.md](./PHASES.md) | Checklist |
| [07](./07-test-strategy.md) | Contract |
| [11](./11-test-apparatus.md) | Suites/harness |
| [08](./08-implementation-plan.md) | Build contract |
