# Provenance index — graycart-n64

Archives and citation notes for the graycart-n64 research pack. Prefer **primary** sources; mark secondary clearly. Do not invent hardware behavior.

**Credits are mandatory:** every in-tree Rust or Markdown source that depends on external docs/code must carry a **file-top attribution** ([`../ATTRIBUTION.md`](../ATTRIBUTION.md), [`../AGENTS.md`](../AGENTS.md)). This `provenance/` tree archives excerpts; it does **not** replace those headers.

## Rules

- Prefer: NEC VR4300 / MIPS R4300i docs, legally citable N64 programming reconstructions (n64brew, carefully attributed ultra64.ca summaries), HW-verified PIF research with authorship. Then open emu references (ares, cen64, mupen64plus, parallel-RDP, angrylion) as **secondary**.
- Copied excerpts live under the area folders below with a header block:

  ```text
  title:
  URL:
  retrieved:
  license/terms:
  why cited:
  ```

- When an implementation or research page uses those materials, **also** put credit at the top of *that* file.
- Unknowns stay marked TBD in the owning doc; provenance files record *what was checked*, not guesses.
- **No** ROM/BIOS/CIC/PIF binary dumps. **No** large copyrighted manual paste (keep excerpts short).

## Area folders

| Folder | Owns excerpts for | Sibling doc |
|--------|-------------------|-------------|
| [cpu/](./cpu/) | VR4300 ISA, pipeline, TLB, cache, COP0/COP1 | [../01-cpu-r4300i.md](../01-cpu-r4300i.md) |
| [bus/](./bus/) | Physical/virtual map, SysAD, PI/SI/RI DMA | [../02-bus-memory-dma.md](../02-bus-memory-dma.md) |
| [rsp/](./rsp/) | SU/VU, IMEM/DMEM, tasks, microcodes | [../03-rsp.md](../03-rsp.md) |
| [rdp/](./rdp/) | Command list, TMEM, pipe, VI path | [../04-rdp.md](../04-rdp.md) |
| [audio/](./audio/) | AI DMA, DACRATE, RSP audio ucode | [../05-audio.md](../05-audio.md) |
| [cart/](./cart/) | Cart ROM, CIC, saves, controllers | [../06-cart-cic-pif-saves.md](../06-cart-cic-pif-saves.md) |
| [io/](./io/) | MI interrupts, VI, SI/PIF interface | [../00-architecture-overview.md](../00-architecture-overview.md) |
| [tests/](./tests/) | Torture suites, homebrew, RDP refs | [../07-test-strategy.md](../07-test-strategy.md) |

Folders may be sparse until owners expand excerpts. Keep stable paths even when vacant.

## Canonical external sources (shared)

| Source | URL | Role |
|--------|-----|------|
| NEC VR4300 UM (7th ed.) | http://n64dev.org/p/U10504EJ7V0UMJ1.pdf · https://static.wikitide.net/n64wiki/5/55/VR4300-Users-Manual.pdf | **Primary** CPU |
| N64brew wiki | https://n64brew.dev/ | Community HW wiki (**prefer** for RCP/peripheral maps; treat as curated reconstruction) |
| Copetti — N64 architecture | https://www.copetti.org/writings/consoles/nintendo-64/ | Pedagogy / block diagram (**secondary**) |
| N64 Programming Manual (community host) | https://ultra64.ca/files/documentation/nintendo/ | Reconstruction — **cite carefully**; minimal excerpt |
| SGI RSP Programmer’s Guide (community host) | https://ultra64.ca/files/documentation/silicon-graphics/ | Reconstruction — **cite carefully** |
| angrylion / angrylion-plus | https://github.com/ata4/angrylion-rdp-plus | RDP LLE reference (**secondary-or-primary** carefully labeled per note) |
| paraLLEl-RDP | https://github.com/Themaister/parallel-rdp | Fast LLE RDP (**secondary**; aims bit-exact vs angrylion) |
| ares | https://github.com/ares-emulator/ares | Secondary full-system |
| cen64 | https://github.com/n64dev/cen64 | Secondary cycle-oriented |
| mupen64plus | https://github.com/mupen64plus | Secondary plugin ecosystem |
| lemmy-64/n64-systemtest | https://github.com/lemmy-64/n64-systemtest | Primary **test** ROM suite (homebrew) |
| Dillonb/n64-tests | https://github.com/Dillonb/n64-tests | Young-emulator CPU/RSP tests |
| PeterLemon/N64 | https://github.com/PeterLemon/N64 | Bare-metal demos / CPUTest |
| libdragon | https://github.com/DragonMinded/libdragon | Homebrew SDK + open IPL3 |

## Overview-pass citations (2026-09-13)

Used while writing [00-architecture-overview.md](../00-architecture-overview.md); full excerpt dumps deferred to area notes.

| Claim cluster | Primary / preferred cite |
|---------------|--------------------------|
| CPU 93.75 MHz VR4300; RCP 62.5 MHz | Copetti N64 (secondary pedagogy) + n64brew VR4300/RCP |
| Physical map RDRAM / RSP / MI…SI | N64brew Memory map |
| RSP 4 KiB IMEM/DMEM; SU+VU | N64brew RSP · RSP Programmer’s Guide (cite, don’t paste) |
| AI stereo DMA; IRQ on start | N64brew Audio Interface |
| PIF joybus / IPL lockout | N64brew PIF-NUS |
| Placeholder repo | https://github.com/graycart/graycart-n64 |

## License caution

Vendor PDFs and Nintendo/SGI manuals have their own terms — store minimal necessary excerpts, keep attribution, and do not republish entire manuals. Official dumps of PIF/CIC: **never** in this repo; document HLE and obtain-your-own paths only.
