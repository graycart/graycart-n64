<!--
Cited: lemmy-64/n64-systemtest; Dillonb/n64-tests; PeterLemon/N64; libdragon;
  graycart-gba 11-test-apparatus posture
URLs: see §1 tables
Note: suite inventory + harness plan; no implementation this research turn. Retrieved 2026-09-13.
-->

# graycart-n64 — Test apparatus (suite inventory + harness)

**Audience:** Dave + implementers.  
**Does not supersede:** [07-test-strategy.md](./07-test-strategy.md)  
**Phase numbers:** [PHASES.md](./PHASES.md) / [08](./08-implementation-plan.md) win.  
**Provenance:** [provenance/tests/](./provenance/tests/)

---

## Relation to doc 07

| Doc | Owns |
|-----|------|
| **07** | Layers, CI culture, anti-patterns |
| **11 (this)** | Named suites, licenses, phase→suite map, vendoring, harness API sketch |
| **12** | Merge freeze enforcement |

---

## 1. Canonical suites

### 1.1 Dillonb/n64-tests — young CPU gate

| | |
|--|--|
| **URL** | https://github.com/Dillonb/n64-tests |
| **License** | Check upstream README/LICENSE before vendor |
| **Covers** | Early CPU; soft-boot friendly |
| **Oracle** | `r30 == -1` pass; `r30 > 0` fail id |
| **Soft-boot** | PC `0x80001000`; copy `0x100000` bytes from cart `0x10001000` → RDRAM `0x00001000` if skipping IPL |
| **Provenance** | [tests/dillonb-n64-tests.md](./provenance/tests/dillonb-n64-tests.md) |

### 1.2 lemmy-64/n64-systemtest — broad board

| | |
|--|--|
| **URL** | https://github.com/lemmy-64/n64-systemtest |
| **License** | Check upstream |
| **Covers** | COP0 wide regs, LL/SC, exceptions, TLB, mem sizes, RSP, quirks; optional timing/cycle/cop0hazard |
| **Boot** | libdragon open IPL3 |
| **Provenance** | [tests/n64-systemtest.md](./provenance/tests/n64-systemtest.md) |

### 1.3 PeterLemon/N64 — demos / CPUTest

| | |
|--|--|
| **URL** | https://github.com/PeterLemon/N64 |
| **Covers** | Bare-metal CPU/RDP visuals |
| **Oracle** | Frame hashes / visual smoke |
| **Provenance** | [tests/peterlemon-n64.md](./provenance/tests/peterlemon-n64.md) |

### 1.4 RDP golden corpus (internal)

| | |
|--|--|
| **Source** | Hand-built DLs; compare to angrylion-plus / paraLLEl outputs generated **offline** |
| **Vendor** | Goldens + scripts only |
| **Provenance** | [tests/rdp-goldens.md](./provenance/tests/rdp-goldens.md) |

### 1.5 libdragon examples

| | |
|--|--|
| **URL** | https://github.com/DragonMinded/libdragon |
| **Use** | Integration smoke; open IPL3 |

---

## 2. Phase → must suites

| Phase | Must | Stretch |
|-------|------|---------|
| P0 | units + fixture stubs | — |
| P1 | Dillonb CPU | systemtest CPU |
| P2 | systemtest mem subset / units | PI quirk ROMs |
| P3 | RSP DMA+SU ROM/unit | — |
| P4 | VU unit + task | — |
| P5 | ≥3 RDP demo hashes | 1 angrylion DL |
| P6 | AI beep | soft WAV |
| P7 | save roundtrip | Flash full |
| P8 | systemtest threshold T₀ | timing feature |
| P9+ | per PHASES | — |

---

## 3. Vendoring rules

| Artifact | Policy |
|----------|--------|
| MIT/Apache homebrew prebuilts | Vendor with LICENSE + pin SHA |
| GPL suites | Document fetch script; avoid mixing into MIT crate without legal review |
| Commercial | `carts/` gitignore only |
| PIF/CIC dumps | **Never** |

---

## 4. Harness API sketch

```rust
// tests/roms/harness.rs
fn run_rom(path: &Path, opts: BootOpts) -> Outcome { … }

struct BootOpts {
    soft_boot: bool,
    entry: Option<u32>,
    expansion_pak: bool,
    max_cycles: u64,
}
```

Matrices behind `#[ignore]` until fixtures present. Path/LICENSE stubs always compile.

---

## 5. No apparatus implementation in this research turn

Design only — coding waits on [08 §7](./08-implementation-plan.md#7-acceptance-criteria-dave-review-gate).
