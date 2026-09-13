<!--
Cited: N64brew Reality Display Processor; Video Interface; angrylion-plus README;
  Themaister parallel-rdp README; Copetti N64 RDP section
URLs: https://n64brew.dev/wiki/Reality_Display_Processor · https://github.com/ata4/angrylion-rdp-plus
  · https://github.com/Themaister/parallel-rdp
Note: RDP overview; angrylion/parallel labeled secondary-or-reference carefully. Retrieved 2026-09-13.
-->

# graycart-n64 — RDP (Reality Display Processor)

**Owns:** command list model, pipeline stages, TMEM, hazards, VI output path, LLE reference strategy.  
**Does not own:** RSP microcodes that *generate* commands ([03](./03-rsp.md)), plugin product UX ([09](./09-graphics-plugin-accuracy.md)).  
**Provenance:** [provenance/rdp/](./provenance/rdp/)

---

## 1. Role

The RDP is a **fixed-function** raster pipeline inside the RCP. It consumes a stream of **64-bit (and multi-word) commands** — set tiles, load TMEM, triangles, rectangles, syncs — and writes pixels into an RDRAM framebuffer (plus z/coverage metadata using RDRAM’s 9th bit where applicable).

It is **not** a programmable shader CPU. Accuracy work is about matching pipeline state machines, memory hazards, and coverage/AA/blender quirks.

---

## 2. Interface

| Path | Who | How |
|------|-----|-----|
| Registers `0x0410_0000` | CPU | Start DMA, status, clocks |
| Registers `0x0420_0000` | CPU | Span / TMEM BIST / test modes |
| RSP COP0 | RSP | Same control while ucode runs |
| Command source | RDRAM or DMEM | DMA into RDP command FIFO |

Status bits feed **MI DP** interrupt when the pipe goes idle / synced per command semantics.

---

## 3. Pipeline blocks (conceptual)

Order varies by **cycle type** / mode bits; classic description:

1. **Rasterizer** — triangles / rects → fragments  
2. **Texture unit + TMEM (4 KiB)** — tile descriptors, format convert, wrap/clamp/mirror  
3. **Texture filter** — point / (tri)linear quirks (N64 “bilinear” is not PC GL bilinear)  
4. **Color combiner** — multi-source RGB/A math (the “N64 material system”)  
5. **Blender** — fog, AA coverage, z-buffer compare/update, framebuffer blend  
6. **Memory interface** — RDRAM FB/Z/TMEM loads  

RDP **operating modes** reconfigure which stages run and how. Bad mode combos + missing syncs → glitches or **hard pipe hangs** on HW.

---

## 4. Command classes

| Class | Examples | Notes |
|-------|----------|-------|
| Set state | Set Other Modes, Combine, Blend Color, Prim/Env/Fog colors, Fill color | State must stick across primitives |
| TMEM | Set Tile, Set Tile Size, Load Block/Tile, Load TLut | 4 KiB is tiny; games thrash TMEM |
| Sync | Sync Pipe / Load / Tile / Full | Hazard avoidance — emus that ignore syncs pass easy demos and fail retail |
| Primitives | Tri* , TexRect, FillRect | Edge walking + scissor |
| Misc | Set Scissor, Set Convert, Set Key, … | |

Command length encoding is in the high bits of the first word — parser must be strict.

Provenance: [rdp/n64brew-rdp-commands.md](./provenance/rdp/n64brew-rdp-commands.md).

---

## 5. VI output path

RDP writes a framebuffer in RDRAM. The **Video Interface**:

- Points at FB DRAM address / width / timing regs (`0x0440_0000`)  
- Generates NTSC/PAL timing, optional interlacing  
- Applies **filters** (divot, gamma, AA fetch patterns) that heavily affect the “blurry N64 look”  
- Raises **MI VI** interrupt per field/line configuration  

Emulator split options:

| Approach | Pros | Cons |
|----------|------|------|
| Present raw RDP FB | Simple | Misses VI filter look |
| LLE VI filter | Accurate CRTs / dumps | Complex; needed for some effects |
| Host upscale + optional VI HLE | Pretty | Not accuracy |

Angrylion traditionally includes VI behavior as part of the video plugin story; paraLLEl-RDP focuses on RDP bit-exactness with angrylion as oracle.

---

## 6. Reference implementations (label carefully)

| Project | Label | Use |
|---------|-------|-----|
| **angrylion** / **angrylion-plus** | CPU LLE RDP; treat as **de facto accuracy oracle** (community primary *behavior*, still third-party code) | Pixel compares; slow |
| **paraLLEl-RDP** | **Secondary** Vulkan LLE aiming bit-exact vs angrylion-plus | Performance LLE path |
| GlideN64 / older HLE GL | **Secondary** HLE | Enhancements; expect divergence |
| ares RDP | **Secondary** full-system cross-check | |

**Never** paste angrylion sources into graycart. Study behavior; write original code; cite.

Provenance: [rdp/angrylion-reference.md](./provenance/rdp/angrylion-reference.md), [rdp/parallel-rdp-secondary.md](./provenance/rdp/parallel-rdp-secondary.md).

---

## 7. Hard hazards (emulator graveyard)

- Missing **SyncPipe** before mode changes mid-frame  
- TMEM load overlapping in-flight texture use  
- Scissor / clip edge off-by-ones  
- Coverage bits / AA incorrect → shimmering silhouettes  
- Depth compare modes + decals  
- Framebuffer as texture (CPU/RSP readbacks, VI mid-frame) — needs coherency, kills naive HLE  
- Fillrate vs XBUS vs DRAM command path stalls (**timing**)  

---

## 8. Bring-up order

1. Command parser + noop/sync + Set* state  
2. FillRect / solid Tri into 16-bit FB  
3. TMEM load + textured rect  
4. Combiner + blender basics  
5. Z-buffer  
6. Coverage / AA  
7. VI present + interrupt  
8. Bit-exact harness vs angrylion dumps / paraLLEl tests (vendored **outputs**, not their engine)  

---

## 9. Acceptance hooks

| Gate | Oracle |
|------|--------|
| Parser | Unit tests from hand-crafted DL |
| Pixels | angrylion bit-exact on homebrew DLs; paraLLEl’s public test ideas as inspiration |
| Integration | libdragon / krom RDP demos hashed |
| Stretch | Retail FB effects (pause, heat haze) |

---

## 10. TBD

- Exact RDP clock counter meanings for profiling regs.  
- Full span-register test mode utility.  
- Legal strategy for any Nintendo RDP command PDF excerpts (prefer n64brew + homebrew headers).
