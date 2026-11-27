<!--
Cited: Graycart N64 Development Plan §3.7; N64 Emulation Deep Dive §05 (Licensing minefield)
Note: Machine-readable third-party license boundary for graycart-n64 (MIT).
  All contributors and AI agents MUST check this table before reading, referencing,
  or citing any external N64 project or test suite.
-->

# Third-Party License Boundary (`THIRD_PARTY.md`)

`graycart-n64` is **MIT-licensed** and follows a **clean-room / bespoke implementation policy**:

- **Zero third-party emulator, RDP, or RSP forks or C++ FFI plugins.** Both our software LLE RDP (`CpuRdpLle`, M5) and our Vulkan compute LLE RDP (**`EdgeWalker`**, M6) are written from scratch in Rust + bespoke shaders.
- **Rust translation does not launder a license.** Translating GPL or MAME-licensed C/C++ into Rust creates a derivative work and is **strictly forbidden**.
- **Workspace isolation (mechanical rule):** Any external reference tree cloned for behavior study MUST live inside the gitignored `ref/` directory — NEVER inside `src/`, `tests/`, or any agent-copyable source path.
- **Mandatory file-top attribution:** Every Rust or Markdown file citing external specs or permissive reference sources MUST carry a top-of-file credit header per [`ATTRIBUTION.md`](./ATTRIBUTION.md).

## Emulators, Renderers & SDKs

| Project | License | Study | Link from MIT | Port / Translate | Repo Rule |
|---|---|---|---|---|---|
| **paraLLEl-RDP** | MIT | Yes | No (policy) | No (policy) | **Bespoke policy:** study & external output/fuzz comparison only. `EdgeWalker` is written from scratch — do not fork or link. |
| **paraLLEl-RSP** | MIT | Yes | No (policy) | No (policy) | Study / cross-check only. `InterpRsp` is written from scratch. |
| **ares** | ISC | Yes | No | Yes (with ISC notice) | Safest accuracy reference if an edge algorithm is ever adapted; MUST preserve per-file ISC provenance header (`Derived from ares, ISC, <file>`). |
| **cen64** | BSD-3-Clause | Yes | No | CPU/bus/RSP only (with BSD notice) | **Trap:** cen64's RDP is Angrylion/MAME-derived — NEVER port cen64's RDP. CPU/bus/RSP only with per-file BSD-3 header. |
| **libdragon + open IPL3** | Unlicense | Yes | Yes (test/boot) | Yes | Preferred open IPL3 & homebrew integration smoke target. |
| **angrylion-rdp-plus** | MAME (non-commercial core) | Yes | **NO** | **NEVER** | Compare offline-generated frame hashes/outputs only. Never port, copy, or link. |
| **Original Angrylion / MAME RDP** | MAME (non-commercial) | Yes | **NO** | **NEVER** | Compare outputs only. Never port, copy, or link. |
| **Mupen64Plus core + plugins** | GPLv2 / GPLv2+ (`cxd4` is CC0) | Yes | **NO** | **NEVER** | Study-only in gitignored `ref/`. Never port or link into MIT crate. |
| **Mupen64Plus-Next** | GPLv2-or-later | Yes | **NO** | **NEVER** | Study / compatibility oracle only. |
| **Project64** | GPLv2 | Yes | **NO** | **NEVER** | Study-only. Do not copy per-game hack databases. |
| **GLideN64** | GPLv2 | Yes | **NO** | **NEVER** | Study-only (also HLE, off-architecture). |
| **simple64** | GPLv3 (archived Feb 2025) | Yes | **NO** | **NEVER** | Study-only. |
| **Gopher64** | GPLv3 | Yes | **NO** | **NEVER** | Study-only. Note: Gopher64 also prohibits AI-authored upstream PRs — never attempt to upstream. |
| **Rosalie's Mupen GUI (RMG)** | GPLv3 | Yes | **NO** | **NEVER** | Study-only. |

## Test Suites & Fixtures

| Suite | License | Vendor in Git | CI / Local Rule |
|---|---|---|---|
| **lemmy-64/n64-systemtest** | MIT | Allowed (with LICENSE) | Frozen regression gate; build/pin from source or vendor built ROM + `LICENSE`. |
| **thelemmy/nemu64-test** | MIT | Allowed (with LICENSE) | Active successor (Jul 2026+); pin commit for dual-gate coverage. |
| **PeterLemon/N64 (krom)** | Unlicense | Curated manifest only | Too large (~2 GB) wholesale. Gate on curated hashed subset; **exclude bundled bootcode** (study-only). |
| **rasky/n64memtest** | MIT / libdragon | Fetch-only (`n64memtest.z64` v0.9) | Prebuilt RDRAM March/pattern diagnostic ROM fetched into gitignored `tests/fixtures/fetched/n64memtest/`. |
| **rasky/n64-systemcrash** | **No explicit license** | **FORBIDDEN** | Fetch-and-run only into gitignored `tests/fixtures/fetched/systemcrash/`; hardware freeze/crash oracle. |
| **Dillonb/n64-tests** | **No license** | **FORBIDDEN** | Absence of license != public domain. **Fetch-and-run only** into gitignored `tests/fixtures/fetched/dillonb/` (`dillon-n64-tests.zip` + `dillon-n64-tests-simpleboot.zip`); NEVER commit or mirror. |
| **240p Test Suite (N64)** | GPL-2.0+ | **FORBIDDEN** | **Fetch-and-run only** into gitignored `tests/fixtures/fetched/240p/`; NEVER commit into this MIT repo. |
| **Commercial ROMs / PIF / CIC / IPL dumps** | Proprietary | **FORBIDDEN** | Local-only under gitignored `carts/`; NEVER commit, NEVER require for CI. |

## Mechanical Checks

Before merging any PR:
1. Confirm `git status` and `git ls-files` contain zero files under `ref/`, `carts/`, `tests/fixtures/fetched/`, or forbidden binary extensions (`.z64`, `.n64`, `.v64`, `.sra`, `.eep`, `.fla`, `.sav`, `pif*.bin`, `cic*.bin`, `ipl*.bin`).
2. Confirm every new `.rs`/`.md` file referencing external documentation carries a top-of-file credit header per [`ATTRIBUTION.md`](./ATTRIBUTION.md).
