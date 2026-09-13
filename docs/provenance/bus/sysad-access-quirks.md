# Provenance: SysAD access-size quirks

```
title:      N64brew Memory map — Physical Memory Map accesses
URL:        https://n64brew.dev/wiki/Memory_map
retrieved:  2026-09-13
license/terms: Wiki contributor content — paraphrase
why cited:  Cached MMIO freeze; RCP ignores access size; PI 16-bit bus halfword bug; async PI/SI writes
```

## Critical behaviors to reproduce

1. Cached access outside RDRAM → hang.
2. 64-bit read of RCP regs → hang.
3. SB/SH to RCP may write shifted full-word values.
4. PI external 16-bit pairing can return wrong halfword for certain addresses.
5. PI/SI writes may complete asynchronously with busy bits.
