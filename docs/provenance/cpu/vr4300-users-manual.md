# Provenance: NEC VR4300 User’s Manual

```
title:      VR4300, VR4305, VR4310 64-Bit Microprocessor User's Manual (U10504EJ7V0UMJ1), 7th edition
URL:        http://n64dev.org/p/U10504EJ7V0UMJ1.pdf
alt:        https://static.wikitide.net/n64wiki/5/55/VR4300-Users-Manual.pdf
retrieved:  2026-09-13
license/terms: © NEC — do not redistribute entire PDF in-repo; cite chapter/section; minimal paraphrase only
why cited:  Primary silicon documentation for MIPS III VR4300 — pipeline, COP0/TLB/cache, exceptions, FPU, encodings
```

## How we use it

- Treat as **primary** for CPU behavior claims in [01-cpu-r4300i.md](../../01-cpu-r4300i.md).
- Prefer section cites (“UM ch.6 Exceptions”) over pasting tables.
- Cross-check N64-specific pin/errata against [n64brew VR4300](https://n64brew.dev/wiki/VR4300) (community).

## Excerpt policy

No large opcode maps or full chapters in git. If a future note needs a short table (e.g. exception vector bases), keep it under ~20 lines and repeat this header.
