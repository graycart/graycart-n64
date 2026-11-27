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
| **Dillonb/n64-tests** | https://github.com/Dillonb/n64-tests | **None (no LICENSE)** | **NEVER commit, mirror, or redistribute.** Fetch-and-run only. | `tests/fixtures/fetched/dillonb/` |
| **240p Test Suite (N64)** | https://github.com/ArtemioUrbina/240pTestSuite | GPL-2.0+ | **NEVER commit into MIT crate.** Fetch-and-run only (VI gate). | `tests/fixtures/fetched/240p/` |
| **Commercial ROMs / PIF / CIC / IPL** | — | Proprietary | **FORBIDDEN in git.** Local smoke only under `carts/`. | `carts/` (gitignored) |

## Fetching Suites Locally or in Opt-In Conformance Jobs

```bash
# Preview target paths and URLs without downloading
bash scripts/fetch-suites.sh --dry-run

# Fetch a specific suite into gitignored tests/fixtures/fetched/
bash scripts/fetch-suites.sh --suite dillonb

# Run ignored ROM matrix explicitly (never in default CI)
RUST_MIN_STACK=16777216 cargo test --test roms -- --ignored --nocapture
```

## Grading Policy

- Gate on strict **`Failed: 0` per category** (CPU / COP0 / TLB / COP1, then RSP), never a whole-ROM lump count.
- Frame-hash goldens (`CpuRdpLle` and `EdgeWalker`, pre-VI-filter and post-VI-filter separately) are **never** re-blessed from our own output without an external hardware/oracle reference.
