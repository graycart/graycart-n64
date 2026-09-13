<!--
Cited: N64brew Memory map; N64brew Audio/PI/SI pages; Copetti N64 (UMA pedagogy)
URLs: https://n64brew.dev/wiki/Memory_map · https://n64brew.dev/wiki/Audio_Interface
Note: bus/DMA map for emulator bring-up; minimal paraphrase. Retrieved 2026-09-13.
-->

# graycart-n64 — Bus, memory map, DMA (PI / SI / RI)

**Owns:** virtual→physical translation handoff, physical region table, SysAD access-size quirks, PI/SI/RI DMA engines, open-bus / freeze cases.  
**Does not own:** RSP internal DMA detail beyond CPU-visible regs ([03](./03-rsp.md)), AI sample math ([05](./05-audio.md)), PIF protocol ([06](./06-cart-cic-pif-saves.md)).  
**Provenance:** [provenance/bus/](./provenance/bus/)

---

## 1. Who owns the map

The **VR4300 never talks to RDRAM or carts directly**. Physical addresses are decoded by the **RCP**. Misunderstanding this is the #1 structural bug in young N64 emulators.

```text
CPU load/store / fetch
        │  virtual address
        ▼
     MMU/TLB (COP0)
        │  physical address + cache attr
        ▼
     SysAD  ←→  RCP decode
        │
        ├─ RI  → RDRAM
        ├─ SP  → DMEM/IMEM/regs
        ├─ DP  → RDP regs
        ├─ MI/VI/AI/PI/SI regs
        ├─ PI external → cart / DD
        └─ SI external → PIF
```

---

## 2. Virtual map (32-bit)

| VA | Name | Translation |
|----|------|-------------|
| `0000_0000–7FFF_FFFF` | KUSEG | TLB |
| `8000_0000–9FFF_FFFF` | KSEG0 | PA = VA − `8000_0000`, cached |
| `A000_0000–BFFF_FFFF` | KSEG1 | PA = VA − `A000_0000`, uncached |
| `C000_0000–DFFF_FFFF` | KSSEG | TLB |
| `E000_0000–FFFF_FFFF` | KSEG3 | TLB |

Internally addresses are 64-bit capable; 32-bit mode sign-extends. Detail: [01](./01-cpu-r4300i.md).

---

## 3. Physical map (implement this table)

Canonical community table: [N64brew Memory map](https://n64brew.dev/wiki/Memory_map).

| PA start | PA end | Device |
|----------|--------|--------|
| `0000_0000` | `03EF_FFFF` | RDRAM |
| `03F0_0000` | `03FF_FFFF` | RDRAM regs (+ broadcast window) |
| `0400_0000` | `0400_0FFF` | RSP **DMEM** (4 KiB) |
| `0400_1000` | `0400_1FFF` | RSP **IMEM** (4 KiB) |
| `0404_0000` | `040B_FFFF` | RSP registers |
| `040C_0000` | `040F_FFFF` | **Unmapped — CPU freeze** |
| `0410_0000` | `041F_FFFF` | RDP command registers |
| `0420_0000` | `042F_FFFF` | RDP span registers |
| `0430_0000` | `043F_FFFF` | **MI** |
| `0440_0000` | `044F_FFFF` | **VI** |
| `0450_0000` | `045F_FFFF` | **AI** |
| `0460_0000` | `046F_FFFF` | **PI** |
| `0470_0000` | `047F_FFFF` | **RI** |
| `0480_0000` | `048F_FFFF` | **SI** |
| `0490_0000` | `04FF_FFFF` | **Unmapped — CPU freeze** |
| `0500_0000` | `05FF_FFFF` | N64DD regs (PI Domain 1) |
| `0600_0000` | `07FF_FFFF` | N64DD IPL ROM |
| `0800_0000` | `0FFF_FFFF` | Cart SRAM/Flash (Domain 2) |
| `1000_0000` | `1FBF_FFFF` | Cart ROM (Domain 1) |
| `1FC0_0000` | `1FC0_07BF` | PIF ROM (boot) |
| `1FC0_07C0` | `1FC0_07FF` | PIF RAM (64 B) |
| `8000_0000` | `FFFF_FFFF` | Unmapped / external SysAD — freeze on retail |

Mirrors: some RCP windows ignore high address bits (see brew “Mirror mask” column). Implement masks explicitly.

---

## 4. SysAD access-size quirks (must)

From n64brew Memory map (paraphrase):

### RDRAM (`0x0000_0000–0x03EF_FFFF`)

- Access sizes 8/16/32/64 work; RI stalls CPU until complete.
- **Only region where cached accesses are legal.**

### RCP registers (`0x0400_0000–0x04FF_FFFF`)

- Access **size ignored**; device sees aligned 32-bit words with CPU-prepared shifting for SB/SH.
- **64-bit reads freeze** (second beat never arrives).
- **Cached access freezes.**
- Sub-word stores can “leak” upper register bits onto the bus — real software sometimes depends on this accidentally; tests will notice.

### PI / SI external

- Writes often **async**: CPU continues; busy bits in PI/SI status; overlapping rules differ (PI read-during-write returns latched pattern; SI delays reads).
- PI cart bus is **16-bit**; RCP still issues paired halfwords — known **halfword read address quirk** (16-bit read at `…2` can return the wrong halfword). Capture in tests; do not “fix” to be nicer than HW.

Provenance: [bus/n64brew-memory-map.md](./provenance/bus/n64brew-memory-map.md), [bus/sysad-access-quirks.md](./provenance/bus/sysad-access-quirks.md).

---

## 5. RI — RDRAM interface

Responsibilities:

- Bring-up / mode / refresh / bank status registers under `0x03F0_0000` and `0x0470_0000`.
- Expansion Pak: detect second 4 MiB; without terminator (Jumper Pak) HW fails — emu should model present/absent expansion bit for games that check.

Early emulator: treat RDRAM as flat RAM with RI register stubs returning safe values; refine refresh timing later.

---

## 6. PI — cartridge / parallel DMA

| Capability | Notes |
|------------|-------|
| Domains 1 & 2 | Latency / pulse / page size / release config per domain |
| DMA | RDRAM ↔ cart ROM or save devices |
| Status | DMA busy, IO busy, error bits |
| Cart ROM window | `0x1000_0000+` — endian swapped vs file formats |

**Endian:** `.z64` (big), `.n64` (byte-swapped), `.v64` (half-swapped) — detect and canonicalize on load ([06](./06-cart-cic-pif-saves.md)).

PI DMA length/alignment quirks are a classic incompatibility minefield — gate with homebrew DMA tests (krom / libdragon examples).

---

## 7. SI — PIF / joybus DMA

- 64-byte DMA to/from PIF RAM at `0x1FC0_07C0`.
- Joybus handshakes run on **SI DMA read**, not on arbitrary MMIO peek ([06](./06-cart-cic-pif-saves.md)).
- Write path async with IO busy semantics.

Without SI+PIF HLE, controllers and EEPROM saves do not work; boot suites using libdragon open IPL3 still need SI behavior for terminate-boot commands.

---

## 8. Other DMA masters (pointers)

| Engine | Doc |
|--------|-----|
| RSP DMA | [03](./03-rsp.md) — RDRAM ↔ IMEM/DMEM |
| RDP DMA | [04](./04-rdp.md) — command load from RDRAM/DMEM |
| AI DMA | [05](./05-audio.md) — RDRAM → DAC, double-buffered |
| VI | Fetches framebuffer; not a general DMA API |

All contend for RDRAM through RI — timing phase models contention; early phases may run engines in coarse slices.

---

## 9. MI — interrupt aggregation

MI_INTR / MI_INTR_MASK bits (SP, SI, AI, VI, PI, DP, …) feed CPU Cause IP bits. Clearing device causes + MI masks is a frequent young-emu bug (spurious IRQ storms or never-firing IRQs).

Provenance: [io/n64brew-mi.md](./provenance/io/n64brew-mi.md).

---

## 10. Implementation order

1. Physical decode table + freeze/unmapped behavior  
2. RDRAM R/W + KSEG0/1  
3. MI/VI/AI/PI/SI/RI register files (even if stubbed)  
4. PI cart ROM DMA  
5. SI ↔ PIF RAM HLE  
6. Async busy bits + PI halfword quirk  
7. Timing / contention  

---

## 11. Acceptance hooks

| Gate | Oracle |
|------|--------|
| Map | Unit tests: each region classify; freeze cases |
| Access sizes | n64-systemtest memory access set |
| PI | Home-brew DMA alignment ROMs; libdragon cart IO |
| SI | Controller present + EEPROM smoke |

---

## 12. TBD

- Exact RCP register access latencies (brew cites ~2–6 PClocks class).
- Full RI init sequence from IPL vs HLE soft-boot.
- 64DD Domain 1 devices (deferred).
