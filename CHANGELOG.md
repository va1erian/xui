# Changelog

## Unreleased

### Breaking changes

- **Widgets are built only through builders in layouts.** Every
  `Widget::new(ui, Rect, ..)` rect constructor of the portable widgets
  (`Button`, `Label`, `Edit`, `ListView::new`/`with_model`, `TreeView`,
  `GridView`, `IconView`, `Panel::new`/`plain`, `Split::row`/`column`,
  `Toolbar::new`/`empty`, `TopBar::new`, `Menu::bar`, `Separator::new`/
  `vertical`, `ColorPanel`, `ColorPicker`, `ColorField`, `HueSlider`,
  `RadioGroup`, `ScrollView`, `Tabs`, `GroupBox`, `FlowText`,
  `MaterialStatusBar`, ...) is now crate-private, and the `::auto`
  constructors are gone: use the `arrange` builders (`button("OK")`,
  `list()`, `tree_view()`, `panel(layout)`, `split(a, b)`, `toolbar()`,
  `menu_bar(..)`, ...) in a layout mounted with `Ui::root`/`mount`/`mount_in`,
  or `absolute()` with `.at(x, y, w, h)` where free placement is the point.
  `set_bounds` is gone from `Panel`, `ScrollView`, `Split`, `ColorPanel`,
  `ColorField` and `HueSlider` (layouts place them); `Control::new` stays for
  custom widgets, which implement `Placeable` and go in through
  `build(|ui| ..)`. Dialogs, menus' `Menu::context`, `Tooltip::attach` and
  `TaskDialog` keep their constructors.
- **Layouts take builders, not widgets.** `arrange` layouts hold widget
  builders (`label("...")`, `button("...").on_click(Msg::X)`, `edit()`,
  `list()`, `group(title, layout)`, `tabs().page(..)`, or `build(|ui| ...)`
  for any other widget), created at mount time with the right parent.
  Already-built widgets (by value, as a `Result`, or as `&Rc<W>`) and
  `arrange::widget` are gone: bind a `Handle<W>` instead.
  `Layout::spacing`/`margins` are now `gap` and `padding` and take plain
  numbers.
- **A nested row or column takes its natural size** along its parent's main
  axis instead of filling it; add `.fill(1)` where it should take the rest.
- **`Placeable::natural_size(ui, dpi)` is now `measure(ui, constraints)`**,
  with the DPI in `Constraints::dpi`; `layout::Leaf` gains `content`, and the
  tree's leaf callback takes `Constraints`.
- **Layouts re-flow on their own after a change, once per event.**
  `Ui::set_visible` no longer re-lays the window at once: like a text,
  icon, title or theme change, it marks the layouts dirty and they re-flow
  once the event being handled is done (before the next paint). Read a
  widget's new bounds in the same handler only after `ui.relayout()`, which is
  otherwise rarely needed now. `group(..)` returns a `GroupBuild` (bindable)
  instead of an `Entry`; `Insets::apply` keeps the result inside the rect
  when the insets are larger than it.
- **`xui-form`: a form nests at most `MAX_DEPTH` (16) levels, and
  `Form::to_ron` returns a `Result`.** The reader and the writer share the
  limit, so a form that is written always loads again. `load` reports a
  deeper node at its line and column; `to_ron` returns
  `WriteError::TooDeep` (or `WriteError::Unwritable`) instead of panicking or
  writing a file `load` rejects; `Form::validate` reports it and `describe`
  returns the new `BuildError::Limit`. `migrate::from_v1` re-loads its own
  output and returns the new `MigrateError::Write` when it does not match.
  Callers of `to_ron` handle the error; forms nested between 17 and about 30
  levels, which loaded before depending on their shape, no longer load. A
  container's content that is the default (omitted from a file) is no level.
  A format-1 form converts only up to seven nested containers: each takes
  two levels (itself and the layout of its content), so a deeper one is
  refused by `migrate`, naming the node.
- **`xui-form`: forms that validated before can fail now, and `render`
  refuses them.** `Form::validate` (and `xui-form check`) reports as errors a
  widget named `form` (`validate::RESERVED_NAMES`), a `selected` past the
  items or pages (a warning for a `ListView`, whose items a script may
  replace), a `min` above `max`, `array: 0`, `array` or `index` on a `Panel`,
  `Group` or `Tabs`, an `array` above `MAX_ARRAY` (1024) or an `index` at or
  past it, a length that is not finite or beyond `MAX_LENGTH` (32768), a
  `ProgressBar` `value` or `max` outside the `i32` range and a float
  property that is not finite. `xui-form render` now validates first and
  renders nothing for a form that has errors (warnings are printed and it
  goes on); `migrate` validates what it converted and refuses a result
  `check` would reject. `ValueType::accepts` refuses a float that is not
  finite, so a script write of `NaN` is a type mismatch.

- **`Split`'s `position` property is a `Dip`.** It was a `Float`; the generic
  property surface now has a design-unit type.

### Additions

- **The property surface is typed and described.** `xui_core::property::Value`
  gains `Enum`, `Color`, `Font`, `Dip` and `Image` (with a `Font` type and
  accessors), and `Properties::schema()` returns one `PropertyInfo` per
  property — its `PropertyType`, category, description and whether it is
  read-only — so a designer can build the right editor. `Label` gains a font
  override (`font(..)`, `set_font(..)`, `current_font()`, reported as a `Font`
  property: `Font.Size`/`Bold` on a label, G17's v1), `ColorPicker` reports
  its `color`, `IconView` reports `icon_size`, and `Split` reports `position`
  as a `Dip`.
- **The HTML views are placeable.** `xui-blitz::BlitzView` and
  `xui-litehtml::HtmlView` implement `Placeable`: a layout gives them their
  slot (`build(|ui| ...)` with `Rect::default()`), the view re-renders at that
  size, and a `BlitzView`'s scrollbar follows its right edge. Like the code
  editor and the rich-text editor they report no natural size of their own,
  so give them a `fill(1)` or a fixed size.
- **A layout entry can keep a width-to-height ratio.** `.aspect(16.0 / 9.0)`
  (on any entry, in any container but `absolute()`, which places by `at` and
  `anchor`) keeps the entry at the largest ratio-correct size that fits the
  space it is given, within its caps: the axis its sizing fixes keeps its
  extent and the other follows the ratio, otherwise the cross extent follows
  the main one. A `.lfm` form has the same `aspect` layout field (finite and
  positive; setting it inside an `Absolute` layout warns), and
  `xui-form schema --json` reports it.
- **The canvas backends repaint only what changed.** `WinitBackend` and
  `OffscreenBackend` share one compositor and keep each window's frame between
  frames. A move, resize, show, hide, raise, destroy, clip, text or painter
  change marks the old and new places of the node (and everything inside it),
  an input event (a press, a release, a wheel step, a key, a character, a
  focus or capture change, the pointer leaving; not a plain pointer move)
  marks the node it was delivered to, handled or not (the random-input cost:
  mean damage about 49.8k px per frame against 24.0k with no rule), and
  `invalidate_rect`, which was ignored, is honoured; the window repaints only
  that damage (a list of at most eight rectangles; a resize, DPI or theme
  change and GL content repaint the whole window) and the windowed backend
  hands `softbuffer` only the repainted rows when `Buffer::age() == 1`,
  converting the whole frame otherwise (always, on X11, where an expose cannot
  be told from a redraw). An idle frame paints nothing and allocates nothing;
  at 1080p, with a card of about 30 widgets, an idle frame went from about 4 to
  6 ms to 0 ms and a card moved by one pixel from about 4 to 6 ms to about
  0.4 ms on the flat themes and about 2.6 ms on Midnight (release,
  `damage_bench`, a 20-thread desktop CPU; machine dependent).
  **A painter cannot draw outside its reach**: its bounds, trimmed to its
  ancestors' clips and grown by an overdraw margin of 8 design units (8 px at
  96 DPI, 12 at 144). The compositor clips every painter to it, in a frame of
  damage and of the whole window alike, so a label whose text overflows its
  node by more than the margin, or a shadow, is cut there (in every canvas
  frame, not only in repaints of damage); a widget that has to draw further
  out must give its node bounds that cover it. Lines, outlines and glows are
  now cut by a rectangular clip as fills are (they were not, so a scrolled
  checkbox's glow showed above its viewport). A stroke is centred on the edge
  of its rectangle, so a frame stroked under a clip of that same rectangle
  loses its outer half: the scroll view, the code editor and the paint canvas
  draw theirs outside their clip, and a widget that frames itself must too. For
  painters that stay within their reach the frames are bit-for-bit those of a
  whole-window repaint: the differential tests compare them after every step
  of seeded random operation sequences (at 96, 120, 144 and 192 DPI) and of
  scripted sessions of the real widgets on the light, dark and Midnight
  themes. `OffscreenBackend::inject` now delivers what the windowed backend
  delivers (`MouseLeave` to the node the pointer left, `KillFocus` and
  `SetFocus` on a press and on `Backend::focus`), so a snapshot of a focused
  widget shows what a window shows (a focus ring and caret, the active
  selection colour), and `Edit` and `NumberField` repaint on a focus change.
  New API: `OffscreenBackend::present` lends the frame and the rectangles
  repainted without a copy, `PresentedFrame`, and
  `OffscreenBackend::invalidate_window` for a test that changed what a painter
  draws behind the backend's back; `render` returns the image it did, for
  painters inside their reach. The winit present path is compile-checked, not
  run. See [Backends](docs/backends.md#repainting-only-what-changed).
- **A portable motion foundation, and the reduced-motion setting.**
  `xui_core::motion` adds `Easing` (linear, quadratic and cubic in/out/in-out,
  and the Windows Phone Toolkit's `ExpoIn`/`ExpoOut` with an `exponent`,
  default 6), `Tween` (`from`, `to`, `duration`, `delay`, `easing`; a pure
  `sample(elapsed)` and `is_finished`), and a `Clock` trait with a
  `ManualClock` for tests and headless runs. `xui-core` reads no clock and uses
  nothing that is unavailable on `wasm32` (not yet compiled for it in CI);
  `xui_canvas::MonotonicClock` is the real one.
  `Backend::prefers_reduced_motion()` (default `false`, so existing backends
  are unchanged) and `Ui::prefers_reduced_motion()` carry the system's
  animation setting: the Win32 backend and the windowed canvas backend read
  `SPI_GETCLIENTAREAANIMATION` on Windows; macOS and Linux report `false`, and
  `OffscreenBackend` reports `false` unless `set_prefers_reduced_motion` says
  otherwise.
  When it is set, motion resolves to a plain fade or an instant change and
  nothing loops. Per-node opacity and transform, a frame driver and widget
  animations are separate, later steps. See [Motion](docs/motion.md).
- **xui-blitz: a scrollbar and a context-menu event.** `BlitzView` draws the
  toolkit's vertical scrollbar along its right edge (a gutter the page is
  laid out beside, so it never covers content; drag the thumb or click the
  track), and a right click raises `BlitzViewEvent::ContextMenu { x, y,
  link, image }` with the absolute URLs of the link and picture under the
  pointer, for a host to offer Save Image As (`BlitzView::download`) or Open
  Link. `BlitzView::id` gives the view's node for placing the popup.

- **xui-blitz: a web view on the Blitz engine**, to replace both
  `xui-litehtml` and `xui-netsurf`. `BlitzView::builder(on_frame, on_event)`
  starts from HTML (`.html(..).base_url(..)`) or a URL (`.url(..)`); with
  `follow_links(false)` (the default) links and forms come back as
  `LinkClicked`, with `follow_links(true)` the view opens them. Pages load
  through the same `Fetcher`/`Downloader` contract `xui-netsurf` had (redirects
  are followed by the view), and it reports the title, URL, loading, status
  text, `LaunchUrl`, `CopyRequested`, fetch failures and downloads. Blitz
  (Stylo, Taffy, Parley; pinned from crates.io, not forked) brings CSS grid,
  `var()`, opacity, SVG, floats, overlay scrollbars, text selection and form
  fields; the theme's light or dark reaches `prefers-color-scheme`. MIT, no C,
  no `unsafe`. `scripts/engine-compare/` renders fixtures with litehtml,
  NetSurf, Blitz and Chrome and measures each engine's binary size.

- **xui-netsurf: what a browser needs from the view.** `NetSurfView` reports
  NetSurf's status line (`StatusChanged`: the link under the pointer, load
  progress) and links it cannot follow (`LaunchUrl`, e.g. `mailto:`), can
  `stop` a load, and downloads what NetSurf cannot show, or what
  `download(url)` asks for, through a host `Downloader` (`set_downloader`)
  with `DownloadStarted`/`DownloadProgress`/`DownloadFinished` events and
  `cancel_download`.

- **`Cursor::Busy`** is the busy (wait) pointer: `winit`'s `Wait` and Win32's
  `IDC_WAIT`. `Cursor` has no `#[non_exhaustive]`, so a backend that matches it
  exhaustively needs the new arm. `xui-netsurf` now maps NetSurf's pointer
  shapes (hand over links, I-beam over fields, busy while loading, the resize
  arrows) to cursors and applies them to its `NetSurfView`.

- **Declarative forms (`xui::form`, feature `form`).** The `xui-form` crate
  moves in from LazyRAD with a new file format: `.lfm` format 2 is a RON
  tree of layouts (`Row`, `Column`, `Wrap`, `Grid`, `Absolute`), containers
  (`Panel`, `Group`, `Tabs`) and widgets, whose plain serde types in
  `xui::form::model` also generate the `Catalog` schema. Defaults are
  omitted and the output is canonical and byte-stable; `load` reports a
  misspelt field or kind with a "did you mean". Control arrays
  (`Button(name: "digit", array: 10)`) share one handler that gets the
  index. `build`/`describe` produce the same `arrange` builders Rust code
  writes, and `Handlers` attaches closures by name. Format-1 TOML forms are
  converted by `xui_form::migrate` (feature `migrate`), not loaded at
  runtime. See [Forms](docs/forms.md) and `examples/form.rs`.
- **The `xui-form` tool** (`crates/xui-form-cli`): `check` (located errors
  with a "did you mean"), `render` (light and dark PNGs headlessly, with an
  optional script, layout overlay and layout report), `fmt`, `migrate`
  (format 1 to 2) and `schema --json`. See [Forms](docs/forms.md).
- **Rhai-scripted forms (`xui::script`, feature `rhai`).** The LazyRAD
  `xui-rhai` crate moves in as `xui-script`: `fn <control>_<event>`
  handlers, the `on_var` control resolver, `form.state`, the operation
  budget and located `ScriptError`s; a control array is a Rhai array
  (`digit[3].text`). Rhai is pinned to `=1.26.1` and stays out of the
  default build. See the calculator in `examples/script.rs`.
- **`layout::Placement`**: an absolute entry's rectangle and anchor in a
  shared cell (`LayoutExt::placement`, `Item::placement`), read each time the
  layout runs, so a designer or a script can move an entry after the mount.
- `TabsBuild::on_change_with`, for a page-change mapping that may raise
  nothing.

- **Declarative app setup.** `xui::app(title).size(w, h).run(|ui| ...)` opens
  a window on the default backend (or `xui_core::app` with `.backend(..)`),
  applies the backend's new `Backend::system_theme`, returns the first widget
  error, honours `XUI_DEMO_AUTOCLOSE_MS`, and with `XUI_SNAPSHOT=<dir>` saves
  light and dark screenshots headlessly. `Ui::root(layout)` keeps a layout
  for the window's lifetime, `Ui::every(ms, msg)` raises a message
  periodically, and `xui::prelude` brings builders, layouts, handles and
  widget types in.
- **Layout v2.** `grid([Track::Auto, Track::Fill(1)])` with column spans,
  per-entry `align`, `max_width`/`max_height`, a layout's `align`/`justify`,
  `group(title, layout)` (a `GroupBox` framing a layout), `Tabs` pages built
  from layouts (`Tabs::add_layout_page`), height-for-width measuring through
  `Constraints`, and `ListView::refresh_model` for live tables that keep their
  scroll position.
- **More containers.** `wrap()` breaks entries into lines, `scroll(layout)`
  scrolls a layout measured at the view's width (`ScrollView::set_layout`),
  `overlay()` and `stack()` layer entries (centred and stretched by default),
  and `absolute()` places entries `.at(x, y, w, h)` with an `.anchor(..)`
  against a `design_size`: the one home for free positions. The pure tree
  gains `Group::wrap`/`layered`/`absolute`, `Item::at`/`anchor` and
  `Group::measure`.
- **Layout debugging.** `Ui::layout_report()` dumps every mounted layout's
  tree and rectangles with `!` warnings for clipping, zero size, truncated
  text and overlap inside rows, columns, grids and wraps (from the pure
  `Group::trace`); `Ui::layout_rects()` and
  `Snapshot::with_layout_overlay()` outline them on a headless capture.
- **Every widget placeable.** `Toolbar`, `TopBar`, a `Menu` bar, `Split`,
  `GridView`, `FlowText` (height for width), `MaterialStatusBar`,
  `ColorField` and `HueSlider` measure a natural size from their content, and
  a button with an icon counts it (an icon-only one is square). Builders:
  `toolbar()`, `top_bar()`, `menu_bar(|menus| ..)`, `grid_view(..)`,
  `icon_view(..)`, `flow_text().run(..)`, `material_status_bar(..)`,
  `color_field(..)`, `hue_slider(..)` and `split(a, b)` (two layouts with a
  divider, `Split::set_layouts`). `development.md` makes `measure` a
  required step for a new widget.
- **Per-axis alignment and a fixed size.** `.align_x(..)`/`.align_y(..)`
  place an entry on one axis (a caption left-aligned and vertically centred
  in its grid cell), and `.size(w, h)` fixes both extents at once;
  `Item::align_x`/`align_y`/`size` in the pure tree. A `fill` item in a grid
  aligned within its row keeps its natural size instead of collapsing.
- **Builder gaps.** A `group(..)` can be bound and hidden as one unit (its
  content hides with it), `children` takes tuples of up to 24, a widget
  shared through `Rc<W>` is `Placeable` (place it with
  `build(move |_| Ok(rc))`), `button`/`toggle_button` take `.icon(..)` and
  `.tooltip(..)` (`Button::set_tooltip`, `ToggleButton::set_tooltip`) and
  `button` takes `.primary()`. `Ui::invalidate_layout` lets a custom widget
  ask for a re-flow.

- **More widgets in layouts.** `RadioGroup`, `TreeView`, `ColorPicker`,
  `ColorPanel`, `Panel` and `ScrollView` implement `Placeable` with a natural
  size from their content and the design tokens (a radio group stacks its
  options under the node a layout places; a scroll view and a tree re-lay
  their scrollbars when placed). New builders: `radio_group(&[..])`,
  `tree_view().rows(..)`, `color_picker(&colors)`, `color_panel()` and
  `panel(layout)` (`Panel::set_layout`); the prelude adds those widget types.
  `Layout::children` takes a `Vec` of any builder, layout or entry type.

- **Password fields.** `Edit::password(true)` masks a single-line field for a
  secret: it paints one bullet (U+2022) per character, with the caret,
  selection and clicks measured on the bullets; refuses copy and cut (paste and
  undo still work); treats the text as one word for Ctrl+arrows,
  Ctrl+Backspace/Delete and double-click; and reports its `text` property as
  bullets. `text()` and `on_change` still deliver the real text, and the cue
  still shows while it is empty. `Edit::is_password` reads the mode. On Win32
  the native `EDIT` gets `EM_SETPASSWORDCHAR` through the new
  `Backend::set_password` (a no-op by default) and `Ui::set_password`.
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
- `xui-code-editor` indents like a modern code editor (#285): Tab advances to the next
  tab stop (display columns, so a tab character counts) and replaces a selection inside one line,
  Backspace in leading spaces goes back to the previous stop, Enter adds a level after
  `{`, `(` or `[` and splits a bracket pair onto three lines, and a closer typed on a
  whitespace-only line dedents it. The new `Options::smart_indent` (default on) gates
  the Enter and closer rules.
- `xui-code-editor` gains a language-agnostic completion popup (#287). The host
  implements `Completer` (a closure works) and passes it to `Editor::with_completer`;
  Ctrl+Space, a second identifier character, `.` and `::` open it, typing filters it,
  Enter/Tab/click accept it as one undo step. `Editor::is_completing`,
  `trigger_completion` and `close_completion` drive it from the host, and `EditorTheme`
  has popup colours.

### Fixes

- `xui-form` format 2 hardening (the minors of the first review): a length
  is finite and within `MAX_LENGTH` (32768) design units, so `load` rejects
  `width: 1e38` (two of them panicked `render` in a debug build) and
  `Form::to_ron` will not write one. The sums are not bounded by the limit
  (six `Button(array: 1024, width: 32768)` in a `Wrap(gap: 32768)` panicked
  `render --dpi 1024`), so `xui-core`'s stack, absolute, wrap and grid
  layouts saturate their pixel sums at `i32::MAX` instead of overflowing. A
  `ProgressBar`'s `value` and `max` are bounded to the `i32` range
  (`max: 4294967296` read back as 1). A `NaN` bound no longer panics a build
  (`f64::clamp` panics on one in a `Slider`): `ValueType::accepts` refuses a
  float that is not finite and the build falls back to the default for one.
  `validate` reports a widget named `form`
  (`validate::RESERVED_NAMES`), a `selected` past the items or pages and a
  `min` above `max`. `load` ignores a byte order mark, points a
  `#`-commented format-1 file at `migrate`, and locates `fill: -1` at its
  sign; `{index}` is also replaced in the items of a control array.
- `xui-form` tool: `check` goes on past a file it cannot read, `render`
  validates and refuses a form with errors (printing the diagnostics),
  `fmt` replaces a file in one step, leaves one it cannot load as it
  was and notes the comments it drops, `migrate` refuses `--out` the file it
  converts, and `schema --json` no longer leaks `$crate::model::` into field
  types. `*.lfm` is `eol=lf`
  in `.gitattributes`, so `fmt --check` holds in a Windows checkout.
- `xui-form migrate` builds a grid only where it lands on the format-1
  pixels. A grid is padded on every side, so the converted rectangles must
  leave at least the padding right of the last column and below the last row
  (two buttons filling a 144-wide window were squeezed to 67 and 53 wide);
  otherwise the layout stays `Absolute`. The content of a `Group` is inset by
  its frame and title, so it stays `Absolute` too (64-wide buttons in a group
  box were squeezed to 60); a `Panel`'s content, card or not, has the panel's
  whole rectangle and may still be a grid.
- `xui-form migrate` no longer drops nodes or overflows the stack on an invalid
  format-1 form. Names that are not unique, a `parent` that does not exist or
  is not a `Panel` or `GroupBox`, a parent cycle and a position that is not an
  integer are reported with their node (the new `MigrateError::Invalid`,
  listing `NodeProblem`s) and nothing is converted; unknown `[window]` and
  top-level keys are noted like `tab_index`. The grid detection uses checked
  arithmetic, so extreme coordinates no longer panic in debug builds, and the
  conversion visits each node once.
- `xui-form`: control arrays are bounded. `array` is at most
  `MAX_ARRAY` (1024) and an `index` is below it; `array: 0` and `array` or
  `index` on a `Panel`, `Group` or `Tabs` are validation errors, and
  `describe` refuses a form that breaks the cap or repeats a container
  (`BuildError::Limit`) instead of building it. Name clashes are checked by
  ranges, a build's name de-duplication is logarithmic rather than
  quadratic, and `xui-script` no longer grows an array to reach an element's
  index. `Button(array: 100000000)` took 770 MB to check.
- `xui-form`: `Form::to_ron` no longer panics near RON's recursion limit
  (40 nested v1 panels migrated to a panic, 20 to a file `check` rejected),
  and a hand-written form at the same depth is read and written alike. The
  deserialiser's stack use is bounded by the depth limit.
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

- `xui-core`: `Image::into_pixels` gives the pixel buffer back, and `xui-canvas`'s image cache writes an evicted upload over the next one of the same size, so a producer of one image per frame (a software renderer, a video player) allocates no buffer per frame once its first few are in flight.
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
