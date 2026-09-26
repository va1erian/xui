# Platform integration: the portable / Windows seam

An application built on xui usually wants integrations that the operating
system owns, not the widget library: now-playing on the taskbar, a jump list,
file associations, a notification, a tray icon. On Windows those are Win32 /
WinRT / shell calls; on Linux and macOS they are D-Bus services, `.desktop`
files and `Info.plist`. This document fixes where that code lives so it never
leaks into xui's portable layer.

It complements [Backends](backends.md) (what each backend supports),
[The Win32 layer](win32.md) (the Windows escape hatches), [macos.md](macos.md)
(what the software backend supports) and
[migration-win32ui-to-xui.md](migration-win32ui-to-xui.md) (Cargo and API
renames). It is not a widget-library change: xui gains no `cfg`-dependent
public API from it.

## Where a feature is allowed to live

xui is split so a feature sits in exactly one of three places.

| | Where | What it is | Example |
|---|---|---|---|
| **(a)** | `xui-core` | already portable, no platform dependency, no `unsafe` | `Theme`, `Dip`, `Key`, the `Node`/`Role`/`Action` accessibility model, `PlatformSpec`/`Backend`, the portable widget layer and its `App`/`Ui` runtime |
| **(b)** | `xui-win32` (`#![cfg(windows)]`) | the Win32 backend, including `cfg(windows)` extension traits over `xui-core` types | `Backdrop`/`TitleBar`, `SystemTheme::system()`, UIA, WGC capture, native controls |
| **(c)** | the application | an OS integration the library will not host, compiled away on other targets | SMTC, taskbar thumbar/jump list/progress, `winshell` associations, notifications |

The rule of thumb:

- If it is geometry, colour, layout, input or a **model** every backend shares,
  it belongs in `xui-core`.
- If it is **how one platform draws or hosts a widget**, it belongs in that
  backend, behind `cfg(windows)` at the crate root. The umbrella crate only
  re-exports that backend on Windows
  (`crates/xui/src/lib.rs`: `#[cfg(all(feature = "win32", windows))]`).
- If it is an **OS service** (media keys, the shell, notifications), it belongs
  to the application. xui's job is to give the app a handle and a message
  hook, not to abstract every desktop.

## The `cfg(windows)` pattern for app integrations

An app keeps the portable code portable by defining one trait up front,
implementing it once per platform, and picking an implementation at startup.
Nothing in the trait's signature names a Win32 type, so it compiles on Linux
and macOS where the Windows impl is gone.

```rust
// src/integration/mod.rs — compiled everywhere
pub trait NowPlaying: Send + Sync {
    /// Push the current track to the OS. `None` means "nothing playing".
    fn set_track(&self, track: Option<&Track>);
    fn set_playing(&self, playing: bool);
    fn set_progress(&self, position: Duration, length: Duration);
}

/// A no-op for targets with no integration.
pub struct NoNowPlaying;

impl NowPlaying for NoNowPlaying {
    fn set_track(&self, _: Option<&Track>) {}
    fn set_playing(&self, _: bool) {}
    fn set_progress(&self, _: Duration, _: Duration) {}
}

#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

/// Each platform may decline (SMTC missing, no session bus, …); the no-op is
/// the fallback for every target, including ones with no integration at all.
pub fn now_playing(msg_tx: Proxy<Msg>) -> Box<dyn NowPlaying> {
    #[cfg(windows)]
    if let Some(integration) = windows::try_now_playing(msg_tx.clone()) {
        return integration;
    }
    #[cfg(target_os = "linux")]
    if let Some(integration) = linux::try_now_playing(msg_tx.clone()) {
        return integration;
    }
    #[cfg(target_os = "macos")]
    if let Some(integration) = macos::try_now_playing(msg_tx.clone()) {
        return integration;
    }
    let _ = msg_tx;
    Box::new(NoNowPlaying)
}
```

Each `mod` is a separate file that is only compiled on its target, so the
`windows` crate is not even a dependency on Linux (see `Cargo.toml` below).

```toml
[target.'cfg(windows)'.dependencies]
# `Media_Media` is the WinRT `Windows.Media` namespace (SMTC);
# `Win32_UI_Shell` has the taskbar, AUMID and shell APIs.
windows = { version = "0.62", features = ["Media_Media", "Win32_UI_Shell"] }

[target.'cfg(target_os = "linux")'.dependencies]
zbus = "4"

[target.'cfg(target_os = "macos")'.dependencies]
# MediaPlayer framework bindings (MPNowPlayingInfoCenter, MPRemoteCommandCenter).
objc2-media-player = "0.3"
```

`Proxy<Msg>` (a `xui_core` type) is `Send + Sync`, so a D-Bus or MPRIS worker
thread can push button presses and state changes back into `App::update`
without touching the UI thread's state directly.

## What emusic uses, mapped

### Now playing: SMTC → MPRIS/D-Bus, MPNowPlayingInfoCenter

**Windows.** `Windows.Media.SystemMediaTransportControls` (projected as the
`ISystemMediaTransportControls` family). Create it on the UI thread after the
window exists, set an explicit AppUserModelID first (see *Taskbar* below), fill
`DisplayUpdater.MusicProperties`, subscribe to `ButtonPressed` and forward
Play/Pause/Next/Previous to `Msg`. The media-key path goes through the shell,
so it must not run before the process has an AppUserModelID.

**Linux.** MPRIS2: own `org.mpris.MediaPlayer2.<App>` on the session bus and
implement `org.mpris.MediaPlayer2.Player`. `Metadata` is an `a{sv}` map
(`mpris:trackid`, `xesam:title`, `xesam:artist`, `mpris:length` in µs),
`PlaybackStatus` is `Playing`/`Paused`/`Stopped`, and `Seeked`/`PropertiesChanged`
carry updates. Incoming `Play`, `Pause`, `Next`, `Previous` calls are D-Bus
messages, so service them on a worker thread and hand the events to `Msg` over
`Proxy<Msg>`. MPRIS is the desktop's SMTC; there is no per-desktop variant to
write.

**macOS.** `MPNowPlayingInfoCenter.default()` for metadata and
`MPRemoteCommandCenter.shared()` for the command handlers, with
`MPMediaItemArtwork` for the thumbnail.

### Taskbar: thumbnail toolbar, jump list, progress

**Windows.** `ITaskbarList3` (and `IApplicationDocumentLists` /
`ICustomDestinationList` for jump lists). `ITaskbarList3` is COM and needs the
taskbar button to exist, which the shell announces with a registered
`TaskbarButtonCreated` message. Observe it through xui's raw-message seam
(below) and create the object from the window's `Hwnd`:

```rust
let hwnd = ui.hwnd();                 // xui_win32::Hwnd, not a `windows` type
ui.on_raw_message(move |msg| {
    // SAFETY/contract: `msg` is a live `MSG` only for this call.
    if is_taskbar_created(msg) && let Some(taskbar) = TaskbarList::new() {
        let _ = taskbar.set_progress(&hwnd, 42, 100);
    }
    false // let xui keep decoding it
});
```

Call `SetCurrentProcessExplicitAppUserModelID` before creating any window, or
the taskbar groups the app under `rust.exe` and the jump list and thumbar
attach to the wrong button.

**Linux.** There is no single API — this is app-level and per desktop:

- KDE/Plasma: `org.kde.JobViewServer` / `org.kde.JobView` for a progress job;
  quicklist-style actions live in the app's `.desktop` `Actions=` entries and
  in the Plasma launcher, not on a thumbar.
- GNOME: there is no EQ of the thumbar; use a `GNotification`/notification or a
  libappindicator/StatusNotifierItem tray item, and route the app's actions
  through `Gio.Action`s in the `.desktop`.
- A portable-ish subset is the **Unity Launcher API**
  (`com.canonical.Unity.LauncherEntry`, a D-Bus signal with `progress` and
  `count`); it is the closest match to `SetProgressValue`, but it is only
  honoured by launchers that implement it. Treat it as best-effort and keep the
  in-window `ProgressBar` as the real indicator.

**macOS.** `NSApplication.shared.dockTile`: a custom `contentView` (an
`NSProgressIndicator`) for progress, `badgeLabel` for a count, and
`NSApplicationDelegate.applicationDockMenu` for the jump-list equivalent.

### File associations: `winshell` → `.desktop`/xdg, `Info.plist`

**Windows.** `winshell` registers under `HKCU\Software\Classes`: a ProgID with
`shell\open\command`, an extension key with `OpenWithProgids`, and an
`Applications\<exe>` entry, then `SHChangeNotify` to refresh the shell. Per-user
registration means no elevation.

**Linux.** Ship a `.desktop` file with `MimeType=` and `Exec=`, install it to
`$XDG_DATA_HOME/applications` (or `/usr/share/applications`), declare the MIME
type in a `shared-mime-info` XML if it is custom, and set the default with
`xdg-mime default app.desktop <type>`. `update-mime-database` and
`update-desktop-database` refresh the caches. "Open with" is the file manager's
dialog; do not try to re-implement it.

**macOS.** Declare `CFBundleDocumentTypes` (and
`UTExportedTypeDeclarations`/`UTImportedTypeDeclarations`) in `Info.plist`;
`LSRegisterURL` / `lsregister` re-registers a bundle during development.

### Also app-level

- **Notifications/toasts**: `Windows.UI.Notifications` (needs an AUMID) →
  `org.freedesktop.Notifications` on Linux → `UNUserNotificationCenter` on
  macOS.
- **Tray / status item**: `Shell_NotifyIcon` → StatusNotifierItem/AppIndicator
  on Linux → `NSStatusItem` on macOS.
- **Single instance**: a named mutex or a message-only window (`FindWindow`)
  → a D-Bus name or a lock file on Linux → `NSApplication` activation on macOS.
- **Global (system-wide) hotkeys**: `RegisterHotKey` → XDG desktop portal
  `org.freedesktop.portal.GlobalShortcuts` on Linux (Wayland) → Carbon/`NSEvent`
  hot keys on macOS.

xui will **not** host these in `xui-core`. They are OS services with no shared
model, and any abstraction over them would either be a lowest-common-denominator
that helps nobody or a Win32 API in a portable costume.

## The escape hatches xui does provide

xui already gives an app the two things an OS integration needs, without
exposing a `windows` type:

- **A raw message hook.** `WindowHandler::raw_message`
  (`crates/xui-win32/src/window/mod.rs`) and `Ui::on_raw_message`
  (`crates/xui-win32/src/app/ui.rs`) see every window message before the widget
  layer decodes it. The pointer is valid only for the call, the same contract
  as winit's `with_msg_hook`; return `true` only for messages you fully handle.
  This is how a shell's registered `TaskbarButtonCreated` is observed.
- **The window handle.** `Ui::hwnd()` returns an opaque `xui_win32::Hwnd`
  (`Copy`, with `raw()`), which is all a COM integration such as
  `ITaskbarList3` needs. No `windows` type crosses the public API.
- **A portable native handle.** On the portable runtime,
  `xui_core::Ui::native_window()` (and `WindowHandle::native`) return an opaque
  `xui_core::NativeWindowHandle` whose `raw()` is the platform handle (`HWND` on
  Windows), so an integration can be written against the portable layer and only
  `#[cfg(windows)]` code interprets the value. `Backend::capture` likewise
  gives the portable runtime the screenshot path the Win32 layer already had.
- **A thread boundary.** `Proxy<Msg>` and `Window::post_wake` carry worker
  results (an MPRIS command, a D-Bus signal) back to `App::update` without
  re-entrancy.
- **Capability-aware widgets.** A custom `CustomWidget` re-themes through
  `Themed` and describes itself through the portable accessibility model, so
  the app's own painted widget uses the same theming and UIA paths as a bundled
  control.

To make the raw message hook portable, the app calls `on_raw_message` only
inside a `#[cfg(windows)]` function; the portable `App::update` sees the
decoded `Msg` the hook produced, never the `MSG`.

## Survey of genuinely Windows-only surfaces in `crates/xui-win32/src/`

| Area | Where | Category | Notes / portable mapping |
|---|---|---|---|
| Geometry, units, colour, layout arithmetic | `xui-core` | (a) | shared by every backend |
| Theme tokens, `Themed`, input vocabulary | `xui-core` | (a) | `Theme::system()` is *not* here; see below |
| Accessibility model (`Node`, `Role`, `Action`) | `xui-core::accessibility` | (a) | the tree is built backend-neutrally |
| `PlatformSpec`/`Backend` window contract (`Decorations`, `caption_inset`, `Backdrop{Opaque,Acrylic,Mica}`) | `xui-core::backend` | (a) | the portable vocabulary a backend maps to its chrome |
| System theme read (`SystemTheme::system()`, `is_theme_change`) | `theme/system.rs` | (b) | extension trait over `Theme`; Win32 reads the registry, DWM and `SPI_GETHIGHCONTRAST` |
| Backdrop material + themed caption (`Backdrop`, `TitleBar`) | `window/backdrop.rs`, `window/title_bar.rs`, `sys/dwm.rs` | (b) | `Backdrop::MicaAlt` and `TitleBar::{Colored,Extended}` have no portable equivalent; `PlatformSpec::backdrop` carries the portable subset (opaque/acrylic/mica) |
| Extended title bar, caption inset, strip menu | `app/title_menu/`, `window/nc.rs`, `app/core/` | (b) | Win32-only; `Ui::caption_inset`/`caption_buttons`/`strip_height` are the app-facing seams |
| Material status/top bars | `controls/statusbar.rs`, `app/top_bar/` | (b) | DWM material bands |
| UIA exposure | `accessibility/`, `sys/uia/` | (b) | native controls are exposed by Windows; custom widgets bridge the portable `Node` tree to `IRawElementProviderSimple` |
| Capture (`capture`/`capture_screen`/`capture_composited`) | `capture.rs`, `sys/capture*.rs` | (b) | `capture_screen` includes DWM output but needs an unoccluded window; `capture_composited` is behind the off-by-default `wgc` feature |
| Native menu bar / popups / accelerators | `controls/menu/`, `accel.rs`, `sys/menu*.rs` | (b) | a separate portable menu model exists in `xui-core::widget::Menu` for the canvas backend |
| Native common controls | `controls/` | (b) | expose UIA, IME and system behaviour for free; the canvas backend paints equivalents (`ImplKind::Painted`) |
| Clipboard | `clipboard.rs` | (b) | Win32 clipboard; no portable trait yet |
| Monitors, placement, fullscreen | `window/monitor.rs`, `window/placement.rs`, `window/fullscreen.rs` | (b) | `MonitorInfo`, `Placement`; the portable `PlatformSpec` has no fullscreen |
| `Hwnd`, raw message hook, `looper`, `Shortcut` | `hwnd.rs`, `window/mod.rs`, `looper.rs`, `accel.rs` | (b) | the escape hatches an app integration builds on |
| SMTC / taskbar thumbar / jump list / progress / shell associations / notifications / tray | *not present* | (c) | app-level; map to MPRIS/D-Bus, `.desktop`/xdg, `Info.plist` as above |

"Category (b)" means the item lives in `xui-win32`, which is `#![cfg(windows)]`
(`crates/xui-win32/src/lib.rs`); on other targets the crate is empty and the
item's portable counterpart is in `xui-core`.

## Cross-target build notes

- `xui-win32` starts with `#![cfg(windows)]`, so it compiles to an empty crate
  elsewhere and the umbrella crate falls back to `xui_core`. Linux and macOS CI
  run `cargo check --all-targets` to prove this.
- Platform crates are under `[target.'cfg(windows)'.dependencies]`, so the
  `windows`/`windows-core`/`glow` bindings are not built on Linux at all. An
  app should follow the same shape: put its Windows-only integration crates
  under a `cfg(windows)` target table.
- The `wgc` feature is off by default; it is the only capture path that works
  for an occluded window. Its `examples/capture.rs` is now gated on
  `all(feature = "wgc", windows)` so `--features wgc` no longer breaks a
  non-Windows `cargo check`.

## What this means for xui

- **Hosted in the library**: the window contract and its portable vocabulary,
  theming, the widget/control layer, UIA, capture, and the raw-message and
  `Hwnd` escape hatches. These are the pieces that are genuinely about *how a
  backend draws and routes*, not about *which desktop service is running*.
- **Left to the app**: every shell/service integration. The app defines a small
  portable trait, implements it per target behind `#[cfg]`, and drives it from
  the `Msg` queue. That keeps `xui-core` platform-free and keeps the Windows
  integration honest — it can use the documented OS APIs directly instead of
  through a wrapper that hides what it needs.
