# Changelog

## Unreleased

### Fixes

- `TopBar` lays its items out and hit-tests them in node-local coordinates, so a
  bar anywhere other than the window origin responds to clicks, hovers and
  slider drags (#157).
- The canvas `OffscreenBackend` no longer holds the window state while painters
  run, so a `ListView`/`TreeView` scrollbar (which reads the DPI during paint)
  composites instead of panicking; a regression test covers a parent-placed
  list's scrollbar thumb tracking its offset (#158).
- A test guards that `Decorations::None` asks `winit` for an undecorated window
  on Windows, so the app's own caption is the only chrome (#150).
- `StatusBar` no longer lets a part's text overflow past its divider into the next part: it is now clipped and end-ellipsized the same way `ListView`'s header cells are (#186).
- `ListView`'s header now reserves room for the sort arrow, so a long sorted-column title is ellipsized before the arrow instead of drawn underneath it (#187).

### Features

- `arrange`: `ListView` and `StatusBar` are now `Placeable`, so they can be mounted in `row()`/`column()` layouts and reflow with the window (`ListView::auto`, `StatusBar::auto`); a mounted list re-lays its scrollbar against its new bounds (#181).

### Breaking

- **The Win32-native widget layer is removed.** `xui-win32` no longer has
  `controls`, the `column!`/`row!`/`tabs!` layout tree, `xui_win32::run_app(
  WindowSpec, ...)`, the strip menu, material status/top bars, `TitleBar`,
  `Shortcut`, or the UI Automation bridge for those controls. `xui-win32` is now
  the `Win32Backend` implementation of `xui-core`'s `Backend` (Direct2D/DirectWrite
  painting, GDI fallback) plus its low-level platform layer. See
  [the migration note](docs/migration-win32ui-to-xui.md). The last commit with the
  old layer is tagged `pre-win32ui-controls-removal`.
- **The `xui` umbrella's default feature is now `canvas`**, and the `win32`
  feature is renamed `d2d`. The umbrella's bare names are always the portable
  `xui-core` widgets.
- `Win32Backend` has no Windows UI Automation support yet (#169).
- `xui-litehtml`'s `HtmlView` is now a portable custom-painted node: `HtmlView::new` takes a `&Ui` and bounds, paints through `Canvas`, and shapes text through the new `Ui::text_shaper` (#168).
- `xui-litehtml` is cross-platform: no `cfg(windows)` gate, no `xui-win32` dependency. Ctrl+C now raises `HtmlViewEvent::CopyRequested(text)` instead of writing the Win32 clipboard (#179).
