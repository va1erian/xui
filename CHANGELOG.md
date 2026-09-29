# Changelog

## Unreleased

### Fixes

- The canvas (`winit`) backend now synthesizes `Event::MouseDoubleClick` from a
  second press of the same button, within the platform's double-click time and
  distance of the first, on the same widget, producing the same
  `Down, Up, DoubleClick, Up` sequence as the Win32 backend
  (`WM_*BUTTONDBLCLK`); a third quick press starts a new sequence (#219).
- A modal `Dialog`/`TaskDialog` now dims the content behind it with a translucent
  scrim (`Theme::scrim`: black at 40% alpha in light mode, 55% in dark) instead
  of painting the window an opaque grey. On compositing backends (canvas) the
  app stays visible through the backdrop; the Win32 backend's separate child
  windows do not composite the scrim over sibling windows. Either way the scrim
  still blocks input to the widgets behind it (#221).
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
- Dragging a `Split` divider now tracks the cursor 1:1 instead of jumping: the drag delta is computed in the split's own coordinates (the divider-local point plus the divider's current origin), which do not move as the divider is resized, rather than in the moving divider's local coordinates (#220).

### Changes

- **`Toolbar` is now compact by default.** Buttons are packed from the left and each is only as wide as its content: an icon-only button is square (as wide as the strip is tall), a labelled one is icon, gap, measured label and padding, and the space right of the last item stays empty. The divider that was drawn between every cell is gone; group items with the new `Toolbar::separator()`, a thin non-clickable line that takes room but no item index, so `on_click` indices are unchanged. Items that do not fit are clipped at the right edge (no overflow menu yet). Call `Toolbar::fill()` to get the previous equal split of the whole width. Painting and hit-testing share one layout function (#217).

### Features

- **`Edit` now has the standard Windows CUA keyboard and selection.** Shift+Left/Right/Home/End extend a selection, Ctrl+Left/Right move by word (adding Shift extends by word), Ctrl+A selects all, Ctrl+Backspace/Ctrl+Delete delete a word, and typing replaces the selection. Ctrl+Z/Ctrl+Y undo and redo with a typing run grouped into one step, and Ctrl+C/Ctrl+X/Ctrl+V plus Shift+Delete/Shift+Insert use a new portable clipboard. A mouse drag selects and a double-click selects a word; the selection is highlighted with theme tokens and the text scrolls so the caret stays visible. The pure edit model lives in `xui-core/src/widget/edit/model.rs` with thorough unit tests (#222).
- **Portable text clipboard.** `Backend` gained `clipboard_text`/`set_clipboard_text`, defaulting to an in-process store so the headless and offscreen backends (and tests) support copy and paste. `Win32Backend` uses the existing Win32 clipboard, and the canvas/winit backend uses the OS clipboard through `arboard` (a new, text-only dependency; its default `image-data` feature is off) (#222).

- `ListView` rows can carry a leading icon: `ListModel::icon(row)` returns an optional `IconRef` (a `Lucide` outline, `Icon` or `Glyph`) drawn before the first column's text in the row's text colour; rows without one are laid out as before (#209).
- `Menu` entries can carry a leading icon: `MenuScope::icon(icon)` gives the entry just appended any `Into<IconRef>`, drawn in an icon column of bar dropdowns and context menus in the entry's text colour (dimmed when disabled); popups with no icons are unchanged (#211).
- `ComboBox` items can carry a leading icon: `ComboBox::item_icon(index, icon)` takes any `Into<IconRef>`, and `set_item_icon(index, Some(icon.into()))` sets or `set_item_icon(index, None)` clears one at runtime; it is drawn before the item's text in the dropdown list and in the closed box for the selected item, in the text colour (#210).
- **One public Lucide icon API.** The vendored outlines generate a public
  `xui_core::icon::Lucide` enum (one variant per SVG), `IconRef` unifies it with
  the legacy `Icon`/`Glyph` sets, and `xui_core::icon::draw_icon` is the single
  drawing entry point. `Button::icon`, `Toolbar` items, `TreeRow`/`RowIcon` and
  `TopBar` now accept any `Into<IconRef>`, while the existing `Icon` and `Glyph`
  callers keep compiling. The icon set grew from 25 to 75 icons; adding one is
  dropping its SVG into `crates/xui-core/assets/lucide/` and re-running
  `generate.py`.
- `arrange`: `ListView` and `StatusBar` are now `Placeable`, so they can be mounted in `row()`/`column()` layouts and reflow with the window (`ListView::auto`, `StatusBar::auto`); a mounted list re-lays its scrollbar against its new bounds (#181).

### Breaking

- `Theme` gained a `scrim: Rgba` field for the modal backdrop, so a full struct
  literal of the palette must set it (#221).
- `RowIcon` gained an `Icon(IconRef)` variant, so an exhaustive match on it must
  handle it; `Button::set_icon` now takes `Option<impl Into<IconRef>>`
  (`Button::clear_icon` removes the icon; a bare `None` needs a type
  annotation).
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
