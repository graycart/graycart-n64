# Provenance: RI / RDRAM

```
title:      N64brew RDRAM / RDRAM Interface pages + Copetti RDRAM notes
URL:        https://n64brew.dev/wiki/RDRAM
            https://n64brew.dev/wiki/RDRAM_Interface
            https://www.copetti.org/writings/consoles/nintendo-64/
retrieved:  2026-09-13
license/terms: Wiki + Copetti — secondary/community
why cited:  UMA RDRAM, Expansion Pak, 9th bit metadata for RDP
```

## Notes

- Base 4 MiB (+ Expansion Pak 4 MiB).
- 9th bit used by RDP coverage/z paths — CPU usually sees 8-bit bytes.
- Early emu: flat RAM + Expansion Pak option; refine RI init later.
