<#
.SYNOPSIS
  Renders every example headlessly, in light and dark, into target/snapshots/.

.DESCRIPTION
  No window opens and no sandbox is needed: the examples run on the offscreen
  software backend (XUI_SNAPSHOT makes each one save <example>-light.png and
  <example>-dark.png and exit).

.PARAMETER Out
  The directory to write to; defaults to target/snapshots.
#>
param([string]$Out = "target/snapshots")

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
# Only existing PNGs are removed; a real failure (a locked file) still stops.
Get-ChildItem -Path $Out -Filter *.png -File | Remove-Item -Force

cargo build -q -p xui --features canvas,rhai --examples
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "target" }
$bin = Join-Path $targetDir "debug/examples"

$examples = @("widgets", "listview", "gridview", "top_bar", "layout", "containers", "absolute", "form", "script")
foreach ($file in Get-ChildItem "crates/xui/examples/controls/*.rs") {
    if ($file.BaseName -ne "support") { $examples += "control_$($file.BaseName)" }
}

$env:XUI_SNAPSHOT = $Out
$failed = @()
foreach ($name in $examples) {
    $exe = Join-Path $bin "$name.exe"
    # A demo that hangs must fail the run, not stall it.
    $process = Start-Process -FilePath $exe -PassThru -NoNewWindow
    # Reading the handle makes the exit code available after a NoNewWindow start.
    $null = $process.Handle
    if (-not $process.WaitForExit(120000)) {
        $process.Kill()
        $failed += $name
        Write-Host "FAIL $name (timed out)"
    } elseif ($process.ExitCode -ne 0) {
        $failed += $name
        Write-Host "FAIL $name"
    } else {
        Write-Host "ok   $name"
    }
}

Write-Host "$($examples.Count) examples, $($failed.Count) failed; images in $Out"
if ($failed.Count -gt 0) { exit 1 }
