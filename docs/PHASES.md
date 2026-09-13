# graycart-n64 — Phase checklist

**Audience:** Dave (review gate) + future implementation agents.  
**Full plan:** [08-implementation-plan.md](./08-implementation-plan.md)  
**Gates:** [07](./07-test-strategy.md) · [11](./11-test-apparatus.md) · [12](./12-test-gates.md)  
**Status:** research pack ready for review — **no coding until this checklist is accepted**.

**Hard requirement:** file-top attribution ([`ATTRIBUTION.md`](./ATTRIBUTION.md), [`AGENTS.md`](./AGENTS.md)). No PIF/CIC/ROM dumps in git.

---

## Legend

| Mark | Meaning |
|------|---------|
| ☐ | Not started |
| ▢ | In progress |
| ☑ | Done (gate green) |
| — | N/A / deferred |

---

## P0 — Scaffold

| | Item |
|--|------|
| ☐ | Crate `graycart-n64` bootstrapped (lib + thin binary stub) |
| ☐ | Module skeleton per [08](./08-implementation-plan.md) §2.2 |
| ☐ | CI: fmt + clippy `-D warnings` + `cargo test` (ubuntu/macOS/windows); `fail-fast: false` |
| ☐ | CI must **not** run `cargo test -- --ignored` |
| ☐ | `tests/fixtures/README.md` license table; empty suite dirs |
| ☐ | `carts/` gitignored; no firmware dumps |
| ☐ | Headless CLI stub (`--frames`, `--version`) |
| ☐ | `AGENTS.md` + `ATTRIBUTION.md` in-repo (may symlink/copy from `docs/`) |

**Exit gate:** default `cargo test` green.  
**Dave accept:** ☐

---

## P1 — CPU (VR4300 interpreter)

| | Item |
|--|------|
| ☐ | MIPS III integer interpreter + delay slots |
| ☐ | COP0 Status/Cause/EPC + exception entry |
| ☐ | KSEG0/1 translate; basic Count/Compare |
| ☐ | Unit/property decode+ALU; Dillonb CPU or equivalent |

**Exit gate (must):** Dillonb `n64-tests` CPU **PASS** (soft-boot OK) **or** agreed systemtest CPU subset.  
**Stretch:** LL/SC naive; DMFC0/DMTC0.  
**Dave accept:** ☐

---

## P2 — Bus / PI / MI / SI stubs

| | Item |
|--|------|
| ☐ | Physical decode table; freeze unmapped windows |
| ☐ | RDRAM R/W; cart ROM map; endian load |
| ☐ | MI interrupt mask/status wiring |
| ☐ | PI DMA ROM→RDRAM; SI regs + PIF RAM bytes |
| ☐ | Cached MMIO → hard error/freeze (debug) |

**Exit gate (must):** memory access suite subset green; PI DMA unit tests.  
**Dave accept:** ☐

---

## P3 — RSP DMA + Scalar Unit

| | Item |
|--|------|
| ☐ | IMEM/DMEM CPU access + DMA engine |
| ☐ | SU interpreter subset; halt/broke/status |
| ☐ | SP → MI interrupt |

**Exit gate (must):** RSP DMA + tiny SU program PASS.  
**Dave accept:** ☐

---

## P4 — RSP Vector + tasks

| | Item |
|--|------|
| ☐ | VU ops needed for one stock ucode path |
| ☐ | libultra/libdragon-style task boot  
| ☐ | Feature-flag stub for HLE graphics ucode (off by default) |

**Exit gate (must):** task runs and signals break; VU unit tests for wired ops.  
**Dave accept:** ☐

---

## P5 — RDP LLE basics + VI

| | Item |
|--|------|
| ☐ | Command parser; FillRect/Tri; FB write |
| ☐ | TMEM load + textured rect smoke |
| ☐ | VI present path + VI IRQ |
| ☐ | Headless `--frames N --hash-out` |

**Exit gate (must):** ≥3 homebrew RDP demos hashed; optional angrylion compare on 1 DL.  
**Dave accept:** ☐

---

## P6 — Audio (AI)

| | Item |
|--|------|
| ☐ | AI DMA double-buffer + DACRATE pacing |
| ☐ | IRQ on buffer start; STATUS ack |
| ☐ | Delayed-carry bug reproduced or flagged |
| ☐ | Host PCM drain |

**Exit gate (must):** CPU-fed sine/beep smoke; soft WAV gate.  
**Dave accept:** ☐

---

## P7 — PIF joybus + saves

| | Item |
|--|------|
| ☐ | Joybus HLE controllers |
| ☐ | EEPROM + SRAM backends; Flash stub or full |
| ☐ | Expansion Pak option |
| ☐ | Never vendor dumps |

**Exit gate (must):** controller readable; one save type roundtrip.  
**Dave accept:** ☐

---

## P8 — systemtest + timing start

| | Item |
|--|------|
| ☐ | n64-systemtest base board at recorded threshold |
| ☐ | Coarse scheduler contention improvements |
| ☐ | TLB suite progress logged |

**Exit gate (must):** recorded systemtest threshold (not necessarily 100%).  
**Dave accept:** ☐

---

## P9 — Frontend playable (homebrew)

| | Item |
|--|------|
| ☐ | winit/wgpu/egui/cpal posture; core≠GUI |
| ☐ | ROM picker, reset, save/load |
| ☐ | Docs: obtain-your-own firmware notes |
| ☐ | First SemVer tag |

**Exit gate (must):** playable homebrew path; CI still dump-free.  
**Dave accept:** ☐

---

## P10 — LLE graphics hardening

| | Item |
|--|------|
| ☐ | angrylion bit-exact corpus growth |
| ☐ | Combiner/blender/z/coverage edges |
| ☐ | Framebuffer effects coherency |

**Exit gate (must):** agreed golden board green.  
**Dave accept:** ☐

---

## P11 — Performance backends

| | Item |
|--|------|
| ☐ | CPU dynarec experimental **or** RSP dynarec |
| ☐ | Optional paraLLEl-RDP-class GPU LLE integration study (license!) |
| ☐ | HLE paths remain optional/explicit |

**Exit gate (must):** interpreter still default for CI accuracy jobs.  
**Dave accept:** ☐

---

## P12 — Family API + umbrella

| | Item |
|--|------|
| ☐ | Implement `GraycartCore` seam ([10](./10-family-api-and-reuse.md)) |
| ☐ | Umbrella submodule pin bump |
| ☐ | Conformance doc published |

**Exit gate (must):** host can drive core headlessly via family API.  
**Dave accept:** ☐

---

## Parallel streams cheat sheet

Full: [08 §4](./08-implementation-plan.md#4-parallel-streams).

---

## Dave pre-code review (sign-off)

- [ ] Phase order + gate table accepted  
- [ ] HLE/LLE + dump policy accepted  
- [ ] SemVer + family API accepted  
- [ ] Hardest-parts list acknowledged (RSP/RDP/timing/TLB)  
- [ ] **Coding authorized** (date / note): _______________
