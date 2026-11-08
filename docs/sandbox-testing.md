# Running UI tests without taking over your desktop

The integration tests create real top-level windows, send them input
messages and confine the cursor (the pointer lock). On your own desktop those
windows appear and take focus while you type, and your typing can make tests
flaky.
`scripts/sandbox/run.ps1` runs them inside **Windows Sandbox**, a throwaway
Hyper-V VM that ships with Windows. It has its own desktop, input queue and
foreground window, so synthetic events stay in the sandbox — with one caveat:
the desktop is presented over RDP and shares the host *pointer* with it (see
[Pointer and input](#pointer-and-input)).

## One-time setup (Windows 10/11 Pro, Enterprise or Education)

```powershell
# as administrator, then reboot
Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM
```

You install nothing else. The sandbox doesn't need Rust or Visual Studio.

## Usage

```powershell
scripts\sandbox\run.ps1                                        # every test
scripts\sandbox\run.ps1 -CargoArgs '-p','xui-d2d','--test','pointer_lock'
scripts\sandbox\run.ps1 -CargoArgs '-p','xui-d2d','--test','window' -TestArgs 'modal'  # libtest filter
```

How it works:

1. Builds on the host with `cargo test --no-run`, into `target\sandbox` with a
   static CRT (the sandbox image has no VC++ runtime).
2. Copies the test executables into `target\sandbox\sandbox-stage\bin`. This
   folder is mapped into the sandbox as `C:\stage`.
3. Starts a network-less sandbox with a vGPU (for D2D, OpenGL and DWM). Its logon
   command runs every executable and writes `out\<name>.log` and
   `out\summary.txt`. The host script then closes the sandbox itself, so you
   don't get the "Windows Sandbox has closed" dialog.
4. Prints the logs. It exits with code 1 if any executable failed.

A cold start takes about 15–30 s on top of the build. The sandbox window opens
once. You can leave it behind your other windows: keyboard and click events stay
in the guest. If you minimize it, Windows may stop rendering the VM's desktop,
which breaks capture tests. Use `-Keep` to leave the sandbox open afterwards so
you can look around.

Limitations: only one sandbox can run at a time, and it needs hardware
virtualization. Windows Home doesn't include it (see the alternatives below).

## Pointer and input

Windows Sandbox presents its desktop over an RDP session and shares the host
pointer with it. Messages a test sends are delivered only to the guest — a
click never lands on a host window — but anything that moves or confines the
guest cursor is mirrored onto your physical pointer.

The window tests post or send messages to their own windows and never move the
cursor. The one suite that touches it is `xui-d2d`'s `pointer_lock`, which
confines the cursor to a test window (`ClipCursor`) and checks that every way
the lock ends frees it again. While it runs, your physical pointer may be held
inside the sandbox window for a moment. If that matters, use a VM with its own
pointer (see *Alternatives*).

## Screenshots

The sandbox has its own desktop with nothing else on it, so a plain
full-screen capture is enough, and focus and occlusion don't matter:

```powershell
# launch the gallery, wait 3 s, save target\sandbox\sandbox-stage\out\widgets.png
scripts\sandbox\run.ps1 -Build -CargoArgs '-p','xui','--example','widgets' -Screenshot
scripts\sandbox\run.ps1 -Exe .\myapp.exe -Screenshot -ScreenshotDelayMs 5000
```

`-Screenshot` starts each executable, waits `-ScreenshotDelayMs`, saves the
sandbox desktop as `out\<name>.png` (GDI+ `CopyFromScreen`, so it works for any
app), then stops the executable. Captures an example or test writes itself
(`XUI_SNAPSHOT`, `Ui::capture`) work as usual when you point them at
`C:\stage\out`. For portable widgets, the headless snapshots
([Headless screenshots](headless-screenshots.md)) need no sandbox at all.

## Client apps

The script works with any Cargo project. Call it from your app's repository:

```powershell
# your app's tests
path\to\xui\scripts\sandbox\run.ps1 -ManifestPath .\Cargo.toml

# build and launch the app itself as a smoke test (give it an auto-close switch
# like the examples' XUI_DEMO_AUTOCLOSE_MS, or it
# runs until the timeout)
path\to\xui\scripts\sandbox\run.ps1 -Build -CargoArgs '--bin','myapp' -Env @{ MYAPP_AUTOCLOSE_MS = '4000' }

# executables you already built
path\to\xui\scripts\sandbox\run.ps1 -Exe .\dist\myapp_tests.exe
```

You can also copy `run.ps1` into your repository. It has no dependency on xui.

## CI

The `windows-latest` GitHub runner is already a disposable VM with an
interactive desktop, so the existing `ci.yml` jobs are sandboxed by design.
Push a branch to run the whole suite without touching your machine. Windows
Sandbox itself can't run on hosted runners (they don't allow nested
virtualization), so `run.ps1` is for local use.

## Alternatives

- **Hyper-V / VirtualBox VM with a Windows dev image**: works on Windows Home
  (VirtualBox) and allows several parallel VMs, but setup is slower.
- **A second RDP session to your own machine** (a local user with an
  autologon session): tests get their own desktop, but you need a Windows
  Server or multi-session SKU, or a second account plus RDP wrapper hacks.
  Windows Sandbox is simpler.
