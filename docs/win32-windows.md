# Windows-only window features

These live in the Win32 backend and the platform layer under it. They are
best-effort: each is decided by what a documented platform call returns, never
by an OS version check, and each falls back cleanly.

## Backdrop material and caption

The portable `PlatformSpec` carries `Backdrop::{Opaque, Acrylic, Mica}`,
`Decorations` and `caption_inset`. `Win32Backend` maps them onto the system
backdrop (`DWMWA_SYSTEMBACKDROP_TYPE`) and a custom caption:

- `Decorations::None` (or a non-zero `caption_inset`) removes the standard
  caption (`WM_NCCALCSIZE`) while keeping the native resize borders, and routes
  `WM_NCHITTEST` through `DwmDefWindowProc` first, so the min/max/close buttons
  — and Windows 11 snap layouts — keep working. The app draws its own caption in
  the reserved strip; `Backend::caption_inset` reports the button area so
  content never sits under it.
- The frame is extended over the strip only (`DwmExtendFrameIntoClientArea` with
  a top margin), so DWM draws the caption buttons there and the material shows
  through while the rest of the client stays opaque.

When DWM rejects the material (Windows 10, builds before 22621), in
high-contrast mode, or when the user disabled transparency effects, the window
falls back to the solid `Theme::background`.

## Monitors, placement and fullscreen

- **Enumeration.** `MonitorInfo` (device and friendly name, rect, work area,
  primary, DPI), `monitors()`, `monitor_of` / `Window::monitor`, and
  `monitor_work_areas()`.
- **Placement.** New windows are centred on the owner's monitor (primary for a
  standalone window). `Placement`, `ShowState` and `centered_in_work_area` are
  public.
- **Fullscreen.** `Window::enter_fullscreen(&MonitorInfo)` / `leave_fullscreen`
  (a topmost `WS_POPUP` covering the monitor's full rect, with the saved style
  and placement restored), plus `hide_cursor_when_idle`.

## Capture

Three paths, documented on the methods:

- `Window::capture`: `PrintWindow`. Cheap and works without DWM, but misses the
  caption buttons, frame and backdrop.
- `Window::capture_screen`: a screen `BitBlt`. It includes the DWM output but
  needs the window on screen and **unobscured**.
- `Window::capture_composited` and the free `capture::capture_hwnd(hwnd)` (behind
  the off-by-default `wgc` feature): the exact `Windows.Graphics.Capture`
  composited surface — frame, caption buttons, rounded corners and backdrop — of
  **any** top-level window, even an occluded one or another process's, without
  raising it or moving the pointer. It returns a typed error for a minimised
  window and does not restore it.

```text
cargo run -p xui-win32 --features wgc --example capture -- --title "My App" --out shot.png
```

`examples/capture.rs` is the tool for agents and test harnesses; its
`--hwnd`/`--title`/`--pid` flags are its own opt-in window lookup.

## Accessibility

`Win32Backend` does not yet expose the portable accessibility model
(`xui_core::accessibility`: `Role`, `Action`, `RangeValue`, `Node`) through
Windows UI Automation, so screen readers cannot read a `Win32Backend` window.
The Win32-native controls that used to provide UIA were removed with the native
widget layer. This is a known gap (issue #169).

## Clipboard and icons

- **Clipboard.** `clipboard::set_text` / `clipboard::text` over the Unicode
  clipboard (`CF_UNICODETEXT`, with LF→CRLF normalisation on write).
- **Icons.** `Icon::{from_resource, …}` and `Window::set_icon`.

## Raw message hook

For an OS integration that needs to observe a window message (a shell's
registered `TaskbarButtonCreated`, for example), `WindowHandler::raw_message`
sees every message before the layer decodes it. Return `true` only for messages
you fully handle. See [Platform integration](platform-integration.md) for the
pattern.

## Related

- [The Win32 layer](win32.md) — the platform layer and `Win32Backend`.
- [Platform integration](platform-integration.md) — where shell-service code
  belongs.
