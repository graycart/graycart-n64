<!--
Cited: N64brew PIF-NUS; N64brew Memory map (cart windows); community CIC research posts
  (attribute per provenance note); Copetti cart/Expansion Pak pedagogy
URLs: https://n64brew.dev/wiki/PIF-NUS · https://n64brew.dev/wiki/Memory_map
Note: cart/PIF/CIC/saves — no dumps. Retrieved 2026-09-13.
-->

# graycart-n64 — Cart, CIC, PIF, saves, controllers

**Owns:** ROM image formats, boot/CIC/PIF HLE policy, joybus, EEPROM/SRAM/Flash, Expansion Pak.  
**Does not own:** PI DMA engine internals beyond cart use ([02](./02-bus-memory-dma.md)).  
**Provenance:** [provenance/cart/](./provenance/cart/)

---

## 1. Cartridge ROM

### File endianness

| Extension (conventional) | Byte order |
|--------------------------|------------|
| `.z64` | Big-endian (native bus order) |
| `.n64` | Little-endian byte-swapped |
| `.v64` | Byteswapped 16-bit (V64/Doctor) |

Detect via header magic (`0x80371240` big-endian word at start when correctly ordered). Always canonicalize to big-endian in memory.

### Header (high level)

| Offset | Field |
|--------|-------|
| `0x00` | Magic / PI DOM1 timing seed bytes |
| `0x04` | Clock rate exported (often ignored) |
| `0x08` | PC entry (usually `80000400` class after IPL3) |
| `0x0C` | Release |
| `0x10` | CRC1/CRC2 |
| `0x20` | Image name (20 bytes) |
| `0x3C` | Publisher / cart ID / region |
| `0x40` | **IPL3** boot code (through ~`0x1000`) |

IPL3 is cart-resident boot; authentic path checksums via PIF/CIC. **Do not vendor IPL3 binaries from commercial dumps** as “fixtures” without clear licensing — prefer **libdragon open IPL3** for CI.

---

## 2. CIC / anti-piracy

Cartridge **CIC** seeds talk to **PIF** during boot (and 6105 challenge/response later). Community has documented algorithms for common CIC variants (6102/6103/6105/6106/…).

**Repo rules:**

- Implement **HLE** checksum/challenge where needed for boot.  
- **Never** commit CIC ROM dumps or PIF-SM5 dumps.  
- Cite research posts/papers in provenance with author + URL ([cart/cic-research.md](./provenance/cart/cic-research.md)).

Wrong CIC HLE → freeze at boot (PIF kills CPU on failed IPL2 checksum).

---

## 3. PIF-NUS

Physical Sharp SM5-based chip. Functions:

1. Hold **IPL1/IPL2** (PIF ROM window during boot only)  
2. CIC channel  
3. **Joybus** to 4 controllers + cart channel (EEPROM/RTC)  
4. Reset button / NMI coordination  
5. Command byte in last of 64-byte **PIF RAM**

Communication: CPU uses **SI DMA** to R/W PIF RAM; command bits parsed by PIF firmware ([n64brew PIF-NUS](https://n64brew.dev/wiki/PIF-NUS)).

### Joybus (controllers)

- Channels 0–3: controller ports  
- Channel 4: cart serial (EEPROM, rare RTC)  
- Parse phase on command bit `0x01`; **execute on SI DMA read**  
- Standard controller: buttons + 8-bit stick; pak accessories (mem pak, rumble, transfer) add commands  

Emulator: HLE joybus responses; accessory stubs as needed.

### Boot HLE for CI

Recommended soft path:

1. Skip PIF ROM execution.  
2. Seed RDRAM / COP0 enough for target ROM.  
3. Copy cart `0x1000` bytes to `0xA0001000` style (classic IPL3 effect) when required by suite docs.  
4. Jump to suite entry (e.g. Dillonb `0x80001000`).  
5. Still implement runtime joybus + terminate-boot command bits for libdragon.

Optional **LLE PIF** using published research — advanced; still no binary dumps in git.

Provenance: [cart/n64brew-pif.md](./provenance/cart/n64brew-pif.md).

---

## 4. Save types

| Type | Window / path | Size class |
|------|---------------|------------|
| **EEPROM 4k/16k** | Joybus channel 4 | 512 B / 2 KiB |
| **SRAM** | PI Domain 2 `0x0800_0000+` | typically 32 KiB |
| **FlashRAM** | Domain 2 + command protocol | 128 KiB class |
| Controller Pak | Joybus accessory | 32 KiB per pak |

Detect via: database (secondary), cart ID heuristics, or user override. Wrong type → silent save loss — **fail loudly** if ambiguous.

Persist `.sra` / `.eep` / `.fla` style images via host; core only sees raw bytes ([10](./10-family-api-and-reuse.md)).

---

## 5. Expansion Pak

- Extra 4 MiB RDRAM.  
- Some titles **hard-require** it (DK64, Majora’s Mask, …).  
- Model as launch option `expansion_pak: bool`; default true for modern hosts, false for accuracy tests that expect base.

---

## 6. Controllers & accessories

Minimum viable:

- Standard pad mapping to Graycart button enum  
- Pak: none / rumble (GPIO-ish) / mempak stub  

Transfer pak / VRU: later.

---

## 7. Bring-up order

1. ROM endian load + header parse  
2. Soft-boot jump for homebrew suites  
3. PI ROM DMA  
4. SI + PIF RAM HLE joybus  
5. EEPROM  
6. SRAM / Flash protocols  
7. CIC/IPL fidelity improvements (still HLE)  

---

## 8. Acceptance hooks

| Gate | Oracle |
|------|--------|
| Load | Header unit tests for z64/n64/v64 |
| Boot | Dillonb / systemtest with documented soft-boot |
| Pad | Controller present status + button read |
| Save | Home-brew EEPROM/SRAM roundtrip |

---

## 9. Legal / dump policy (hard)

| Allowed in repo | Forbidden |
|-----------------|-----------|
| Open IPL3 (libdragon) under its license | Retail IPL3, PIF ROM, CIC dumps |
| Home-brew test ROMs with LICENSE | Commercial `.z64` |
| HLE source implementing published algorithms | “Firmware.bin” blobs of unknown origin |

---

## 10. TBD

- FlashRAM command state machine complete table.  
- Region PAL/NTSC PIF ROM differences under HLE.  
- Precise 6105 challenge vectors test vectors (homebrew).
