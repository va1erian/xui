# Plan: an editable rich-text component (`xui-rich-text`)

Status: draft for review · Branch: `claude/rich-text-component-plan-d43688`

## Goal

A portable, editable rich-text widget solid enough to build a basic word
processor on: styled runs, paragraph formatting, lists, inline images, and
images that text flows around. It must run unchanged on every backend —
`xui-canvas` (winit + tiny-skia / GPU), `xui-win32` (Direct2D) and the
offscreen/headless backend — so it is built only on the portable `Canvas` and
`TextShaper` contracts, with no platform code.

### In scope (v1)

- Character styles: family, size, weight, italic, underline, strikethrough,
  colour, highlight, link, super/subscript.
- Paragraph styles: alignment (left/centre/right/justify), left/right/first-line
  indent, space before/after, line spacing, bulleted and numbered lists with
  nesting.
- Inline images (sit on the baseline like a large glyph) and floating images
  (left/right, wrap *square* and *top-and-bottom*) that text flows around,
  across paragraph boundaries.
- Caret, selection (mouse, keyboard, double/triple click), full keyboard
  navigation, typing, undo/redo with coalescing, cut/copy/paste (plain text to
  the OS, rich within the process), image selection and resize handles.
- Scrolling with the shared scrollbar, light and dark theme, headless snapshot
  tests.

### Later (designed for, not built in v1)

Page view/pagination and print, IME composition, bidi reordering, rich OS
clipboard (HTML/RTF/images), tables, tight/polygon wrap, accessibility text
pattern, spell-check squiggles, Markdown import.

### Not planned

HTML import/export and DOCX/RTF files. Saving is the native JSON format;
sharing is Markdown export.

### Decisions (2026-10-03)

1. **No page view in v1** — continuous layout only. The `FlowArea` seam stays so
   pagination can be added later without a rewrite.
2. **Persistence is native JSON plus Markdown export**; no HTML.
3. **No IME in v1** — text input is `Event::Char`, like every xui text widget
   today. IME stays a separate backend issue.
4. **Opt-in component**: its own crate, `xui-rich-text`, behind the umbrella's
   off-by-default `rich-text` feature.

## What already exists and what we reuse

| Piece | Where | Use |
|---|---|---|
| `TextShaper` / `TextLayout` (`Send + Sync`, hit-test, selection rects) | `xui-core/src/backend/text.rs`; cosmic-text in `xui-canvas`, DirectWrite in `xui-win32`, deterministic monospace in headless | Shape every text piece. |
| `Canvas::draw_layout`, `draw_image`, clips, translation | `xui-core/src/backend/canvas.rs` | All painting. |
| `Image` (RGBA8, decode PNG/JPEG, resample, stable `id`) | `xui-core/src/image.rs` | Image payload, `Arc<Image>`. |
| Word-per-run layout + selection over runs | `xui-litehtml` (`text_runs.rs`, `selection/`) | Proven approach to copy: shape per word, place words ourselves. |
| Transactional undo, clipboard seam, caret timer, key handling | `xui-code-editor` (`buffer.rs`, `platform.rs`, `events.rs`) | Patterns for history coalescing and the `Clipboard` trait. |
| Shared scrollbar (`paint_state` / `hit` / `offset_from_drag`) | `xui-core` scrollbar module (used by `HtmlView`, #250) | Vertical scrollbar. |
| `Custom` node + painter + event mapper, `Control<M>` | `xui-core` widget layer | Widget shell. |
| `xui_canvas::snapshot::render` | headless screenshots | Light/dark snapshots in tests. |

Key finding: the shaper shapes **one string in one `FontSpec`**, and neither
cosmic-text nor DirectWrite offers float exclusions. So the line breaker lives
in the portable layer: we segment a paragraph into pieces of uniform style
between break opportunities, shape each piece with the existing shaper, and fit
pieces into the line intervals the floats leave free. This is what litehtml
already does here, it needs almost nothing new from backends, and it gives
identical behaviour on all of them.

## Architecture

### Packaging: an opt-in component

`crates/xui-rich-text` is its own workspace crate and is never pulled in by
default:

- An app either depends on `xui-rich-text` directly, or enables the umbrella
  crate's `rich-text` feature (`rich-text = ["dep:xui-rich-text"]`, off by
  default, like `icons`), which re-exports it as `xui::xui_rich_text`.
- Its dependencies (`unicode-linebreak`, `unicode-segmentation`, optional
  `serde`/`serde_json`) therefore cost nothing to apps that do not use it.
- Inside the crate, `serde` (JSON save/load) is its own feature, on by
  default; Markdown export has no dependency and is always available.

### Layers

New workspace crate `crates/xui-rich-text`, a peer of `xui-code-editor`:
`#![forbid(unsafe_code)]`, depends on `xui-core` plus pure-Rust
`unicode-linebreak` (UAX #14) and `unicode-segmentation` (graphemes, words),
`serde` behind a feature. Three layers, each testable without a window:

```text
model/   pure document + edit operations + history   (no shaping, no UI)
layout/  pure flow engine over a &dyn TextShaper     (headless-testable)
view/    the widget: Custom node, paint, input, scroll, commands
```

### 1. Document model (`model/`)

- `Document { blocks: Vec<Paragraph>, styles: StyleTable, objects: ObjectTable }`.
  A `Vec` of paragraphs is enough for word-processor sizes (tens of thousands of
  paragraphs); an edit touches one or two. Swap for a chunked tree later if
  profiling says so — positions are opaque so callers don't notice.
- `Paragraph { text: String, spans: Vec<Span>, style: ParaStyleId }` where
  `Span { len: usize, style: CharStyleId }` run-length-encodes character style
  and always covers `text` exactly (an invariant tested by proptest).
- Styles are interned (`StyleTable`), so a span is two words, equality is an id
  compare, and the layout shape cache can key by id.
- `CharStyle { family, size: Dip, weight, italic, underline, strike,
  color: TextColor, highlight: Option<Color>, link: Option<String>,
  baseline: Baseline }`. `TextColor::Auto` follows the theme's `text` token
  (what makes dark mode work); `TextColor::Fixed(Color)` is document content.
- `ParaStyle { align, indent_left, indent_right, indent_first: Dip,
  space_before, space_after: Dip, line_spacing: LineSpacing, list: Option<ListItem>,
  kind: BlockKind }` with `BlockKind::{Body, Heading(1..=3), Quote}`. The kind
  carries default styling (a heading is larger and bold) and is what the
  Markdown export maps to `#`/`>`; it is also the word processor's "Normal /
  Heading 1" style picker.
- Images are **anchored objects**: the paragraph text holds `U+FFFC` (object
  replacement character) at the anchor, mapped to an `ObjectTable` entry
  `InlineImage { image: Arc<Image>, size: (Dip, Dip), wrap: Wrap, alt: String }`
  with `Wrap::{Inline, Square { side: Side, margin: Dip }, TopAndBottom { margin }}`.
  One char per object keeps caret movement, deletion, undo and copy uniform.
- Positions: `DocPos { para: usize, byte: usize }` (always a grapheme
  boundary) plus `Affinity` for the caret at a soft line break.
  `Selection { anchor, head }`, and `Selection::Object(ObjectId)` when an image
  is selected.
- Edits are invertible ops: `InsertText`, `Delete(range)`, `SplitParagraph`,
  `MergeParagraph`, `SetCharStyle(range, patch)`, `SetParaStyle(paras, patch)`,
  `InsertObject`, `SetObject`. `apply` returns the inverse; `History` groups ops
  into transactions and coalesces consecutive typing/backspace like the code
  editor. Style changes take a *patch* (`CharStylePatch { bold: Some(true), .. }`)
  so "toggle bold" over mixed text leaves other attributes alone.
- `Document::style_at(selection) -> StyleSummary` reports each attribute as
  `Uniform(v)` or `Mixed`, which is what a toolbar needs to show B/I/U state.

### 2. Layout engine (`layout/`)

Pipeline per paragraph:

1. **Segment** (`segment.rs`): split text at UAX #14 break opportunities and at
   span boundaries into `Piece`s (`Text { range, style } | Object(id) | Tab`),
   recording whether a break is allowed/mandatory after each.
2. **Shape** (`shape_cache.rs`): shape each text piece unwrapped
   (`max_width = INFINITY`) through `TextShaper`; cache by
   `(text, CharStyleId, dpi)` in a bounded LRU so retyping a paragraph reshapes
   only the changed words. Shaping happens in the layout pass, never in paint.
3. **Float context** (`floats.rs`): placed floats as exclusion rects;
   `free_intervals(y, height, area) -> Interval` returns the horizontal band a
   line of that height may use at `y` (v1: one interval between the left-side
   and right-side floats).
4. **Break lines** (`line.rs`): greedy fill. For each line, ask the float
   context for the interval at the current `y` using the tallest piece so far;
   if the line's height grows past a float edge, re-query. If the interval is
   narrower than the next unbreakable piece, drop `y` to the next float bottom
   and retry. Over-long words fall back to grapheme breaking. Then apply
   alignment and justification (stretch inter-word gaps; never the last line).
5. **Baselines**: a line's ascent/descent is the max over its pieces; inline
   images contribute their height as ascent. Line spacing multiplies or fixes
   the line height.
6. **Floats placement**: when the breaker reaches a float's anchor it places
   the float at the top of the current line (or below the previous float on that
   side), Word's "move with text" behaviour, then re-flows the current line with
   the new exclusion. `TopAndBottom` ends the line, places the image across the
   whole interval, and continues below it.
7. **Flow** (`flow.rs`): the document flows into a sequence of `FlowArea`s
   (width, optional height). Continuous view is one area of unbounded height;
   page view (later) is one area per page, so pagination is a new area
   provider, not a rewrite.

Output: `ParaLayout { y, height, lines: Vec<Line> }`, `Line { y, baseline,
height, pieces: Vec<PlacedPiece> }`, and placed floats. Each placed text piece
holds its `Arc<dyn TextLayout>`, so paint just calls `draw_layout`.

**Incremental relayout.** Edits mark paragraphs dirty. Relayout starts at the
first dirty paragraph and stops once a paragraph's input (top `y`, float
context entering it) and output height match the previous pass — usually after
one paragraph. Floats can push content across paragraph boundaries, which is
why the stop condition compares the float context, not just `y`. A width or DPI
change relays out viewport-first, then the rest in idle slices (timer), with
estimated heights for the scrollbar until done.

**Hit-testing** (`hit.rs`): `pos_at(point) -> DocPos` (line by `y`, piece by
`x`, then the piece's own `hit_test_point`, so complex scripts stay accurate),
`caret_rect(pos)`, `selection_rects(range)`, `object_at(point)`. Round-trip
`pos_at(caret_rect(p).center) == p` is a proptest property.

### 3. Widget (`view/`)

`RichTextEditor<M>` follows the widget layer: one `Custom` node, a painter and
an event mapper, events mapped to the app's `Msg` through closures fixed at
construction, `Themed`, `Dip` design values, no numeric ids.

```rust
let editor = RichTextEditor::new(ui, rect)?
    .document(Document::from_plain_text(text))
    .on_change(|change: &Change| Some(Msg::Edited))
    .on_selection(|sel: &SelectionInfo| Some(Msg::Selection(sel.style.clone())))
    .on_link(|url: &str| Some(Msg::OpenLink(url.to_owned())));

editor.exec(Command::ToggleBold);
editor.exec(Command::SetAlign(Align::Justify));
editor.insert_image(image, ImageOptions::new(Dip(240.0), Dip(160.0)).wrap(Wrap::square(Side::Left)));
```

- **Paint** (`paint.rs`): background, visible paragraphs only (binary search
  by `y`), selection rects behind text, highlights, text via `draw_layout`,
  underline/strike lines from line metrics, images via `draw_image`, list
  markers, caret, image selection handles, scrollbar. No per-piece allocation
  or shaping in paint.
- **Input** (`events.rs`, `keys.rs`): click/drag select, double-click word,
  triple-click paragraph, shift-extend, autoscroll while dragging off-edge;
  arrows by grapheme, Ctrl+arrows by word, Up/Down with a sticky x, Home/End,
  Ctrl+Home/End, PageUp/PageDown; Enter splits (Shift+Enter is a line break
  `U+2028`), Backspace/Delete merge at edges and exit a list item at its start;
  Tab/Shift+Tab indent list levels; Ctrl+B/I/U, Ctrl+Z/Y, Ctrl+X/C/V, Ctrl+A.
  Caret blink through `Control::set_timer`.
- **Images** (`objects.rs`): click selects an image; eight handles resize
  (corners keep aspect ratio); Delete removes; drag moves the anchor to the drop
  position. Wrap mode and side are commands, so an app can offer them in a
  context menu.
- **Commands** (`commands.rs`): a public `Command` enum (formatting, list,
  alignment, indent, insert image/link, undo/redo, select all) so toolbars,
  menus and keys share one path, each one undoable transaction.
- **Clipboard** (`clipboard.rs`): plain text through the portable clipboard
  (reuse the code editor's `Clipboard` trait shape). A copy also keeps the rich
  fragment in a thread-local store tagged with the plain text it wrote; paste
  uses the fragment when the OS text still matches, else inserts plain text.
- **Theme**: chrome (background, selection, caret, handles, scrollbar, list
  markers in `Auto` colour) uses semantic tokens only; `Auto` text follows
  `Theme::text`, so a default document reads correctly in dark mode. Fixed
  colours and highlights are document data and render as authored.
- **Accessibility**: `Role::Edit` with the plain-text value and selection;
  a full UIA text pattern is a later issue.

### 4. Persistence (`format/`)

- **Native JSON** (`format/json.rs`, `serde` feature): the lossless save
  format. Versioned (`{"version": 1, ...}`) so the model can grow; styles are
  written as the interned table plus ids; images as base64 PNG (re-encoded from
  `Image`, so a JPEG source round-trips as PNG). Load validates the invariants
  (spans cover text, every `U+FFFC` has an object) and returns a typed error
  rather than panicking on hand-edited files.
- **Markdown export** (`format/markdown.rs`, always on, no dependency): a
  lossy GitHub Flavored Markdown (GFM) writer for sharing basic documents.
  Everything but strikethrough is plain CommonMark.

  | Model | Markdown |
  |---|---|
  | `Heading(n)`, `Quote` | `#`×n, `> ` |
  | bold / italic / strike | `**`, `*`, `~~` (GFM) |
  | link | `[text](url)` |
  | bullet / numbered lists, nesting | `- ` / `1. `, indented by level |
  | Shift+Enter line break | trailing backslash |
  | image | `![alt](target)` |
  | underline, colour, highlight, size, font, alignment, indents, spacing, wrap | dropped |

  Emphasis markers are emitted per maximal run so overlapping styles nest
  correctly, and Markdown metacharacters in text are escaped. Image targets
  come from an `ImageExport` policy the caller picks: `DataUri` (self-contained
  file) or `Callback(Fn(&Image, &str alt) -> String)` so an app can write
  `doc_images/1.png` beside the `.md` and return that path. The crate does no
  file I/O itself.
- `Document::from_plain_text` / `to_plain_text` for paste and quick import.
- Not planned: HTML, RTF, DOCX. Markdown import is a possible later addition.

## Backend contract changes (small, all default-implemented)

Everything above uses existing APIs except:

1. **Line metrics on `TextLayout`** — `fn baseline(&self) -> f32` (first-line
   ascent), default `height() * 0.8`. Needed to put mixed font sizes and images
   on one baseline. Implement in cosmic-text (`LayoutRun::line_y`), DirectWrite
   (`DWRITE_LINE_METRICS::baseline`) and headless (`3/4 · height`, matching
   `estimate_metrics`).
2. **Underline/strike metrics** — use `baseline` plus a size-derived offset in
   v1; add `decoration_metrics()` later only if the approximation looks wrong
   on a backend.
3. **IME (later, separate issue)** — `Event::ImePreedit { text, cursor }` /
   `Event::ImeCommit(String)` and `Backend::set_ime_cursor_area(rect)`; winit
   already delivers `Ime` events; Win32 needs IMM32 helpers in a new
   `src/sys/ime.rs`. Until then typing uses `Event::Char`, as every xui text
   widget does today.
4. **Image clipboard (later, separate issue)** — an image format on `Backend`
   so pasted screenshots become images. No HTML clipboard format is planned.

Each lands as its own PR in the backend crates before the widget needs it.

## Portability

- The crate depends only on `xui-core`; it never names a backend.
- Layout tests run against the headless shaper (deterministic geometry), and
  a second test set in `xui-canvas/tests/` runs the same documents through the
  cosmic-text shaper and asserts invariants (no overlaps, text never enters a
  float rect, round-trip hit-testing) rather than exact pixels.
- Light and dark snapshots through `xui_canvas::snapshot::render`; the Win32
  backend is checked with the composited capture only when its rendering is the
  question.

## Proposed file layout (each < 300 lines)

```text
crates/xui-rich-text/
  src/lib.rs
  src/model/{mod, style, paragraph, object, pos, ops, history, summary, tests}.rs
  src/layout/{mod, segment, shape_cache, floats, line, flow, hit, tests}.rs
  src/view/{mod, paint, events, keys, objects, scroll, clipboard}.rs
  src/commands.rs
  src/format/{mod, json, markdown}.rs
  examples/wordpad/{main, app, toolbar}.rs
  tests/{editing, floats, snapshots}.rs
```

## Phases (one PR each)

| # | Deliverable | Done when |
|---|---|---|
| 0 | `TextLayout::baseline` in core + three backends | Unit tests per backend; existing tests green. |
| 1 | `model/`: document, styles, objects, ops, history, style summary | Proptests: `apply(inverse(apply(op)))` is identity; span/object invariants hold over random edit sequences. |
| 2 | `layout/` without floats: segmentation, shape cache, breaking, alignment/justify, indents, spacing, inline images, hit-testing | Headless tests for wrap points, alignment, baseline mixing; hit-test round-trip proptest. |
| 3 | Floats: square left/right, top-and-bottom, cross-paragraph flow, incremental relayout stop rule | Geometry tests: no line intersects a float; text resumes full width below it; relayout of one paragraph touches only what moved. |
| 4 | Widget: paint, caret, selection, navigation, typing, undo, plain clipboard, scrollbar, theme | Offscreen synthetic-input tests (with watchdog); light/dark snapshots; `wordpad` example. |
| 5 | Formatting commands, headings/quote (`BlockKind`), lists, `StyleSummary` toolbar state, rich in-process clipboard | Example toolbar drives every command; tests per command incl. undo. |
| 6 | Image interaction: select, handles, resize, delete, drag-move anchor, wrap commands | Synthetic-drag tests; snapshots with handles. |
| 7 | Persistence: versioned JSON (`serde`), Markdown export, plain text in/out; Save / Export buttons in `wordpad` | JSON round-trip proptest (random documents incl. images); invalid-file tests; Markdown golden tests per mapping row, escaping and nested emphasis. |
| 8+ | Page view / print, IME, bidi, Markdown import, a11y text pattern, find/replace, spell-check decorations | Separate issues. |

Phases 1–3 need no window and are where the hard correctness lives; review
them before the widget is built on top.

## Risks and decisions

- **Per-word shaping** loses kerning and ligatures across spaces and shapes
  complex scripts only within a word. It is what litehtml does here and is fine
  for Latin, CJK and most scripts; revisit with a multi-span shaper API if
  needed.
- **Bidi** is deferred; v1 lays out pieces left to right. The piece model
  allows adding `unicode-bidi` line reordering later without changing the model.
- **Float semantics** follow Word's "square, move with text" subset; tight and
  through wrapping need polygon exclusions and are out of v1.
- **No IME in v1** means CJK and other composed input does not work yet;
  dead keys and AltGr still arrive as `Event::Char`. The caret-rect API from
  `hit.rs` is what `set_ime_cursor_area` will need, so adding it later touches
  only the event mapper.
- **Markdown is lossy** by design; the UI should label it "Export", never
  "Save", so nobody loses formatting by saving to it.
- **Large documents**: shape cache plus incremental and viewport-first
  relayout; measure with a 500-page generated document in a bench before
  phase 4 ships.
- **Separate crate vs `xui-core`**: decided — `xui-rich-text` is a separate,
  opt-in component (see *Packaging*), following the precedent of
  `xui-code-editor` and `xui-litehtml` for large, self-contained widgets with
  their own dependencies. Nothing in `xui-core` changes except the
  `TextLayout::baseline` metric of phase 0.

