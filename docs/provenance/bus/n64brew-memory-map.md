# Provenance: N64brew Memory map

```
title:      Memory map — N64brew Wiki
URL:        https://n64brew.dev/wiki/Memory_map
retrieved:  2026-09-13
license/terms: Wiki terms / contributors — community reconstruction; prefer HW-verified notes on page
why cited:  Canonical physical + virtual map tables for [02-bus-memory-dma.md](../../02-bus-memory-dma.md)
role:       PRIMARY-for-RCP-map (community curated)
```

## Excerpt — physical region list (compressed)

RDRAM `0x0000_0000–0x03EF_FFFF`; RDRAM regs `0x03F0_0000+`; DMEM/IMEM `0x0400_0000` / `0x0400_1000`; RSP regs `0x0404_0000`; RDP `0x0410_0000` / `0x0420_0000`; MI `0x0430_0000`; VI `0x0440_0000`; AI `0x0450_0000`; PI `0x0460_0000`; RI `0x0470_0000`; SI `0x0480_0000`; cart ROM `0x1000_0000+`; PIF ROM/RAM `0x1FC0_0000+`.

Unmapped RCP holes freeze the CPU — implement explicitly.
