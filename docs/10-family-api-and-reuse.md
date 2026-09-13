<!--
Cited: Project store docs/graycart-family/01-core-api.md; graycart-gba 10-core-api-and-gb-reuse.md posture
Note: family reuse notes for N64 — no silicon reuse from gba. Retrieved 2026-09-13.
-->

# graycart-n64 — Family API / reuse notes

**Owns:** how graycart-n64 plugs into Graycart hosts; what to reuse from siblings; what is greenfield.  
**Family docs:** store [`docs/graycart-family/`](../graycart-family/README.md) · [`01-core-api.md`](../graycart-family/01-core-api.md)  
**GBA analogue:** [`docs/graycart-gba/10-core-api-and-gb-reuse.md`](../graycart-gba/10-core-api-and-gb-reuse.md)

---

## 1. Greenfield vs reuse

| Reuse | Do **not** reuse |
|-------|------------------|
| CI culture, SemVer, attribution, fixtures layout | ARM7 / GBA PPU / GB APU code |
| Host chrome patterns (egui/winit/cpal) | SM83 or GBA bus waitstate tables |
| `GraycartCore` trait shape | Any sibling’s CPU interpreter |
| Research provenance *discipline* | SNES/NES trees (sibling-owned) |

N64 silicon is **new**. Process is **copied** from gba research pack.

---

## 2. Capability mapping (family checklist)

| Capability | N64 notes |
|------------|-----------|
| Load ROM bytes | Canonicalize z64/n64/v64 |
| Firmware slots | Optional PIF/IPL user blobs — **never required for CI** |
| Reset | Soft vs power-on (PIF terminate-boot state) |
| run_frames / run_cycles | Frame ≈ VI field; cycle = VR4300 or RCP tick (document) |
| Framebuffer | After VI present; format RGB555/RGBA888 converted at boundary |
| Audio drain | From AI resampled to host rate |
| Buttons | N64StandardPad + optional pak enum |
| Battery image | EEPROM/SRAM/Flash bytes |
| Savestate | Versioned blob including RDRAM+RCP+CPU |
| Model options | Expansion Pak, region, HLE flags |

---

## 3. Illustrative Rust surface

```rust
pub struct N64Core { /* … */ }

impl GraycartCore for N64Core {
    type Button = N64Button;
    // load_rom, load_firmware(FirmwareSlot::PifRom, …), run_frames, frame, drain_audio, …
}
```

C ABI (`gc_*`) follows family extract when `graycart-abi` exists — N64 should not invent a parallel ABI.

---

## 4. Host responsibilities

- File dialogs, path consent, shade→sRGB, audio device, input devices.  
- Mapping Expansion Pak toggle + region.  
- Never downloading copyrighted dumps.

---

## 5. Linux / multi-core future

Same library-first rule as gba: `graycart-n64` crate has **zero** GUI deps. graycart-linux can dlopen/link later.

---

## 6. Umbrella repo

[graycart/graycart](https://github.com/graycart/graycart) pins `graycart-n64` as a submodule ([family repos.md](../graycart-family/repos.md)). Pin bumps are separate PRs after child tags.

---

## 7. TBD

- Exact `FirmwareSlot` enum members.  
- Savestate format v1 layout.  
- Whether HLE flags are core options or host-only.
