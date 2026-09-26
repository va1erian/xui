<#
.SYNOPSIS
    Runs mutation testing over the portable core with cargo-mutants.

.DESCRIPTION
    Mutation testing changes one operator or literal at a time and checks that
    the suite notices (a "surviving" mutant is a place the tests do not really
    assert). It is deliberately scoped to `xui-core`: pure logic, fast headless
    tests, no desktop. Use the report to find weak tests, not as a per-PR gate.

    Requires: cargo install cargo-mutants

.EXAMPLE
    scripts\mutants.ps1
    scripts\mutants.ps1 -ExtraArgs '--in-place'   # leave mutations for review
#>
[CmdletBinding()]
param(
    # Extra arguments passed straight to cargo-mutants.
    [string[]]$ExtraArgs = @()
)

$ErrorActionPreference = 'Continue'

if (-not (Get-Command cargo-mutants -ErrorAction SilentlyContinue)) {
    Write-Host "cargo-mutants is not installed. Run: cargo install cargo-mutants" -ForegroundColor Red
    exit 1
}

# Only mutate (and only run) xui-core, so the GUI suites never open a window.
cargo mutants --package xui-core --test-package xui-core @ExtraArgs
exit $LASTEXITCODE
