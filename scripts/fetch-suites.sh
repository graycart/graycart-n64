#!/usr/bin/env bash
# Fetch test ROM suites into gitignored tests/fixtures/fetched/.
#
# Policy: THIRD_PARTY.md & tests/fixtures/README.md
# - Dillonb/n64-tests (no license), rasky/n64-systemcrash (no license), and
#   240p Test Suite (GPL-2.0+) are strictly fetch-and-run only and MUST NEVER
#   be committed to this MIT repository.
# - Default CI MUST NEVER require these fixtures or run `cargo test -- --ignored`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FETCH_ROOT="${REPO_ROOT}/tests/fixtures/fetched"

SUITE="all"
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/fetch-suites.sh [--suite dillonb|n64memtest|systemtest|nemu64|systemcrash|peterlemon|240p|all] [--dry-run]

Fetches external N64 test suites into gitignored tests/fixtures/fetched/.
Never writes to tracked repository paths.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --suite)
      SUITE="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

run_or_echo() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $*"
  else
    "$@"
  fi
}

fetch_dillonb() {
  local dest="${FETCH_ROOT}/dillonb"
  local dest_simple="${FETCH_ROOT}/dillonb-simpleboot"
  echo "==> Dillonb/n64-tests (no license — fetch-only, gitignored; 26 CPU ROMs + simpleboot)"
  run_or_echo mkdir -p "${dest}" "${dest_simple}"
  run_or_echo curl -fsSL -o "${dest}/dillon-n64-tests.zip" \
    "https://github.com/Dillonb/n64-tests/releases/download/latest/dillon-n64-tests.zip"
  run_or_echo unzip -qo "${dest}/dillon-n64-tests.zip" -d "${dest}"
  run_or_echo curl -fsSL -o "${dest_simple}/dillon-n64-tests-simpleboot.zip" \
    "https://github.com/Dillonb/n64-tests/releases/download/latest/dillon-n64-tests-simpleboot.zip"
  run_or_echo unzip -qo "${dest_simple}/dillon-n64-tests-simpleboot.zip" -d "${dest_simple}"
}

fetch_n64memtest() {
  local dest="${FETCH_ROOT}/n64memtest"
  echo "==> rasky/n64memtest (RDRAM March & pattern diagnostics, v0.9) -> ${dest}"
  run_or_echo mkdir -p "${dest}"
  run_or_echo curl -fsSL -o "${dest}/n64memtest.z64" \
    "https://github.com/rasky/n64memtest/releases/download/v0.9/n64memtest.z64"
}

fetch_systemtest() {
  local dest="${FETCH_ROOT}/systemtest"
  echo "==> lemmy-64/n64-systemtest (MIT — frozen regression gate, build via Rust + nust64) -> ${dest}"
  run_or_echo mkdir -p "${dest}"
  if [[ -d "${dest}/repo/.git" ]]; then
    run_or_echo git -C "${dest}/repo" fetch --depth=1 origin
  else
    run_or_echo git clone --depth=1 "https://github.com/lemmy-64/n64-systemtest.git" "${dest}/repo"
  fi
  echo "    Build ROM locally: cd ${dest}/repo && cargo install nust64 && cargo run --release"
}

fetch_nemu64() {
  local dest="${FETCH_ROOT}/nemu64"
  echo "==> thelemmy/nemu64-test (MIT — pinned dual-gate successor, build via Rust + nust64) -> ${dest}"
  run_or_echo mkdir -p "${dest}"
  if [[ -d "${dest}/repo/.git" ]]; then
    run_or_echo git -C "${dest}/repo" fetch --depth=1 origin
  else
    run_or_echo git clone --depth=1 "https://github.com/thelemmy/nemu64-test.git" "${dest}/repo"
  fi
  echo "    Build ROM locally: cd ${dest}/repo && cargo install nust64 --version 0.4.1 --locked && cargo run --release"
}

fetch_systemcrash() {
  local dest="${FETCH_ROOT}/systemcrash"
  echo "==> rasky/n64-systemcrash (no explicit license — fetch-only hardware freeze oracle) -> ${dest}"
  run_or_echo mkdir -p "${dest}"
  if [[ -d "${dest}/repo/.git" ]]; then
    run_or_echo git -C "${dest}/repo" fetch --depth=1 origin
  else
    run_or_echo git clone --depth=1 "https://github.com/rasky/n64-systemcrash.git" "${dest}/repo"
  fi
}

fetch_peterlemon() {
  local dest="${FETCH_ROOT}/peterlemon"
  echo "==> PeterLemon/N64 curated manifest (Unlicense, excluding bootcode) -> ${dest}"
  run_or_echo mkdir -p "${dest}"
  echo "    Manifest: ${REPO_ROOT}/tests/fixtures/peterlemon-manifest.json"
  echo "    (Individual curated entries are downloaded by raw path once hashes are pinned in M1/M3/M5.)"
}

fetch_240p() {
  local dest="${FETCH_ROOT}/240p"
  echo "==> 240p Test Suite N64 (GPL-2.0+ — fetch-only, gitignored) -> ${dest}"
  run_or_echo mkdir -p "${dest}"
  echo "    Download official release from https://github.com/ArtemioUrbina/240pTestSuite into ${dest}/"
}

case "${SUITE}" in
  dillonb) fetch_dillonb ;;
  n64memtest) fetch_n64memtest ;;
  systemtest) fetch_systemtest ;;
  nemu64) fetch_nemu64 ;;
  systemcrash) fetch_systemcrash ;;
  peterlemon) fetch_peterlemon ;;
  240p) fetch_240p ;;
  all)
    fetch_dillonb
    fetch_n64memtest
    fetch_systemtest
    fetch_nemu64
    fetch_systemcrash
    fetch_peterlemon
    fetch_240p
    ;;
  *)
    echo "error: unsupported --suite '${SUITE}'" >&2
    usage >&2
    exit 1
    ;;
esac
