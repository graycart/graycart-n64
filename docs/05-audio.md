<!--
Cited: N64brew Audio Interface; Copetti N64 audio notes; libdragon AI docs (secondary)
URL: https://n64brew.dev/wiki/Audio_Interface
Note: AI + RSP audio path; minimal register paraphrase. Retrieved 2026-09-13.
-->

# graycart-n64 — Audio (AI + RSP microcodes)

**Owns:** Audio Interface DMA/DAC programming, IRQ semantics, relationship to RSP audio tasks, host PCM drain.  
**Does not own:** VU details ([03](./03-rsp.md)), frontend device graphs ([10](./10-family-api-and-reuse.md)).  
**Provenance:** [provenance/audio/](./provenance/audio/)

---

## 1. Architecture

```text
 CPU or RSP audio ucode
        │  writes PCM (or compressed→PCM) into RDRAM
        ▼
   AI DMA (double-buffered)
        │  16-bit stereo samples
        ▼
   External DAC (BU9480-class) ← BITRATE / DACRATE clocks from VI domain
        │
        ▼
   Analog out
```

**AI does not mix, ADPCM-decode, or resample in software terms** — it only DMA-feeds the DAC. All synthesis is CPU/RSP work.

---

## 2. Registers (phys base `0x0450_0000`)

| Offset | Name | Role |
|--------|------|------|
| `0x00` | AI_DRAM_ADDR | Next DMA RDRAM address (8-byte aligned; low bits forced 0) |
| `0x04` | AI_LENGTH | Bytes to transfer; reads remaining |
| `0x08` | AI_CONTROL | DMA enable |
| `0x0C` | AI_STATUS | FULL / BUSY / ENABLED / clocks; **write acknowledges IRQ** |
| `0x10` | AI_DACRATE | Sample period vs VI clock: `rate ≈ VI_clk / (DACRATE+1)` |
| `0x14` | AI_BITRATE | I²S bit clock helper |

Example (n64brew): DACRATE `1103` → ~**44136 Hz** on NTSC.

Many AI regs are **write-only** mirrors of LENGTH on read — implement mirrors or break poorly written software.

Provenance: [audio/n64brew-ai.md](./provenance/audio/n64brew-ai.md).

---

## 3. DMA + IRQ semantics (easy to get wrong)

- Double-buffer: enqueue second buffer while first plays (`FULL` when both occupied).  
- **IRQ fires when a transfer starts**, not when it ends — so games can prepare the next buffer in time.  
- IRQ via **MI AI** bit; acknowledge by writing AI_STATUS.

### Delayed-carry bug

If a transfer ends exactly on an **8 KiB (`0x2000`) page boundary**, HW may add `0x2000` to the **next** buffer address. libdragon documents a workaround. Emulators should **reproduce the bug** (accuracy) or offer a compatibility flag — default reproduce, document clearly.

---

## 4. RSP audio microcodes

Retail stacks use RSP tasks to:

- Decode ADPCM / VADPCM  
- Resample  
- Mix channels into AI buffers  

HLE audio plugins traditionally intercept these abi lists. LLE runs the ucode.

| Strategy | Phase |
|----------|-------|
| HLE common asp ucodes | Early playable sound |
| LLE audio tasks | Accuracy / obscure ucodes |
| CPU-only soft mix | Homebrew teaching demos |

---

## 5. Host integration

Core should expose:

- Pull stereo `i16` or float PCM at a negotiated rate  
- Optional soft resampler if AI DACRATE ≠ host device rate  
- Underrun counters for debug (gba lessons apply)

Do not block the emulator thread on the OS audio callback beyond a ring buffer.

---

## 6. Bring-up order

1. AI register file + LENGTH remaining countdown in scheduler time  
2. DMA consume at DACRATE-derived sample pace  
3. MI AI IRQ on buffer start  
4. Soft sine via CPU poke → hear something  
5. Delayed-carry reproduction  
6. RSP audio HLE or LLE  

---

## 7. Acceptance hooks

| Gate | Oracle |
|------|--------|
| Regs | Unit tests LENGTH/STATUS bits |
| IRQ | Home-brew AI paced beep |
| Soft | WAV hash soft-gate (like gba) until hardened |
| Stretch | Retail music vs LLE |

---

## 8. TBD

- Exact VI clock constants NTSC vs PAL for DACRATE math.  
- BITRATE interaction with COUNT in AI_STATUS (brew “believed” BCLK).  
- Full list of HLE audio ucode checksums worth shipping.
