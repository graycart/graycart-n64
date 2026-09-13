<!--
Cited: angrylion-plus README; Themaister parallel-rdp; mupen64plus plugin ecosystem folklore (secondary);
  N64brew RDP/RSP
URLs: https://github.com/ata4/angrylion-rdp-plus · https://github.com/Themaister/parallel-rdp
Note: accuracy tradeoff doc — not an endorsement to vendor foreign engines. Retrieved 2026-09-13.
-->

# graycart-n64 — Graphics plugin / accuracy tradeoffs

**Owns:** LLE vs HLE product matrix, plugin seams, what we will and will not claim.  
**Siblings:** [03 RSP](./03-rsp.md) · [04 RDP](./04-rdp.md) · [08 plan](./08-implementation-plan.md)

---

## 1. Two different “graphics” problems

| Layer | Question |
|-------|----------|
| **RSP** | Do we **execute** microcode (LLE) or **reimplement** GBI/ucode behavior (HLE)? |
| **RDP** | Do we **simulate the RDP pipe** (LLE) or **draw with host GPU APIs** from high-level tris (HLE)? |

Mixing **HLE RSP + LLE RDP** generally **does not work** for angrylion-class renderers: LLE RDP expects a real command stream from real ucode.

---

## 2. Matrix

| RSP \ RDP | LLE RDP (angrylion-class) | HLE GL/Vulkan |
|-----------|---------------------------|---------------|
| **LLE RSP** | **Accuracy path** (slow CPU RDP; faster with GPU LLE) | Possible but odd; still needs correct DL feed |
| **HLE RSP** | **Broken / unsupported** for bit-exact LLE RDP | Classic mupen “fast” path |

---

## 3. LLE RDP options

| Engine class | Pros | Cons | Graycart posture |
|--------------|------|------|------------------|
| In-tree CPU LLE | Full control; CI-friendly | Slow | **Default accuracy goal** — original code |
| angrylion-plus | Oracle | License/integration; not our code | **Reference only** |
| paraLLEl-RDP | Fast; aims bit-exact vs angrylion | Vulkan dep; secondary | Optional later study — **do not copy**; evaluate license |

---

## 4. HLE microcode realities

HLE wins performance and enables upscale/filters, but fails when:

- Game uses **modified / rare ucodes**  
- Relies on **ucode bugs** or undocumented GBI  
- Does **FB effects** / CPU paint / RDP readback mid-frame  
- Needs exact coverage dither patterns  

Graycart: HLE allowed behind **`--hle-rsp` / `--hle-rdp`** (names TBD) with UI text that says **compatibility mode**, not “accurate.”

---

## 5. VI and presentation

Even perfect RDP can “look wrong” if VI filtering is skipped. Options:

1. Raw FB nearest (debug)  
2. Soft VI filter LLE  
3. Host shader approximating VI  

Document which mode produced a screenshot in bug reports.

---

## 6. Recommended product profiles

| Profile | RSP | RDP | VI | Audience |
|---------|-----|-----|----|----------|
| `ci-lle` | interp | CPU LLE | raw/soft | Gates |
| `play-lle` | dynarec | GPU LLE or fast CPU | soft VI | Accuracy players |
| `play-hle` | HLE | HLE | host | Casual |

Default for developers: `ci-lle`.

---

## 7. What we refuse to claim

- “Bit-perfect” without angrylion (or HW) corpus evidence.  
- HLE as equivalent to LLE.  
- That dynarec alone fixes RDP hazards.

---

## 8. TBD

- Exact feature-flag names.  
- Whether to ever embed a Vulkan LLE vs IPC to external tool.  
- Enhancement (widescreen, hi-res tex) policy — likely host-only, off in accuracy profile.
