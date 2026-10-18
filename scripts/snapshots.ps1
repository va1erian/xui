<#
.SYNOPSIS
  Renders every example headlessly, in light and dark, into target/snapshots/.

.DESCRIPTION
  No window opens and no sandbox is needed: the examples run on the offscreen
  software backend (XUI_SNAPSHOT makes each one save <example>-light.png and
  <example>-dark.png and exit).

  Each example also runs with XUI_LAYOUT_REPORT, and its layout report must
  have no warning: an example whose layout clips, overlaps or truncates fails
  the run.

.PARAMETER Out
  The directory to write to; defaults to target/snapshots.
#>
param([string]$Out = "target/snapshots")

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
# Only existing PNGs and logs are removed; a real failure (a locked file)
# still stops.
Get-ChildItem -Path $Out -Filter *.png -File | Remove-Item -Force
Get-ChildItem -Path $Out -Filter *.log -File | Remove-Item -Force

cargo build -q -p xui --features canvas,rhai --examples
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "target" }
$bin = Join-Path $targetDir "debug/examples"

$examples = @("widgets", "listview", "gridview", "top_bar", "layout", "containers", "absolute", "form", "script", "cookbook", "menu_bar_flicker")
foreach ($file in Get-ChildItem "crates/xui/examples/controls/*.rs") {
    if ($file.BaseName -ne "support") { $examples += "control_$($file.BaseName)" }
}

$env:XUI_SNAPSHOT = $Out
$env:XUI_LAYOUT_REPORT = "1"
$failed = @()
foreach ($name in $examples) {
    $exe = Join-Path $bin "$name.exe"
    $log = Join-Path $Out "$name.log"
    $err = Join-Path $Out "$name.err.log"
    # A demo that hangs must fail the run, not stall it.
    $process = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardOutput $log -RedirectStandardError $err
    # Reading the handle makes the exit code available after a NoNewWindow start.
    $null = $process.Handle
    if (-not $process.WaitForExit(120000)) {
        $process.Kill()
        $failed += $name
        Write-Host "FAIL $name (timed out)"
    } elseif ($process.ExitCode -ne 0) {
        $failed += $name
        Write-Host "FAIL $name"
        Get-Content $log -Tail 20 -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_ }
    } else {
        $marker = Select-String -Path $log -Pattern '^XUI_LAYOUT_WARNINGS:(\d+)' -ErrorAction SilentlyContinue | Select-Object -Last 1
        $warnings = if ($marker) { $marker.Matches[0].Groups[1].Value } else { $null }
        if ($warnings -eq "0") {
            Remove-Item $log, $err -Force -ErrorAction SilentlyContinue
            Write-Host "ok   $name"
        } else {
            $failed += $name
            Write-Host "FAIL $name (layout warnings: $(if ($null -ne $warnings) { $warnings } else { 'none reported' }))"
            Select-String -Path $log -Pattern '^\s*! ' -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_.Line }
        }
    }
}

Write-Host "$($examples.Count) examples, $($failed.Count) failed; images in $Out"
if ($failed.Count -gt 0) { exit 1 }
