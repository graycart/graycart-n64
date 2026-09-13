# Provenance: VR4300 exceptions / COP0 (notes)

```
title:      NEC VR4300 UM — Exceptions + COP0 (summary notes)
URL:        http://n64dev.org/p/U10504EJ7V0UMJ1.pdf
retrieved:  2026-09-13
license/terms: © NEC — paraphrase only
why cited:  Exception entry order, vector selection, Status EXL/ERL, Cause fields for emulator bring-up
secondary:  https://n64brew.dev/wiki/VR4300
```

## Emulator-facing checklist (paraphrase)

1. On exception: save PC (+ BD adjustment) to EPC or ErrorEPC per type; copy Status→ prior; set EXL.
2. Vector depends on bootstrap vs normal vs TLB refill — **confirm exact bases in UM before coding**.
3. Interrupts sample IP bits against IM masks when IE=1 and EXL/ERL clear.
4. NMI from PIF is distinct from Reset — soft-reset paths matter for save safety.

## TBD

Exact retail N64 reset COP0 defaults vs UM generic defaults — verify with homebrew.
