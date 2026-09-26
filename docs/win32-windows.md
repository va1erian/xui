# Windows-only window features

These live in the Win32 layer and have no portable equivalent. They are
best-effort: each is decided by what a documented platform call returns, never
by an OS version check, and each falls back cleanly.

## Backdrop material and caption

`WindowSpec::backdrop(Backdrop::Mica | MicaAlt | Acrylic)` asks DWM for the
system backdrop (`DWMWA_SYSTEMBACKDROP_TYPE`). `WindowSpec::title_bar(TitleBar)`:

- `TitleBar::Standard` — the ordinary system caption.
- `TitleBar::Colored` — the standard caption painted from theme tokens
  (`DWMWA_CAPTION_COLOR`).
- `TitleBar::Extended` — see below.

When DWM rejects the material (Windows 10, builds before 22621), in
high-contrast mode, or when the user disabled transparency effects, the window
falls back to the solid `Theme::background`; `Ui::backdrop_active()` reports
which path was taken.

## Extended title bar

`TitleBar::Extended` removes the standard caption (`WM_NCCALCSIZE`) while
keeping the native resize borders, and routes `WM_NCHITTEST` through
`DwmDefWindowProc` first, so the min/max/close buttons — and Windows 11 snap
layouts — keep working. The app draws its own caption in the reserved strip:

- a restored window's client starts at its top edge, so the caption strip is
  client area; the top resize band is hit-tested by the crate;
- a maximized window is inset by the frame it overhangs the monitor with;
- the frame is extended over the strip only (`DwmExtendFrameIntoClientArea` with
  a top margin), so DWM draws the caption buttons there and the material shows
  through while the rest of the client stays opaque; the strip is erased to
  black (DWM's "glass" colour);
- the strip's caption colour is set to none when the material is active (so
  "show accent on title bars" does not paint over it) and to `Theme::background`
  otherwise.

The free strip drags, widgets marked `ControlExt::set_caption_interactive`
accept clicks, and `Ui::caption_inset()` / `Ui::title_bar_height()` reserve the
button area so content never sits under it.

## Strip menu

With `WindowSpec::menu_in_strip(true)` the menu bar is not a native `HMENU`:
`Ui::set_menu_bar` draws the items in the strip with Direct2D/DirectWrite on a
premultiplied-alpha surface, so they stay opaque over the material (GDI text
over glass writes zero alpha and DWM drops it). The popups remain native
`TrackPopupMenuEx`, owner-drawn on a dark theme, so a chosen item still reaches
`App::update` through the normal queue.

`MenuStripPlacement::Inline` shares the caption row after the title (Windows
Terminal style); the default `Stacked` gives the menu its own row below the
caption. Hover, pressed and keyboard focus are translucent theme-token pills,
mnemonics underline while Alt is held, and the strip handles its own hit-testing,
mouse and keyboard input. When the material cannot be shown, the native menu bar
is used unchanged.

## Material status and top bars

A child `HWND` cannot blend with the parent's DWM material, so
`MaterialStatusBar::new(ui)` draws on a bottom material band instead. The frame
is extended at the bottom too, the band is painted with the same alpha-correct
Direct2D path as the strip, and the app reserves the height with
`Ui::material_status_bar_height()` as a bottom layout margin. `set_parts` /
`set_text` mirror `StatusBar`; when the material cannot be shown the band is
filled opaquely from `StatusBarTheme`. It requires an extended title bar (the
constructor returns an error otherwise, so the app can fall back to `StatusBar`).

`MaterialTopBar` and its `TopBarItem`/`TopBarId`/`TopBarEvent` vocabulary do the
same at the top of the client area (see `Ui::material_top_bar_height`,
`material_top_bar_slot`).

## Monitors, placement and fullscreen

- **Enumeration.** `MonitorInfo` (device and friendly name, rect, work area,
  primary, DPI), `monitors()`, `monitor_of` / `Window::monitor`, and
  `monitor_work_areas()`.
- **Placement.** New windows are centred on the owner's monitor (primary for a
  standalone window). `Ui::placement()` / `set_placement()` persist and restore
  geometry; `Placement`, `ShowState` and `centered_in_work_area` are public.
- **Display changes.** `Ui::on_display_change` maps `WM_DISPLAYCHANGE` to a
  `Msg`.
- **Fullscreen.** `Window::enter_fullscreen(&MonitorInfo)` / `leave_fullscreen`
  (a topmost `WS_POPUP` covering the monitor's full rect, with the saved style
  and placement restored), plus `hide_cursor_when_idle`.

## Capture

Three paths, documented on the methods:

- `Window::capture` (and `Ui::capture`): `PrintWindow`. Cheap and works without
  DWM, but misses the caption buttons, frame and backdrop.
- `Window::capture_screen` (and `Ui::capture_screen`): a screen `BitBlt`. It
  includes the DWM output but needs the window on screen and **unobscured**.
- `Window::capture_composited`, `Ui::capture_composited` and the free
  `capture::capture_hwnd(hwnd)` (behind the off-by-default `wgc` feature): the
  exact `Windows.Graphics.Capture` composited surface — frame, caption buttons,
  rounded corners and backdrop — of **any** top-level window, even an occluded
  one or another process's, without raising it or moving the pointer. It returns
  a typed error for a minimised window and does not restore it.

```text
cargo run -p xui-win32 --features wgc --example capture -- --title "My App" --out shot.png
```

`examples/capture.rs` is the tool for agents and test harnesses; its
`--hwnd`/`--title`/`--pid` flags are its own opt-in window lookup. The demo's
screenshot path uses the composited capture when `wgc` is on and falls back to
`PrintWindow` otherwise.

## UI Automation

Native common controls are exposed to Windows by the system. The crate adds a
portable accessibility model (`xui_core::accessibility`: `Role`, `Action`,
`RangeValue`, `Node`), and a custom widget bridges its own `Node` tree to
`IRawElementProviderSimple` via `CustomWidget::accessibility` /
`accessibility_action`. `tests/accessibility.rs` drives the native UIA tree.

## Clipboard, icons and shortcuts

- **Clipboard.** `clipboard::set_text` / `clipboard::text` over the Unicode
  clipboard (`CF_UNICODETEXT`, with LF→CRLF normalisation on write).
- **Icons.** `Icon::{from_resource, …}` and `Ui::set_icon`.
- **Shortcuts.** `Shortcut` (modifier constructors, `FromStr`, `Display` as
  `"Ctrl+N"`). `Ui::accelerator` registers one; `Ui::set_menu_bar` registers
  each menu item's shortcut automatically so the menu and the keyboard agree.
  `ShortcutParseError` reports a bad string.

## Raw message hook

For an OS integration that needs to observe a window message (a shell's
registered `TaskbarButtonCreated`, for example), `WindowHandler::raw_message`
and `Ui::on_raw_message` see every message before the layer decodes it. Return
`true` only for messages you fully handle. See
[Platform integration](platform-integration.md) for the pattern.

## Related

- [The Win32 layer](win32.md) — the platform and native widget layers.
- [Platform integration](platform-integration.md) — where shell-service code
  belongs.
- [Migration from `win32ui`](migration-win32ui-to-xui.md).
