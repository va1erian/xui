# Changelog

## Unreleased

### Additions

- **Midnight theme and decoration tokens.** `Theme::midnight()` is a navy dark
  theme with vertical gradients, bevelled cards and controls, round swatches
  and an accent glow on checked indicators. `Theme` gains `background_end`,
  `surface_end`, `bevel`, `shade`, `corner_radius` and `glow` (off in
  `light()`/`dark()`, which paint as before), and `theme::look` paints them.
  A `Theme` built as a struct literal must now set these fields.
- **Gloss, captions, plain panels and primary buttons.** `Theme::gloss`
  lightens a control face toward its top (Midnight's buttons, check boxes,
  radios and swatches); `Label::title` and `Label::caption` set a page title
  and an upper-case section caption (a rounded theme's `GroupBox` titles use
  the caption style); `Panel::plain` is a container that draws nothing;
  `Button::primary` marks the default action in the accent with a halo; on a
  decorated theme `IconView` highlights the whole tile like a selected row and
  `ColorPicker` draws glossy swatches ringed apart when selected.
- **Widgets draw on their container.** `Canvas::composites_parents` tells a
  painter whether its ancestors are already painted under it (true on
  `xui-canvas`, through `Surface::with_canvas_over_parents`); bundled widgets
  then skip their background fill, so labels, check boxes, radio groups and
  swatch rows no longer sit in `background`-coloured boxes inside a panel or
  scroll view. Win32 is unchanged.
- **`ToggleButton` icons.** `ToggleButton::icon`, `set_icon` and `clear_icon`
  mirror `Button`'s: any `IconRef` is drawn before the label, or centred in an
  icon-only button, in the label's colour (on-accent while checked, dimmed
  while disabled). Eighteen more Lucide outlines are vendored for a formatting
  toolbar: `Bold`, `Italic`, `Underline`, `Strikethrough`, `TextAlignStart`,
  `TextAlignCenter`, `TextAlignEnd`, `TextAlignJustify`, `ListOrdered`,
  `IndentIncrease`, `IndentDecrease`, `Heading1`, `Heading2`, `Heading3`,
  `TextQuote`, `Link`, `FileText` and `WrapText`. The `xui-rich-text` wordpad
  example's formatting row now uses them, with tooltips on the icon-only
  buttons and icons in the block-kind picker.
- **Editable rich text (`xui-rich-text`).** An optional, portable crate (also
  the `rich-text` feature of `xui`) with styled runs, paragraph formatting,
  headings, quotes, bullet and numbered lists, and inline or floating images.
  It is built in testable layers: an invertible-edit document model with
  history, a flow layout around floats, a UI-free `edit::EditorState` driven by
  `Command`s (with image resize handles), and a `RichTextEditor` view. Documents
  save as versioned JSON (`to_json`/`from_json`) and export to GFM Markdown
  (`to_markdown`, `ImageExport`).
- **`TextLayout::baseline`.** The distance from the top of a text layout to its
  first line's baseline, so runs of different sizes can sit on one baseline.
  The default is an approximation; backends may override it.
- **`HtmlView` has a vertical scrollbar.** The view draws xui-core's shared
  scrollbar along its right edge (same geometry, painter and theme tokens as
  `ScrollView`/`ListView`): drag the thumb, click the track to page. Before,
  the page scrolled only with the wheel and keys and showed no position. The
  bar's strip is always reserved, so the page lays out narrower by its width.
- **`xui-canvas` builds without a windowing system.** An on-by-default
  `winit-backend` feature gates `winit`, `softbuffer`, `glutin`, `glow`,
  `arboard`, the `windows` double-click metrics and `xui-gpu`; with
  `default-features = false` the crate is the software painter core over
  `xui-core`, `tiny-skia` and `cosmic-text` (`SkiaCanvas`, `Surface`,
  `measure_text`, `OffscreenBackend`), with `tests/deps.rs` guarding the
  dependency tree. It also gains in-memory fonts
  (`set_default_font`/`add_font`/`set_default_family`, which skip
  `load_system_fonts` and any file memory-mapping), per-line horizontal
  alignment for natural-width runs, and `Surface::pixels` for a clone-free
  present — everything a custom backend such as LazyOS needs.

- **Icon set (`xui-icons`).** An optional crate (also the `icons` feature of
  `xui`) with 36 multi-colour vector icons in four categories. `Icon` names an
  icon and `draw(canvas, icon, rect, &Palette)` draws it through the `Canvas`
  path calls, so `xui-canvas` rasterises it with `tiny-skia`. Two looks are
  vendored and the one you build is the only one compiled in: flat 90s "Global
  Village" (default, recolourable with `Palette`) and glossy Vista-style "Aero"
  (`--features aero`). SVG, PNG and `.ico` exports live under
  `crates/xui-icons/assets/<style>/`; `generate.py` compiles the SVGs to Rust.
- **Gradient paths on `Canvas`.** `Canvas::fill_path_linear` fills a path with a
  `PathGradient` (path-space end points, borrowed stops). `xui-canvas`
  implements it with `tiny-skia`; other backends fall back to the gradient's
  middle colour until they override it.
- **Portable spatial file explorer.** New `xui-explorer` crate: one window per
  open folder, an `IconView` listing (folders first, then files, grouped and
  sorted case-insensitively), a `StatusBar` summary, a context menu, Delete /
  Properties dialogs and `Delete` / `Alt+Enter` / `F5` shortcuts. All OS
  specifics sit behind two object-safe traits — `Platform` (list, symlink-aware
  metadata, delete, home) and `Launcher` — that mention only `Path`/`OsStr` and
  `io::Result`; a `testing::MemPlatform` backs every test so none touches the
  real disk, and the default `std-platform` feature supplies `StdPlatform` and
  `DesktopLauncher`. A target with its own filesystem and shell (such as LazyOS)
  implements the two traits plus a `Backend` and nothing else. Deletion never
  follows a symlink, refuses a filesystem root, always asks first and reports
  per-item failures; a shared path-to-window registry makes an already-open
  folder a no-op and closes the windows below a deleted folder.
- **Portable `IconView`.** `xui-core` gains `widget::IconView`, a virtualized
  Windows XP-style icon view: tiles of an icon plus up to three ellipsised text
  lines, flowed left to right and wrapped, with a scrollbar, three icon sizes
  (`IconSize::{Small, Medium, Large}` = 16/32/48 DIP, default `Large`),
  single/multi/range selection, hover/selected/focused/disabled states from
  semantic tokens, and pointer, keyboard, wheel and context interaction. It is
  backed by an `IconModel` and reuses `CellData`. It lives in `xui-core` and
  draws only through the `Backend` contract, so it runs on every backend — the
  software (`xui-canvas`) path included — with no new dependencies and no
  `unsafe`.
- **Global Village tiles and an open-folder flash in `xui-explorer`.** The
  explorer draws its tiles with `xui-icons` under the new default
  `village-icons` feature (a shared `FileClass` picks folder/image/music/
  archive/document; on a dark theme only the set's ink is retinted so outlines
  stay visible); with the feature off it falls back to the Lucide icons.
  Opening a folder, including one already open, shows its open icon for two
  seconds via a per-window list pruned by a single repeating timer that stops
  when idle and is stopped on window close.
- `IconModel::paint_icon` lets a model draw a multi-colour or app-drawn tile
  icon itself: `IconView` calls it for each visible tile first and falls back to
  the single-colour `IconModel::icon` when it returns `false`. `IconView::invalidate`
  repaints after a live appearance change.
- **Portable file open/save picker.** `xui-core` gains `widget::FileDialog`
  (open and save modes) with a constructor-closure API (`open_file`/`save_file`,
  `initial_dir`, `suggested_name`, `filter`, `require_existing`,
  `on_accept(PathBuf)`, `on_cancel`), keyboard navigation, type-to-filter,
  extension filtering and overwrite confirmation. It never writes: it lists
  directories through a new `FileSystem` seam (`StdFileSystem` uses only
  `std::fs`/`std::env`; an in-memory impl backs the tests) that a partial
  filesystem such as LazyOS can replace. `Backend::file_dialog` is a new
  optional hook — the default declines, so the portable themed modal is used
  everywhere and the app cannot tell which picker answered. No new
  dependencies, no `unsafe`. The notepad example's Open and Save As now use it
  instead of a typed-path prompt (#235).
- `xui-code-editor` gains `document`, a UI-free file model: line-ending detection
  and round-tripping (LF/CRLF), a preserved UTF-8 BOM, atomic saves (temp file,
  `sync_all`, rename, resolved symlinks, preserved permissions) and a
  revision-based dirty state, with `DocumentError` variants for I/O, invalid
  UTF-8 (with the byte offset), oversize files and non-files. `search` is a
  find/replace session over `find` for a thin UI, with match counts, the current
  index and capture-group replacement. `Editor` gains `revision` and
  `caret_line_col` accessors.
- `Ui::on_key` maps shortcut keys to the app's message ahead of the focused
  widget; the event still reaches the widget, so a shortcut must be a key the
  widget leaves unhandled. `Dialog::set_message` replaces a message/confirm
  dialog's body at run time.
- `xui-code-editor`'s `notepad` example is now a small cross-platform text
  editor: a menu bar, a find/replace bar, a status bar, keyboard shortcuts
  (Ctrl/Cmd, F3), unsaved-change confirmations, error dialogs and atomic saves,
  running on `xui_canvas::WinitBackend` on Windows, Linux and macOS. Open and
  Save As use the portable `FileDialog`; a path can also come from an argument.

- `xui-code-editor`: a reusable code-editor widget (rope buffer with undo, monospace view, find/replace, diagnostics markers, pluggable `Highlighter`, and the `rhai-syntax` feature for a Rhai lexer), moved from LazyRAD (#231). It copies and pastes through the backend's portable clipboard; `Editor::with_clipboard` overrides it.
- The shared scrollbar is public as `xui_core::widget::scrollbar` (#231): `ScrollBar` (vertical or horizontal, over a bar node), `Scroll`, and the geometry and painting helpers (`thumb`, `hit`, `offset_from_drag`, `paged_offset`, `paint_state`), so a custom widget that paints its own bar matches `ScrollView`, `ListView` and `TreeView`. `ScrollBar` is also re-exported from `xui_core::widget`.
- `Ui::clipboard_text` and `Ui::set_clipboard_text` are public.
- `Backend::set_window_icon(window, &Image)` sets a top-level window's icon from
  portable RGBA pixels (no-op by default). The canvas (`winit`) backend applies
  it with `Window::set_window_icon`, remembering an icon set before the native
  window exists; the Win32 backend converts it to an `Icon`; `OffscreenBackend`
  records it and exposes `window_icon(window)` for app tests.

### Fixes

- `ColorPicker` now hit-tests in its own coordinates. Pointer events arrive relative to the node while `Ui::bounds` is relative to the parent, and the picker compared the two, so a picker not at the top-left of its container ignored every click and never showed hover. The same applied to the swatch grid inside `ColorPanel`.
- `xui-code-editor` now paints text when the highlighter emits no tokens
  (`PlainText`, the default). Previously `paint` drew only token spans, so a
  plain-text editor showed an empty grid; a tokenless line is now drawn as one
  plain token in the editor's text colour.
- The canvas backend no longer types the letter of a Ctrl (or Windows/Command) shortcut: `winit` reports Ctrl+C with the text "c", which was delivered as a `Char` after the shortcut, so copy and paste also inserted a letter. Ctrl+Alt (AltGr) still types.
- The canvas (`winit`) backend delivers typed text for keys that have no portable
  `Key` code: punctuation, symbols and accented letters (`;`, `{`, `é`, AltGr
  combinations on non-US layouts) were dropped entirely, so they could not be
  typed in any text widget.
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
