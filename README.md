# graycart-n64

Future Graycart Nintendo 64 emulator (library-first core).

Part of the [Graycart family](https://github.com/graycart). This core will implement against the shared Graycart core API (see Project store `docs/graycart-family/` when published).

## Status

**Milestone M0 (Scaffold) underway** following ratification of [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) (2026-11-27).

- **Core:** Library-first Rust crate (`graycart-n64`, zero GUI deps), physical bus + bit-packed 9th-bit RDRAM side array, deterministic event-queue scheduler, `.z64`/`.v64`/`.n64` ROM loader + CIC detection, and headless ISViewer CLI runner.
- **Graphics roadmap:** Bespoke pure-Rust `CpuRdpLle` reference oracle (M5) paired with **`EdgeWalker`** (M6), our from-scratch Vulkan compute LLE RDP (zero third-party RDP forks/FFI), plus an opt-in M11 enhancement tier (LLE upscaling, SSAA, widescreen, VI/CRT filters, Oracle Lens, Live Frame Inspector) behind `EnhancementConfig`.

Start here: [`docs/DEVELOPMENT-PLAN.md`](./docs/DEVELOPMENT-PLAN.md) · [`STATUS.md`](./STATUS.md) · [`AGENTS.md`](./AGENTS.md) · [`THIRD_PARTY.md`](./THIRD_PARTY.md) · [`ATTRIBUTION.md`](./ATTRIBUTION.md)

## Policy

- No commercial ROMs, PIF/CIC/IPL dumps, or large copyrighted manuals in this repo.
- File-top attribution required for sources that cite external docs/code ([`ATTRIBUTION.md`](./ATTRIBUTION.md)).
- Mechanical third-party license boundary in [`THIRD_PARTY.md`](./THIRD_PARTY.md).

MIT — Copyright (c) 2026 Graycart.
