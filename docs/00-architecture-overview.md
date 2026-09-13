<!--
Cited: N64brew Memory map / RCP / RSP / RDP / PIF-NUS; Copetti Nintendo 64 Architecture;
  NEC VR4300 User's Manual (title+section cites only)
URLs: https://n64brew.dev/wiki/Memory_map · https://www.copetti.org/writings/consoles/nintendo-64/
  · http://n64dev.org/p/U10504EJ7V0UMJ1.pdf
Note: system map for graycart-n64 research; not a manual reprint. Retrieved 2026-09-13.
-->

# graycart-n64 — Architecture overview

Deep system map for a Nintendo 64 emulator. Sibling docs own subsystem detail; this page owns chips, clocks, memory regions, and how **VR4300 / RSP / RDP / peripherals** interact. Cross-links only — do not treat this as a substitute for [01–10](./README.md).

**Repo:** [graycart/graycart-n64](https://github.com/graycart/graycart-n64) (placeholder: LICENSE + README only as of 2026-09-13; no source).  
**Family gold standard:** [graycart-gba docs](https://github.com/graycart/graycart-gba) research pack (provenance discipline).  
**Primary / preferred refs:** [NEC VR4300 UM](http://n64dev.org/p/U10504EJ7V0UMJ1.pdf); [N64brew](https://n64brew.dev/). Secondary: [Copetti N64](https://www.copetti.org/writings/consoles/nintendo-64/), ares / cen64 / mupen64plus / parallel-RDP / angrylion — never as sole authority.

Unknowns / contested timings are marked **TBD** or **contested**. Do not invent HW behavior.

---

## 0. Product goal

1. **Library-first N64 core** matching Graycart engineering culture (CI without dumps, SemVer, core≠host, attribution).
2. **Accuracy with honesty:** LLE for CPU early; RSP/RDP start with clear HLE/LLE seams ([08](./08-implementation-plan.md), [09](./09-graphics-plugin-accuracy.md)).
3. **Test-gated bring-up** via homebrew / torture suites ([07](./07-test-strategy.md), [11](./11-test-apparatus.md)) — commercial titles are smoke, not oracles.

---

## 1. What we are emulating

The Nintendo 64 (codename **NUS**) is a Unified Memory Architecture console:

| Block | Role |
|-------|------|
| **NEC VR4300** | Main CPU ≈ **93.75 MHz**, MIPS III (R4300i-compatible), 5-stage pipeline, I/D cache, TLB, COP0 + COP1 (FPU) |
| **Reality Coprocessor (RCP)** | ≈ **62.5 MHz** SGI chip: **RSP** + **RDP** + memory/peripheral interfaces |
| **RDRAM** | Base **4 MiB** usable (+ Expansion Pak → **8 MiB**); 9-bit physical (9th bit for RDP coverage/z metadata) |
| **PIF-NUS** | Boot (IPL1/2), CIC handshake, joybus (controllers + cart EEPROM), reset |

Everything the CPU does to memory or I/O goes through the **RCP** on the **SysAD** bus. There is no CPU-side DMA engine outside what RCP provides.

**Product scope:** retail cartridge path first. **64DD** is stretch / later. Expansion Pak detection required for titles that hard-require it.

---

## 2. Reality Coprocessor = RSP + RDP + interfaces

```text
                 ┌──────────────────────────────────────────────┐
  Cartridge ──►  │  PI (Parallel / Peripheral Interface)        │
  Controllers─┐  │  SI (Serial Interface) ──► PIF-NUS           │
              │  │  AI (Audio Interface) ──► DAC                │
              │  │  VI (Video Interface) ──► video encoder      │
              │  │  RI (RDRAM Interface) ──► RDRAM banks        │
  VR4300 ═════╪══╡  MI (MIPS Interface) — IRQ aggregate         │
  SysAD       │  │                                              │
              │  │  ┌─────────────┐    XBUS / RDRAM     ┌─────┐ │
              │  │  │ RSP         │ ──────────────────► │ RDP │ │
              │  │  │ SU + VU     │   display lists     │pipe │ │
              │  │  │ IMEM/DMEM   │                     │TMEM │ │
              │  │  └─────────────┘                     └─────┘ │
              └──────────────────────────────────────────────┘
                              ▲
                           RDRAM (UMA)
```

| Peripheral | Base (phys) | Job |
|------------|-------------|-----|
| **MI** | `0x04300000` | Interrupt status/mask; version; init modes |
| **VI** | `0x04400000` | Framebuffer fetch, timing, filter, NTSC/PAL |
| **AI** | `0x04500000` | Stereo PCM DMA to DAC |
| **PI** | `0x04600000` | Cart ROM / SRAM / Flash / 64DD DMA + domains |
| **RI** | `0x04700000` | RDRAM init, refresh, bank status |
| **SI** | `0x04800000` | DMA to PIF RAM; joybus orchestration |

RSP DMEM/IMEM and registers live under `0x04000000+`; RDP command/span regs under `0x04100000` / `0x04200000`. Full tables: [02](./02-bus-memory-dma.md).

---

## 3. Clocks and domains

| Domain | Nominal | Notes |
|--------|---------|-------|
| VR4300 PClock | **93.75 MHz** | Copetti / n64brew consensus |
| RCP | **62.5 MHz** | RSP/RDP/peripherals |
| RDRAM | **~250 MHz** base signalling (effective higher with RSL) | High bandwidth, **high latency** (~hundreds of ns to first word — contested exact) |
| VI | NTSC ≈ **60.15** fields/s class; PAL lower | Exact line/dot counts: see VI notes (**TBD** fine constants in provenance) |

Emulator schedulers typically pick a **common tick** (often RCP cycles or a GCD of CPU/RCP) and convert. Cycle-perfect multi-master UMA is a late accuracy phase ([PHASES](./PHASES.md) timing phase).

---

## 4. Memory model (sketch)

### Virtual (32-bit software view)

| Range | Segment | Mapping |
|-------|---------|---------|
| `0x00000000–0x7FFFFFFF` | KUSEG | TLB |
| `0x80000000–0x9FFFFFFF` | KSEG0 | Direct → phys `addr - 0x80000000`, **cached** |
| `0xA0000000–0xBFFFFFFF` | KSEG1 | Direct → phys `addr - 0xA0000000`, **uncached** |
| `0xC0000000–0xDFFFFFFF` | KSSEG | TLB |
| `0xE0000000–0xFFFFFFFF` | KSEG3 | TLB |

Retail games mostly run in **kernel** mode using **KSEG0** for RDRAM code/data and **KSEG1** for MMIO.

### Physical (RCP-implemented)

| Range | Name |
|-------|------|
| `0x00000000–0x03EFFFFF` | RDRAM |
| `0x03F00000–0x03FFFFFF` | RDRAM registers |
| `0x04000000–0x04001FFF` | RSP DMEM + IMEM (4+4 KiB) |
| `0x04040000+` | RSP regs |
| `0x04100000+` / `0x04200000+` | RDP regs |
| `0x04300000–0x048FFFFF` | MI…SI |
| `0x10000000+` | Cart ROM (via PI) |
| `0x08000000+` | Cart SRAM/Flash (Domain 2) |
| `0x1FC00000+` | PIF ROM (boot only) + PIF RAM |

**Critical HW trap:** cached SysAD access is valid for **RDRAM** only. Cached access to RCP regs / PI / SI **freezes** the CPU on hardware. Emulators that silently “fix” this hide bugs.

Detail: [02](./02-bus-memory-dma.md).

---

## 5. Graphics pipeline (one paragraph)

CPU builds **tasks** / display lists in RDRAM → **RSP** microcode transforms geometry / lighting into RDP command streams (via XBUS or RDRAM) → **RDP** rasterizes into a framebuffer in RDRAM (using TMEM for textures) → **VI** scans the framebuffer out to the DAC/encoder. HLE shortcuts replace RSP microcodes and/or RDP with host GL/Vulkan approximations ([09](./09-graphics-plugin-accuracy.md)).

---

## 6. Audio pipeline (one paragraph)

RSP (or CPU) synthesizes **16-bit stereo** PCM into RDRAM → **AI** DMA feeds the DAC at `VI_clock / (DACRATE+1)`. AI IRQ fires when a buffer **starts** (so software can enqueue the next). AI does **no** mixing/ADPCM — that is microcode/CPU work ([05](./05-audio.md)).

---

## 7. Boot / security sketch

1. PIF releases CPU; **IPL1/IPL2** from PIF ROM run (region baked in).
2. **CIC** ↔ PIF seed / checksum protocol authenticates cart.
3. **IPL3** in cart header (`0x40`…) checksummed; loads game.
4. PIF ROM locked; joybus available for controllers / EEPROM.

**Repo policy:** HLE soft-boot for CI (jump to entry after stubbing IPL3 effects). Optional user-supplied dumps never vendored ([06](./06-cart-cic-pif-saves.md)).

---

## 8. Interrupt fabric

MI aggregates device IRQs (SP, SI, AI, VI, PI, DP, …) into the VR4300 **IP** lines via COP0 Cause. Mask in MI; Status/Cause on CPU. Exact bit tables: provenance `io/` + [02](./02-bus-memory-dma.md).

---

## 9. Emulator orchestration sketch

1. **Event / slice scheduler** in a chosen master clock.
2. Per slice: retire PI/SI/AI/RSP/RDP DMA progress → run CPU until next event → step RSP if tasked → step RDP command drain → sample MI → VI line/field advance.
3. **Bus** (physical map + side effects) is the only memory authority.
4. Plugin seams: `RspBackend` (interpreter / dynarec / HLE ucode) and `RdpBackend` (LLE CPU, LLE GPU, HLE).

---

## 10. Contrasts vs graycart-gba

| | GBA | N64 |
|--|-----|-----|
| CPU | ARM7TDMI ~16.78 MHz | VR4300 ~93.75 MHz MIPS III |
| GPU | Scanline PPU in SoC | Programmable RSP + fixed RDP |
| Memory | Multi-region waitstates | UMA RDRAM via RCP |
| Boot | BIOS SWI / HLE | PIF + CIC + IPL3 |
| Hard parts | Prefetch, DMA FIFO audio | RSP/RDP LLE, TLB/cache, timing |

Reuse **process** (phases, provenance, gates), not silicon code.

---

## 11. Source index

| Topic | Doc |
|-------|-----|
| CPU | [01](./01-cpu-r4300i.md) |
| Bus / DMA | [02](./02-bus-memory-dma.md) |
| RSP | [03](./03-rsp.md) |
| RDP | [04](./04-rdp.md) |
| Audio | [05](./05-audio.md) |
| Cart / PIF | [06](./06-cart-cic-pif-saves.md) |
| Tests | [07](./07-test-strategy.md) · [11](./11-test-apparatus.md) · [12](./12-test-gates.md) |
| Plan | [08](./08-implementation-plan.md) · [PHASES](./PHASES.md) |
| Graphics tradeoffs | [09](./09-graphics-plugin-accuracy.md) |
| Family API | [10](./10-family-api-and-reuse.md) |
