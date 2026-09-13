<!--
Cited: docs 00–07, 09–12; graycart-gba 08 posture; graycart-family core API
Note: build contract for graycart-n64; research gate before coding. Retrieved 2026-09-13.
-->

# graycart-n64 — Implementation plan

**Audience:** Dave (review before any coding) + future implementation agents.  
**Repo:** [graycart/graycart-n64](https://github.com/graycart/graycart-n64)  
**Checklist twin:** [PHASES.md](./PHASES.md)  
**Research index:** [README.md](./README.md)  
**Family API:** [10](./10-family-api-and-reuse.md) · store `docs/graycart-family/`

This plan is the **build contract**. Hardware behavior lives in [00](./00-architecture-overview.md)–[06](./06-cart-cic-pif-saves.md); oracles in [07](./07-test-strategy.md)/[11](./11-test-apparatus.md). Mark **TBD** — do not invent silicon.

---

## 1. Goals and non-goals

### Goals

1. Bootstrap independently `cargo test`-able N64 core with graycart culture.  
2. Phase-gated accuracy with named suites.  
3. CI green without dumps/ROMs.  
4. Parallel streams with clear file ownership.  
5. Honest **HLE vs LLE** seams for RSP/RDP ([09](./09-graphics-plugin-accuracy.md)).  
6. CPU **interpreter first**; dynarec behind a trait when playable LLE demands it.

### Non-goals (until promoted)

- Pixel-perfect retail as merge gate.  
- Bit-exact audio early.  
- 64DD.  
- Netplay.  
- Shipping enhancement upscalers as default accuracy mode.  
- Coding before Dave accepts §7.

### Attribution (hard)

File-top credits per [`ATTRIBUTION.md`](./ATTRIBUTION.md). No dumps.

---

## 2. Target architecture

### 2.1 Crate / host split

| Layer | Owns | Must not own |
|-------|------|--------------|
| **lib** | VR4300, bus, RCP peripherals, RSP, RDP, PIF HLE, cart | winit/wgpu/egui/cpal |
| **bin** | Window, devices, UI | TLB, RDP hazards |

Public surface (align [10](./10-family-api-and-reuse.md)): load ROM, optional firmware slots, reset, run frames/cycles, framebuffer, PCM drain, buttons, battery image, savestate blob.

### 2.2 Module map

```text
src/
  lib.rs
  cpu/          # interpreter (+ future dynarec/)
  bus/
  mi/ vi/ ai/ pi/ si/ ri/
  rsp/          # lle/ + optional hle/
  rdp/          # lle/ + plugin trait
  pif/          # HLE only in-tree
  cart/
  schedule/
  frontend/     # binary
```

### 2.3 Backend traits (sketch)

```rust
trait CpuBackend { fn step(&mut self, bus: &mut Bus) -> Cycles; }
trait RspBackend { fn run(&mut self, bus: &mut Bus, until: RspStop); }
trait RdpBackend { fn submit(&mut self, cmds: &[u64], mem: &mut Rdram); }
```

Default: `InterpCpu`, `InterpRsp`, `CpuRdpLle` (slow). Features: `rsp-hle`, `rdp-hle` (explicit).

---

## 3. Phase summary

See [PHASES.md](./PHASES.md) for checkboxes. Narrative:

| Phase | Intent |
|-------|--------|
| **P0** | Scaffold + CI + attribution |
| **P1** | VR4300 interpreter + COP0 basics |
| **P2** | Bus map + PI ROM + MI/SI stubs |
| **P3** | RSP DMA + SU |
| **P4** | RSP VU + task boot |
| **P5** | RDP LLE basics + VI present |
| **P6** | AI audio path |
| **P7** | PIF joybus + saves |
| **P8** | systemtest thresholds + timing start |
| **P9** | Frontend playable homebrew |
| **P10** | LLE hardening (angrylion goldens) |
| **P11** | Optional dynarec / RSP dynarec |
| **P12** | Family API extract + umbrella pin |

---

## 4. Parallel streams

| ID | Start when | Must not touch |
|----|------------|----------------|
| S0 CI/fixtures | Day 0 post-accept | ISA decode |
| S1 CPU | After P0 interfaces | RDP |
| S2 Bus/PI | After P0 | RSP VU |
| S3 RSP | After DMEM map | Frontend |
| S4 RDP | After RDRAM FB write path | Cart Flash FSM |
| S5 AI | After scheduler timebase | TLB random |
| S6 PIF/saves | After SI regs | RDP combiner |
| S7 Frontend | After FB+PCM APIs | Waitstate tables |
| S8 Dynarec | After P8 interpreter solid | HLE-only hacks |

---

## 5. SemVer / release

- Start `0.0.1`; first playable tag `0.1.0`.  
- Patch on user-visible fixes; tag `vX.Y.Z` == crate version.  
- Umbrella submodule pin on separate `graycart` PR.

---

## 6. Honest hardest parts

1. **RSP LLE** — VU correctness + dual-issue.  
2. **RDP LLE** — hazards, coverage, TMEM.  
3. **Timing/UMA** — multi-master RDRAM.  
4. **TLB/cache** — freeze cases, `CACHE`, self-mod.  
5. **PIF/CIC** — boot without dumps.

---

## 7. Acceptance criteria (Dave review gate)

Copy to sign-off:

- [ ] Phase order + gates accepted (this file ↔ PHASES ↔ 07/11/12)  
- [ ] HLE vs LLE policy accepted ([09](./09-graphics-plugin-accuracy.md))  
- [ ] Dump/ROM policy accepted  
- [ ] SemVer + family API direction accepted ([10](./10-family-api-and-reuse.md))  
- [ ] Parallel stream ownership accepted  
- [ ] Known TBD list accepted as non-blocking for P0–P2  
- [ ] **Coding authorized** (date / note): _______________

**Status:** research pack ready for review — **no coding until accepted**.
