# Attribution -- graycart-n64

Mandatory for in-repo Rust and Markdown that depend on external code or documentation.

Also stated in [`AGENTS.md`](./AGENTS.md). Provenance archives under [`provenance/`](./provenance/README.md) remain required for research excerpts — they do **not** replace per-file credits on sources that use those materials.

## Rule

If a file references someone else's **code** or **internet documentation** (NEC VR4300 manuals, n64brew, ultra64.ca reconstructions, angrylion notes, another emulator, blogs, SDK notes, etc.), put **credit at the top of that file**:

| Field | Required |
|-------|----------|
| What | Document / project / section used |
| Where | URL and/or canonical name |
| How | Brief note: *cited* / *inspired by* / *ported from* / *cross-checked against* |

Applies to:

- Rust modules (`//!` module docs or a short header comment)
- Markdown in this repo when the page quotes, ports, or structurally follows an external source

Does **not** replace:

- Minimal excerpts + license/terms in provenance folders
- Fixture LICENSE tables under `tests/fixtures/` (when the crate exists)

Never commit full manuals, commercial ROMs, PIF/CIC/IPL dumps, or Nintendo/SGI firmware images.

## Example -- Rust (`.rs`)

```rust
//! AI DMA length + DACRATE sample pacing.
//!
//! Cited: N64brew — Audio Interface
//!   https://n64brew.dev/wiki/Audio_Interface
//! Cross-check: libdragon AI delayed-carry workaround notes (secondary).
```

## Example -- Markdown (`.md`)

HTML comment (keeps the visible title clean):

```markdown
<!--
Cited: N64brew — Memory map
URL: https://n64brew.dev/wiki/Memory_map
Note: physical region table outline; not a full reprint.
-->
# Bus map
```

Or a visible header block under the title:

```markdown
# VR4300 exception vectors

title: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1)
URL: http://n64dev.org/p/U10504EJ7V0UMJ1.pdf
retrieved: 2026-09-13
license/terms: © NEC — cite section; minimal excerpt only; do not redistribute entire PDF in-repo
why cited: exception vector locations / COP0 Cause fields
```

## Primary vs secondary (quick legend)

| Tier | Examples | Use |
|------|----------|-----|
| **Primary** | NEC VR4300 UM; silicon-facing register maps reconstructed on n64brew with HW verification notes; dumped-PIF research posts with clear authorship | Prefer for behavior claims |
| **Community primary / reconstruction** | N64 Programming Manual / RSP Programmer’s Guide summaries hosted on ultra64.ca — cite carefully; do not paste large copyrighted spans | Prefer for RCP programming model |
| **Secondary** | Copetti pedagogy; ares; cen64; mupen64plus; parallel-RDP; angrylion-plus | Structure / cross-check only |
| **Forbidden in git** | PIF ROM binaries, CIC dumps, commercial carts | Obtain-your-own; HLE stubs for CI |

## Checklist before coding a behavior change

1. Name the primary reference and the acceptance test.
2. Add or update the **file-top credit** in every touched source that relied on that material.
3. If you paste or closely paraphrase an excerpt into research docs, also land a provenance file with the standard header block.

## Related

- [`AGENTS.md`](./AGENTS.md) — full agent norms
- [`provenance/README.md`](./provenance/README.md) — archive index
