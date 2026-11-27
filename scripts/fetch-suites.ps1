<#
.SYNOPSIS
    Fetch external N64 conformance test suites into gitignored tests\fixtures\fetched\ on Windows.

.DESCRIPTION
    Mirrors scripts/fetch-suites.sh using native Windows PowerShell (no WSL/Git Bash required).
    Policy: THIRD_PARTY.md & tests/fixtures/README.md
    - Dillonb/n64-tests (no license), rasky/n64-systemcrash (no license), and 240p Test Suite (GPL-2.0+)
      are strictly fetch-and-run only and MUST NEVER be committed to this MIT repository.
    - Default CI MUST NEVER require these fixtures or run `cargo test -- --ignored`.

.EXAMPLE
    .\scripts\fetch-suites.ps1 -Suite dillonb
    .\scripts\fetch-suites.ps1 -Suite all -DryRun
#>
[CmdletBinding()]
param(
    [ValidateSet("dillonb", "n64memtest", "systemtest", "nemu64", "systemcrash", "peterlemon", "240p", "all")]
    [string]$Suite = "all",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$FetchRoot = Join-Path $RepoRoot "tests\fixtures\fetched"

function Invoke-Step {
    param([string]$Description, [scriptblock]$Action)
    if ($DryRun) {
        Write-Host "[dry-run] $Description"
    } else {
        Write-Host "    $Description"
        & $Action
    }
}

function Ensure-Dir {
    param([string]$Path)
    if (-not $DryRun) {
        New-Item -ItemType Directory -Force -Path $Path | Out-Null
    }
}

function Fetch-Dillonb {
    $dest = Join-Path $FetchRoot "dillonb"
    $destSimple = Join-Path $FetchRoot "dillonb-simpleboot"
    Write-Host "==> Dillonb/n64-tests (no license — fetch-only, gitignored; 26 CPU ROMs + simpleboot)"
    Ensure-Dir $dest
    Ensure-Dir $destSimple

    $zipStd = Join-Path $dest "dillon-n64-tests.zip"
    $zipSimple = Join-Path $destSimple "dillon-n64-tests-simpleboot.zip"

    Invoke-Step "curl dillon-n64-tests.zip -> $zipStd" {
        curl.exe -fsSL -o $zipStd "https://github.com/Dillonb/n64-tests/releases/download/latest/dillon-n64-tests.zip"
        if ($LASTEXITCODE -ne 0) { throw "Failed to download dillon-n64-tests.zip" }
        Expand-Archive -Force -Path $zipStd -DestinationPath $dest
    }
    Invoke-Step "curl dillon-n64-tests-simpleboot.zip -> $zipSimple" {
        curl.exe -fsSL -o $zipSimple "https://github.com/Dillonb/n64-tests/releases/download/latest/dillon-n64-tests-simpleboot.zip"
        if ($LASTEXITCODE -ne 0) { throw "Failed to download dillon-n64-tests-simpleboot.zip" }
        Expand-Archive -Force -Path $zipSimple -DestinationPath $destSimple
    }
}

function Fetch-N64Memtest {
    $dest = Join-Path $FetchRoot "n64memtest"
    Write-Host "==> rasky/n64memtest (MIT/libdragon — RDRAM March & pattern diagnostics) -> $dest"
    Ensure-Dir $dest
    $rom = Join-Path $dest "n64memtest.z64"
    Invoke-Step "curl n64memtest.z64 (v0.9) -> $rom" {
        curl.exe -fsSL -o $rom "https://github.com/rasky/n64memtest/releases/download/v0.9/n64memtest.z64"
        if ($LASTEXITCODE -ne 0) { throw "Failed to download n64memtest.z64" }
    }
}

function Fetch-Systemtest {
    $dest = Join-Path $FetchRoot "systemtest"
    $repoDir = Join-Path $dest "repo"
    Write-Host "==> lemmy-64/n64-systemtest (MIT — Mooneye-equivalent frozen regression gate, built via Rust + nust64) -> $dest"
    Ensure-Dir $dest
    if (Test-Path (Join-Path $repoDir ".git")) {
        Invoke-Step "git fetch $repoDir" { git -C $repoDir fetch --depth=1 origin }
    } else {
        Invoke-Step "git clone https://github.com/lemmy-64/n64-systemtest.git $repoDir" {
            git clone --depth=1 "https://github.com/lemmy-64/n64-systemtest.git" $repoDir
        }
    }
    Write-Host "    Build ROM locally: cd $repoDir; cargo install nust64; cargo run --release"
}

function Fetch-Nemu64 {
    $dest = Join-Path $FetchRoot "nemu64"
    $repoDir = Join-Path $dest "repo"
    Write-Host "==> thelemmy/nemu64-test (MIT — active successor to n64-systemtest, built via Rust + nust64) -> $dest"
    Ensure-Dir $dest
    if (Test-Path (Join-Path $repoDir ".git")) {
        Invoke-Step "git fetch $repoDir" { git -C $repoDir fetch --depth=1 origin }
    } else {
        Invoke-Step "git clone https://github.com/thelemmy/nemu64-test.git $repoDir" {
            git clone --depth=1 "https://github.com/thelemmy/nemu64-test.git" $repoDir
        }
    }
    Write-Host "    Build ROM locally: cd $repoDir; cargo install nust64 --version 0.4.1 --locked; cargo run --release"
}

function Fetch-Systemcrash {
    $dest = Join-Path $FetchRoot "systemcrash"
    $repoDir = Join-Path $dest "repo"
    Write-Host "==> rasky/n64-systemcrash (no explicit license — fetch-only hardware freeze/crash oracle) -> $dest"
    Ensure-Dir $dest
    if (Test-Path (Join-Path $repoDir ".git")) {
        Invoke-Step "git fetch $repoDir" { git -C $repoDir fetch --depth=1 origin }
    } else {
        Invoke-Step "git clone https://github.com/rasky/n64-systemcrash.git $repoDir" {
            git clone --depth=1 "https://github.com/rasky/n64-systemcrash.git" $repoDir
        }
    }
}

function Fetch-PeterLemon {
    $dest = Join-Path $FetchRoot "peterlemon"
    Write-Host "==> PeterLemon/N64 curated manifest (Unlicense, excluding bundled bootcode) -> $dest"
    Ensure-Dir $dest
    Write-Host "    Manifest: $(Join-Path $RepoRoot 'tests\fixtures\peterlemon-manifest.json')"
    Write-Host "    (Curated entries download by raw path once hashes are pinned at M1/M3/M5.)"
}

function Fetch-240p {
    $dest = Join-Path $FetchRoot "240p"
    Write-Host "==> 240p Test Suite N64 (GPL-2.0+ — fetch-only, gitignored) -> $dest"
    Ensure-Dir $dest
    Write-Host "    Download official N64 release from https://github.com/ArtemioUrbina/240pTestSuite into $dest"
}

switch ($Suite) {
    "dillonb"     { Fetch-Dillonb }
    "n64memtest"  { Fetch-N64Memtest }
    "systemtest"  { Fetch-Systemtest }
    "nemu64"      { Fetch-Nemu64 }
    "systemcrash" { Fetch-Systemcrash }
    "peterlemon"  { Fetch-PeterLemon }
    "240p"        { Fetch-240p }
    "all" {
        Fetch-Dillonb
        Fetch-N64Memtest
        Fetch-Systemtest
        Fetch-Nemu64
        Fetch-Systemcrash
        Fetch-PeterLemon
        Fetch-240p
    }
}
