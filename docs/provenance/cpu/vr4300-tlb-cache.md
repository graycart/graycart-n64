# Provenance: VR4300 TLB + cache

```
title:      NEC VR4300 UM — MMU/TLB + cache chapters (summary notes)
URL:        http://n64dev.org/p/U10504EJ7V0UMJ1.pdf
retrieved:  2026-09-13
license/terms: © NEC — paraphrase only
why cited:  Segment map, TLB entry fields, CACHE op classes, coherency expectations
community:  https://n64brew.dev/wiki/Memory_map (virtual segments)
```

## Notes

- KSEG0 cached direct map; KSEG1 uncached — software chooses intentionally.
- TLB-mapped kuseg used less in retail but tested by n64-systemtest.
- `CACHE` instruction required for correct I-cache management after DMA into code.

## Secondary cross-check

Copetti N64 “Memory management” section — pedagogy only: https://www.copetti.org/writings/consoles/nintendo-64/
