<#
.SYNOPSIS
    Generates code coverage for the workspace with cargo-llvm-cov.

.DESCRIPTION
    Runs the whole test suite under LLVM source-based instrumentation and writes
    both an LCOV file and an HTML report under target\coverage\. This is what
    the CI `coverage` job runs; it needs no desktop because the GUI tests run in
    the check job, not here.

    Requires:
      rustup component add llvm-tools-preview
      cargo install cargo-llvm-cov

.EXAMPLE
    scripts\coverage.ps1
    scripts\coverage.ps1 -CovArgs '-p','xui-core'   # one crate only
#>
[CmdletBinding()]
param(
    # Extra arguments passed straight to cargo-llvm-cov.
    [string[]]$CovArgs = @()
)

# Native commands write progress to stderr; PowerShell 5.1 would turn that into
# a terminating error under 'Stop', so leave the preference relaxed and check
# $LASTEXITCODE instead.
$ErrorActionPreference = 'Continue'

if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
    Write-Host "cargo-llvm-cov is not installed. Run: cargo install cargo-llvm-cov" -ForegroundColor Red
    exit 1
}

$out = 'target/coverage'
cargo llvm-cov --workspace --all-features `
    --lcov --output-path "$out/lcov.info" `
    --html --output-dir "$out/html" @CovArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "LCOV: $out/lcov.info"
Write-Host "HTML: $out/html/index.html"
