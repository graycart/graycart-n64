# Provenance: N64brew Audio Interface

```
title:      Audio Interface — N64brew Wiki
URL:        https://n64brew.dev/wiki/Audio_Interface
retrieved:  2026-09-13
license/terms: Wiki — community
why cited:  Register map, DMA double-buffer, IRQ-on-start, DACRATE math, delayed-carry bug
```

## Behaviors retained

- 16-bit stereo only; no on-AI mixing.
- IRQ when DMA **starts**.
- DACRATE: sample rate ≈ VI_clock / (DACRATE+1).
- Delayed-carry +0x2000 when transfer ends on 8 KiB boundary.
