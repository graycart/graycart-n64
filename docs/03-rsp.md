<!--
Cited: N64brew Reality Signal Processor; SGI Nintendo Ultra64 RSP Programmer's Guide
  (community host ultra64.ca — cite carefully, minimal excerpt); Copetti N64 RSP section
URLs: https://n64brew.dev/wiki/Reality_Signal_Processor
  · https://ultra64.ca/files/documentation/silicon-graphics/SGI_Nintendo_64_RSP_Programmers_Guide.pdf
Note: RSP overview for graycart-n64; not a microcode listing dump. Retrieved 2026-09-13.
-->

# graycart-n64 — RSP (Reality Signal Processor)

**Owns:** SU + VU model, IMEM/DMEM, task/DMA interface, microcode loading, HLE vs LLE posture.  
**Does not own:** RDP pipe internals ([04](./04-rdp.md)), AI DAC ([05](./05-audio.md)).  
**Provenance:** [provenance/rsp/](./provenance/rsp/)

---

## 1. Role

The RSP is a **programmable** second CPU inside the RCP (~**62.5 MHz**). Commercial games almost never write “raw” RSP asm day-to-day — they ship Nintendo/SGI **microcodes** (Fast3D, F3DEX, F3DEX2, audio ucodes, …). Emulators either:

- **LLE:** run the real microcode on an RSP core (interpreter or dynarec), or  
- **HLE:** recognize the task / ucode checksum and implement the algorithm in host code.

Graycart policy: **support LLE as the accuracy path**; HLE may exist for speed but must be explicitly labeled ([09](./09-graphics-plugin-accuracy.md)).

---

## 2. Hardware blocks

| Block | Spec (n64brew / manuals) |
|-------|---------------------------|
| **Scalar Unit (SU)** | Cut-down MIPS; 32×32-bit GPRs; subset of MIPS III (no full multiply/divide/64-bit/interrupt model like VR4300) |
| **Vector Unit (VU)** | COP2; 32×128-bit regs as 8×16-bit lanes; rich fixed-point SIMD |
| **IMEM** | 4 KiB instruction mem |
| **DMEM** | 4 KiB data mem |
| **DMA** | RDRAM ↔ IMEM/DMEM; CPU or RSP driven |
| **Dual-issue** | SU + VU op in one cycle when patterned correctly; shared IF/RD/WB stages |

Addresses inside RSP are physical to IMEM/DMEM; microcodes often use a **segment table** in DMEM to translate segment addresses from display lists into RDRAM physical addresses.

---

## 3. CPU-visible interface

Mapped under `0x0400_0000` (DMEM/IMEM) and `0x0404_0000` (regs). Important reg groups:

- DMA: DRAM addr, DMEM/IMEM addr, read/write length, busy/full status  
- Status / halt / broke / interrupt bits (wire into **MI SP** interrupt)  
- Semaphore  
- PC  
- IMEM BIST (ignore until needed)

Both VR4300 and RSP (via COP0) can poke RDP; RSP typically finishes geometry by emitting RDP commands.

Exact offsets: provenance [rsp/n64brew-rsp-interface.md](./provenance/rsp/n64brew-rsp-interface.md) + SGI RSP Programmer’s Guide tables (cite, don’t paste).

---

## 4. Tasks

libultra / modern homebrew pattern:

1. CPU prepares a **task** structure in RDRAM (ucode boot, data pointers, flags).  
2. CPU loads ucode into IMEM (or points boot microcode), starts RSP.  
3. RSP DMAs inputs, runs, DMAs outputs / feeds RDP, signals break/interrupt.  
4. OS overlays next graphics or audio task.

Emulator HLE hooks often match on **ucode text checksum** or known boot patterns. LLE ignores that and just runs bytes.

---

## 5. Vector ISA (implementation notes)

- Element selects / broadcast / quarters — get lane swizzles wrong and every matrix dies quietly.  
- Accumulators are wider than 16-bit (brew: 48-bit class final accumulator) — clamping modes matter.  
- Compare / store conditional vector ops feed clip/cull paths.  
- Dual-issue hazards / VU pipeline stalls are a **timing** concern; functional LLE can start in-order retiring both pipes with conservative stalls (**TBD** exact).

Primary learning refs: RSP Programmer’s Guide (reconstruction); cen64 / ares / parallel-RSP as **secondary**.

---

## 6. Microcode landscape (non-exhaustive)

| Family | Role | Emulator note |
|--------|------|---------------|
| Fast3D / Fast3D.DRAM / .FIFO | Early Nintendo geometry | HLE common historically |
| F3DEX / F3DEX2 | Improved geometry, homebrew-friendly | Still HLE’d widely; LLE preferred long-term |
| L3DEX* | Line microcodes | |
| S2DEX | 2D sprites | |
| Audio (asp*) | ADPCM, mix, resample | Often HLE’d; LLE needed for obscure ucodes |

**Unknown custom ucodes** (rare retail, more homebrew) **force LLE**.

---

## 7. HLE vs LLE decision matrix

| Mode | When to use | Risk |
|------|-------------|------|
| **LLE interpreter** | Accuracy bring-up, angrylion pairing | Slow |
| **LLE dynarec** (parallel-RSP class) | Playable accuracy | Complex; still secondary-inspired |
| **HLE graphics** | Fast path / enhancements | Breaks custom ucode; GBI mismatches |
| **HLE audio** | Early sound | Wrong ucode → garbage / hangs |

Angrylion-class **LLE RDP requires LLE RSP** (HLE RSP will not drive it correctly). See [09](./09-graphics-plugin-accuracy.md).

---

## 8. Bring-up order

1. DMEM/IMEM CPU R/W + DMA R/W lengths/status  
2. SU interpreter (enough to run tiny homebrew RSP programs)  
3. VU ops used by Fast3D/F3DEX2 (expand via tests)  
4. Task boot from libultra/libdragon  
5. Wire SP interrupt + status bits  
6. Optional HLE overlays behind feature flags  
7. Dynarec stretch  

---

## 9. Acceptance hooks

| Gate | Oracle |
|------|--------|
| DMA/SU | Dillonb RSP tests (as they land); systemtest RSP section |
| VU | Dedicated VU ROMs / ares tests (**secondary** until we vendor homebrew) |
| Integration | RDP triangle from LLE path vs angrylion dump compare ([07](./07-test-strategy.md)) |

---

## 10. TBD

- Exact dual-issue packing rules and illegal pairings.  
- Complete VU opcode timing.  
- Official GBI documentation legal excerpt strategy (prefer homebrew GBI headers under permissive licenses).  
- 64DD / specialty ucodes.
