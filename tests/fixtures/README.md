<!--
Cited: Graycart N64 Development Plan §3.1 & §3.7; docs/11-test-apparatus.md; docs/12-test-gates.md
Note: Test-suite license and vendoring contract. Default `cargo test` MUST stay 100% ROM-free.
-->

# Test Fixtures (`tests/fixtures/`)

All conformance ROM matrices live behind `#[ignore]` in [`tests/roms/`](../roms/main.rs).
Default CI (`cargo test` without `-- --ignored`) MUST remain **100% ROM-free and dump-free**.

## Suite Inventory & License Rules

| Suite | Upstream URL | License | Git Policy | Local Path |
|---|---|---|---|---|
| **lemmy-64/n64-systemtest** | https://github.com/lemmy-64/n64-systemtest | MIT | Vendoring built ROM + `LICENSE` allowed (frozen regression gate) | `tests/fixtures/fetched/systemtest/` |
| **thelemmy/nemu64-test** | https://github.com/thelemmy/nemu64-test | MIT | Vendoring built ROM + `LICENSE` allowed (pinned dual-gate successor) | `tests/fixtures/fetched/nemu64/` |
| **PeterLemon/N64 (krom)** | https://github.com/PeterLemon/N64 | Unlicense | Curated manifest only (`peterlemon-manifest.json`); **exclude bundled bootcode** | `tests/fixtures/fetched/peterlemon/` |
| **rasky/n64memtest** | https://github.com/rasky/n64memtest | MIT / libdragon | Fetch-and-run (`v0.9/n64memtest.z64`) | `tests/fixtures/fetched/n64memtest/` |
| **rasky/n64-systemcrash** | https://github.com/rasky/n64-systemcrash | **None declared** | **NEVER commit.** Fetch-and-run only (hardware freeze oracle). | `tests/fixtures/fetched/systemcrash/` |
| **Dillonb/n64-tests** | https://github.com/Dillonb/n64-tests | **None (no LICENSE)** | **NEVER commit, mirror, or redistribute.** Fetch-and-run only (`dillon-n64-tests.zip` + `dillon-n64-tests-simpleboot.zip`, 26 ROMs each). | `tests/fixtures/fetched/dillonb/` & `dillonb-simpleboot/` |
| **240p Test Suite (N64)** | https://github.com/ArtemioUrbina/240pTestSuite | GPL-2.0+ | **NEVER commit into MIT crate.** Fetch-and-run only (VI gate). | `tests/fixtures/fetched/240p/` |
| **Commercial ROMs / PIF / CIC / IPL** | — | Proprietary | **FORBIDDEN in git.** Local smoke only under `carts/` or `../carts/n64/`. | `carts/` (gitignored) |

## Fetching Suites Locally or in Opt-In Conformance Jobs

```powershell
# Windows (PowerShell):
.\scripts\fetch-suites.ps1 -Suite dillonb
.\scripts\fetch-suites.ps1 -Suite n64memtest
.\scripts\fetch-suites.ps1 -Suite all -DryRun
$env:RUST_MIN_STACK = "16777216"; cargo test --test roms -- --ignored --nocapture
```

```bash
# macOS / Linux (Bash):
bash scripts/fetch-suites.sh --dry-run
bash scripts/fetch-suites.sh --suite dillonb
bash scripts/fetch-suites.sh --suite n64memtest
RUST_MIN_STACK=16777216 cargo test --test roms -- --ignored --nocapture
```

## Grading Policy

- Gate on strict **`Failed: 0` per category** (CPU / COP0 / TLB / COP1, then RSP), never a whole-ROM lump count.
- Frame-hash goldens (`CpuRdpLle` and `EdgeWalker`, pre-VI-filter and post-VI-filter separately) are **never** re-blessed from our own output without an external hardware/oracle reference.
