# Plan: a Markdown view (`xui-markdown`)

Status: phase 1 (render) merged (va1erian/xui#329); selection, copy and the
highlighter (phases 2 and 3, xui side) implemented · Branch:
`claude/markdown-select-highlight` ·
2026-11-08 · For: the miniscent editor's script guide, LazyOS Docs

## Goal

A read-only widget that shows styled Markdown. It must be **fast** (no
stutter on a 100 KB document, and scrolling must not reshape text), let
people **select and copy text**, and stay **small**. Its first job is to let
the miniscent editor drop `xui-blitz`. Its second is to let LazyOS Docs stop
handing pages to WebKit.

It is portable. It shapes text only through `xui-core`'s `TextShaper` and
paints only through the `Canvas`, so it runs on `xui-canvas`, `xui-d2d` and
the LazyOS backend (which implements `text_shaper`) with no platform code.

## What the two consumers render today

| | miniscent editor: script guide | LazyOS Docs (`xui-app/docs`) |
|---|---|---|
| Source | `assets/guide/scripting.md`, a subset parsed by `script/guide.rs` | any `.md` file, up to `MAX_BYTES` (1 MiB); the largest OS doc is 108 KB |
| Pipeline | subset → `Block`s → HTML (`ui/guide_html.rs`, 338 lines) → `BlitzView` | pulldown-cmark (tables, strike-through) → HTML → WebKit light view in `webd` |
| Blocks | h1, h2, paragraphs, flat lists, indented code, a two-column reference table | h1–h4, paragraphs, nested lists, fenced code, blockquotes, GFM tables, rules |
| Inline | `code`, **bold** | emphasis, strong, strike-through, code, links, images |
| Code | coloured by the editor's own lexer (`ScriptColours`), with an "Insert in script" link under each example | plain monospace |
| Links | only `insert:N`; everything else ignored | `links.rs` sorts each link into anchor, document, file, URL or refused. Anchors do nothing, because a light view cannot scroll |
| Copy | `BlitzViewEvent::CopyRequested(text)` → app | WebKit's own |
| Cost | Blitz brings **209 crates** that `xui-core` does not (Stylo, html5ever, Taffy, Parley, vello_cpu, …) | a separate WebKit process, and an HTML round trip on every theme change |

They need the same set of features: CommonMark with GFM tables and
strike-through, theme colours, link callbacks, selection and copy. miniscent
also needs coloured code and an action on each code block.

## Decision

Make **a new crate, `xui-markdown`**, with three dependencies:

- `xui-core`
- `pulldown-cmark` with `default-features = false` (no `html` and no `simd`
  feature; it brings in only `bitflags`, `memchr` and `unicase`)
- `unicode-linebreak` and `unicode-segmentation`, which `xui-rich-text`
  already uses

Budget: about **2,500 lines** of non-test code, every file under 300 lines.

Rejected:

- **Keep Blitz.** It is a full CSS engine used to draw five kinds of block.
  It costs 209 crates, build time and binary size, and every theme change
  rebuilds a stylesheet.
- **Make `xui-rich-text` read-only and add Markdown import.** That crate is
  15 k lines built for editing (history, floats, pages, editable tables), and
  a viewer would carry all of it. Its model also has no shaded, coloured code
  blocks. (Markdown import into the editor is still worth doing one day. It
  could reuse this crate's parser module.)
- **Write our own Markdown subset parser.** miniscent's guide would be fine
  with one, but LazyOS Docs opens arbitrary files. CommonMark's edge cases
  (lazy continuation, nested emphasis, list tightness) are where hand-written
  parsers break. pulldown-cmark without features is small, and it is already
  in LazyOS's dependency tree.
- **Extend the backends with multi-run layout now.** See phase 5. It is the
  fastest option long term, but it changes the `Backend` contract for three
  backends. The design below does not need it.

## Architecture

```text
&str ──parse──▶ Document ──layout (lazy, per block)──▶ BlockLayout cache ──paint──▶ Canvas
                   │                                          ▲
                   └──────── Selection (byte offset) ─────────┘── hit-test / copy
```

### Model (`model.rs`)

A **flat list of blocks** that all point into **one text buffer**. Nesting is
stored on each block instead of building a tree, so document order is simply
block order, which makes selection and copy easy.

- `Document { text: String, blocks: Vec<Block>, spans: Vec<Span>, links: Vec<Box<str>>, anchors: Vec<(Box<str>, u32)> }`
- `Block { kind, text: Range<u32>, spans: Range<u32>, quote: u8, list: u8 }`
  - `quote` is how many blockquotes the block sits in; `list` is its list
    nesting depth.
- `BlockKind`:
  - `Heading(1..=6)`
  - `Paragraph`
  - `ListItem(Marker)`, where `Marker` is `Bullet`, `Number(u64)` or
    `Task(bool)`
  - `Code { lang }`
  - `TableRow { table: u32, header: bool, cells: Range<u32> }`
  - `Rule`
- `Span { range: Range<u32>, style: Inline, link: Option<u32> }`
  - `Inline` is a bitset: bold, italic, strike, code.
- `anchors` maps GitHub-style heading slugs to block indices, for `#section`
  links.

`Document` is `Send` and has no UI types. An app can parse it on a worker
thread, or build it directly from code.

### Parsing (`parse/`)

pulldown-cmark is configured with `ENABLE_TABLES | ENABLE_STRIKETHROUGH |
ENABLE_TASKLISTS`, and its events are folded into the model.

- **Raw HTML** (block and inline) becomes literal text. This matches the
  current Docs security stance, and nothing is ever interpreted.
- **Images** become their alt text in phase 1, styled weak. Phase 4 adds an
  optional resolver.
- **Soft breaks** become spaces. **Hard breaks** become a line break inside
  the block.
- **Inputs over a size cap** (default 1 MiB, settable) are cut at a block
  boundary, followed by a notice block, the same as Docs' `MAX_BYTES`.

### Style (`style.rs`)

`MarkdownStyle` holds sizes, spacing and indents in `Dip`, plus colour roles.
It is built with `MarkdownStyle::from_theme(&Theme)` and the widget's
`Themed` impl rebuilds it.

- Text uses `text`, links use `accent`, quote bars and rules use `border`,
  code and table-header backgrounds use `surface`, and selection uses
  `selection` / `selection_unfocused`.
- Fonts: body in the default UI family. Code uses a monospace family the app
  can set (`Consolas` on Windows; `Liberation Mono` on LazyOS).
- Code token colours come from `CodePalette`. Its defaults are derived from
  theme tokens, and an app can replace it (miniscent passes its current
  colours).

Nothing in the widget uses a hard-coded colour.

### Layout (`layout/`)

Each block is laid out on its own and the result is cached per
`(width, dpi, style generation)`. Block tops are kept as a prefix sum.

1. **Viewport first.** Blocks not yet laid out get an *estimated* height,
   from their character count and the average glyph width at that size. A
   paint or scroll lays out only the blocks that overlap the viewport (plus
   one screen of margin), then corrects the prefix sum. The rest is laid out
   in slices of a few milliseconds each, so the scrollbar reaches its real
   size within a few frames. A 1 MiB file never blocks the first paint.
2. **Single-style fast path.** If a block's text is all one font and one
   colour, it is shaped with **one** `TextShaper::layout(text, spec, width)`
   call. The backend wraps it, hit-tests it and gives selection rectangles
   natively. In practice that covers most blocks: plain paragraphs and list
   items, headings, each line of an unhighlighted code block, and plain
   table cells.
3. **Mixed-style blocks** use a greedy line breaker:
   - split into pieces at `unicode-linebreak` opportunities and at style
     boundaries;
   - shape each piece through a **shape cache**, keyed by `(font key,
     &str)` and holding an `Arc<dyn TextLayout>` (words repeat a lot, so
     most lookups hit);
   - pack lines and align the pieces on their baselines.

   Known limits: no kerning across a piece boundary, and no bidi
   reordering across pieces. `xui-rich-text` has the same limits.
4. **Tables.** Each column's width is the widest cell's natural width,
   clamped. If the columns do not fit, they shrink in proportion and the
   cells wrap. A row's height is its tallest cell.
5. **Code blocks** never wrap; a long line is clipped. Horizontal scrolling
   inside a code block comes later (phase 5). With a highlighter, each line
   is shaped once whole, for the x position of every token, and each token
   (and each stretch of plain text between them) is shaped again through the
   shape cache, so a highlighted line costs about twice an unhighlighted one
   at layout time, and nothing more to paint. Tabs are shaped as four spaces,
   and positions map back to the document's bytes.

`BlockLayout` stores, for each piece: `Arc<dyn TextLayout>`, origin, byte
range and colour role, plus any decoration rectangles (code background,
quote bar, rule, table grid, bullet).

### Paint (`view/paint.rs`)

The viewport is mapped to a range of blocks by binary search on the prefix
sum. For each block, paint draws its decoration rectangles and then its
cached layouts with `draw_layout`. **Drawing a laid-out block allocates
nothing and shapes nothing.** Before drawing, the painter lays out any block
just coming into view (the first paint, a resize, scrolling onto new text);
only that block costs shaping. A scroll over text already laid out only
repaints. Selection rectangles are worked out per block when the block is
first painted with a selection, and cached until the selection or that
block's layout changes.

### Selection and copy (`layout/hit.rs`, `view/selection.rs`, `view/pointer.rs`, `copy.rs`)

- **Positions.** A position is a byte offset into `Document::text`, and a
  selection is an anchor and a focus.
  - Blocks hold increasing ranges of that text, so one offset orders
    positions across the document.
  - Two neighbouring blocks share their boundary offset: the end of one is
    the start of the next. So hit-testing also returns the block it found,
    and a word or block selection uses that block.
  - Hit-testing finds the block by binary search, then the nearest piece,
    then asks the piece's `TextLayout::hit_test_point`.
- **Mouse.**
  - Dragging selects. Past the top or bottom edge, the view scrolls toward
    the pointer, and goes on scrolling on a 40 ms timer while the pointer is
    held there; on a backend without timers, only while it moves.
  - Shift+click extends the selection.
  - Double-click selects a word (`unicode-segmentation`); triple-click
    selects the block.
  - Dragging from a link selects text; a click on a link without dragging
    follows it.
- **Keys.** The widget can take focus.
  - Ctrl+A selects all and Ctrl+C copies.
  - Arrows, PageUp/PageDown and Home/End scroll.
  - Escape clears the selection.
  - Moving the selection with Shift+arrows is not built.
- **Context menu.** The app's own: `on_context` maps a right-click to a
  message, and the app's menu calls `copy()` and `select_all()`.
- **Copy** goes through `Ui::set_clipboard_text`, as plain text rebuilt from
  the blocks:
  - a blank line between paragraphs;
  - list markers kept (`- `, `1. `, `- [x] `) when the selection reaches
    past a single item, and dropped for text from inside one item;
  - code copied verbatim;
  - table cells separated by tabs and rows by newlines (this pastes into a
    spreadsheet).

  The app does nothing to make copy work. Copying as Markdown (from source
  ranges kept by `into_offset_iter`) is phase 5.

### Links

`on_link: Fn(&str) -> Option<Msg>` receives the destination as CommonMark reads it (escapes and entities
resolved; a relative path stays relative), the
same shape as `FlowText` and `xui-rich-text`'s `on_link`.

- A `#slug` href that matches a heading scrolls there **inside the view**,
  before `on_link` is asked. Docs' anchors start working this way.
- Hovering a link shows the hand cursor. Links are always underlined.
  Reporting the hovered link (`on_hover_link`, for a status bar) is not
  built.
- The view itself never opens anything.

## Public API (sketch)

Phase 1 follows the repo's widget style rather than a separate builder:
`MarkdownView::new(ui, bounds)?` and chained setters, as `RichTextEditor`
does. It has `markdown`/`set_markdown`, `document`/`set_document`,
`style`/`set_style`, `on_link`, `scroll_to_anchor`, `set_scroll`/`scroll`,
`focus`, `set_bounds` and `id`.

Phases 2 and 3 add selection and copy (`selected_text`, `has_selection`,
`copy`, `select_all`, `clear_selection`, `on_context`) and code colours
(`highlighter`, `code_palette`). `on_hover_link` is not built yet.

```rust
use xui_markdown::{MarkdownView, CodeClass, CodeSpan, Highlighter};

let view = MarkdownView::new(ui, bounds)?
    .markdown(&text)                          // or .document(Arc<Document>)
    .on_link(|href| links::classify(href).map(Msg::Open))
    .on_context(|point| Some(Msg::ShowMenu(point)))
    .highlighter(ScriptHighlighter)           // optional: impl Highlighter
    .code_palette(palette);                   // optional: else theme tokens

view.set_markdown(&text);                    // new document, scroll to top
view.scroll_to_anchor("installing");         // false if no such heading
view.selected_text();                        // String
```

```rust
/// Colours code by line, carrying state across lines.
pub trait Highlighter {
    fn line(&self, lang: &str, line: &str, state: u32) -> (Vec<CodeSpan>, u32);
}
pub struct CodeSpan { pub range: Range<usize>, pub class: CodeClass }
pub enum CodeClass { Keyword, Function, String, Comment, Number, Type }
```

- `Highlighter` lives in this crate, so it does not pull in
  `xui-code-editor`. miniscent adapts `ScriptColours` to it in about ten
  lines.
- An action on a code block (miniscent's "Insert in script") is an ordinary
  link the app writes into its Markdown, such as `[Insert in script](insert:3)`,
  so no `code_action` API is needed.
- No numeric ids and no `windows` types appear in the API. Behaviour comes
  from the `Placeable`, `Themed` and focus capability traits.
- The umbrella crate gets a `markdown` feature that re-exports the crate as
  `xui::xui_markdown`.

## Source layout

```text
crates/xui-markdown/
  src/lib.rs              re-exports, crate docs
  src/model.rs            Document, Block, Span, Marker; anchors
  src/parse/              pulldown events → model (builder, events), slugs
  src/copy.rs             a selection as plain text
  src/style.rs            MarkdownStyle, CodePalette, colour roles
  src/highlight.rs        Highlighter, CodeSpan, CodeClass
  src/layout/mod.rs       prefix sum, viewport-first scheduling
  src/layout/estimate.rs  gaps and estimated heights
  src/layout/block.rs     one block: indent, markers, quote bars
  src/layout/flow.rs      inline flow: one-call path, words
  src/layout/line.rs      packing words onto lines
  src/layout/code.rs      code blocks, highlighting, tab mapping
  src/layout/table.rs     column widths, rows
  src/layout/cache.rs     shape cache
  src/layout/hit.rs       hit-testing, selection rectangles
  src/view/mod.rs         MarkdownView and its API
  src/view/shared.rs      shared state, timers, delivering events
  src/view/state.rs       scroll anchor, scrollbar
  src/view/events.rs      wheel, keys, event dispatch
  src/view/pointer.rs     clicks, drags, autoscroll, cursor
  src/view/selection.rs   selection, word and block, rectangle cache
  src/view/paint.rs       painting
  examples/               viewer (a window), snapshot (headless)
  tests/                  parse, view, select, highlight, snapshots, perf
```

## Performance targets

These are checked by tests that use the headless `xui-canvas` shaper, in
release builds:

| Measure | Target | Document |
|---|---|---|
| Parse | < 5 ms | `webd-plan.md` (108 KB) |
| First paint, 1280×800 | < 16 ms | same |
| Scroll frame over text already laid out | no shaping and no allocation; paint time grows with what is visible, not with document size | same |
| Resize | lay out only the visible blocks, the rest in slices | same |
| miniscent guide page switch | < 5 ms (Blitz: one restyle and relayout of an HTML page) | each guide page |

**Size target.** On the miniscent editor, measured with `cargo tree` and the
release exe (its `opt-level = "s"`, fat LTO profile) before and after:
about 205 fewer crates, and a smaller exe. The exact saving gets measured,
not assumed; the d2d plan's numbers suggest Blitz/Stylo is a large part of
the 18.7 MB.

### Measured (phase 1)

Release build, `xui-canvas`'s cosmic-text shaper, 1280 px wide.
`src/layout/perf.rs` (layout alone) and `tests/perf.rs` (parse, and a whole
first paint in a new headless window) produce these numbers. Both are
`#[ignore]`d; run them with `--release -- --ignored --nocapture`, and set
`XUI_MARKDOWN_PERF_FILE` to time another document.

| | generated 100 KB (sample × 60) | `lazyos/docs/webd-plan.md` (108 KB) |
|---|---|---|
| Parse | 1.4 ms (2,730 blocks) | 0.7 ms (303 blocks) |
| First screen layout | 0.45 ms | 9.2 ms |
| Repaint (layout already done) | < 1 µs | < 1 µs |
| A screen further down | 0.27 ms | 1.4 ms |
| Everything else, in 4 ms slices | 23 ms | 29 ms |
| First screen after a resize | 0.28 ms | 0.47 ms |
| First paint in a new window, all included | 41 ms | 53 ms |

Layout figures leave out the font system loading each face (regular, bold,
italic, code). That happens once, the first time a face is used, costs 19 to
26 ms, and is not the view's cost.

A new window's first paint does include it, along with creating the window
and rasterising the frame. An empty document's first paint takes 15 ms, and
the one-screen sample's 41 ms, the same as the 100 KB document. So a
document's length adds nothing to its first paint: the view lays out only the
first screen. Before the review's fix to the estimates (a layout kept the
heights it had guessed with no width until every block was laid out), a
100 KB document's first paint took about 30 ms longer than the sample's.

## Testing

What phase 1 tests, under `crates/xui-markdown`:

- **Parse** (`tests/parse.rs`, `src/parse/slug.rs`, `src/model.rs`).
  - An invariant checker runs on every document: blocks in order, spans
    partitioning their block, table cells covering their row.
  - One test per construct.
  - Raw HTML stays literal text.
  - `parse_limited` cut points, including inside a multi-byte character.
  - Unique heading slugs, and percent-decoded `#` links.
  - A large generated document.
- **Layout** (`src/layout/tests/`, against a deterministic monospace shaper).
  - Single-style blocks take the one-call path; mixed styles are packed word
    by word.
  - A word too wide for any line is cut at a character boundary (not a
    grapheme yet), and no line passes the width.
  - Hard breaks.
  - Link hit areas, underline, strike-through and code shading.
  - List markers by depth, quote bars joined across gaps, heading rules,
    code blocks.
  - Table columns line up and shrink.
  - Laying out only the first screen.
  - Estimates follow the first width a layout is given, and converge to the
    real heights.
  - A new width or DPI drops the layouts. Only a DPI or style change clears
    the shape cache, since shaped words do not depend on the width or theme.
- **View** (`tests/view.rs`: headless, a watchdog on every test).
  - A link click versus a press that moves off the link.
  - `#anchor` scrolling.
  - Wheel, keys and the scrollbar track.
  - The top padding shown at first.
  - End landing on, and staying on, the real end, including when the last
    blocks are taller than estimated.
- **Snapshots** (`tests/snapshots.rs`, `examples/snapshot.rs`). Light and
  dark renders checked on pixels: page and text colours, link colour, code
  surface and borders.
- **Not covered.**
  - The background layout timer starting and stopping: the headless backend
    has no timers.
  - `scripts/snapshots.ps1` and `examples/demo/` cover only the `xui`
    crate's examples, as for `xui-rich-text`.
  - Selection, copy and highlighting have their own tests:
    `src/layout/tests/hit.rs`, `src/layout/tests/code.rs`, `src/copy.rs`,
    `tests/select.rs` and `tests/highlight.rs`.
  - The miniscent guide test moves with the migration.

## Migration

**miniscent editor**

- `guide.rs` keeps splitting `scripting.md` into pages at `# `, and hands
  each page's Markdown over as is.
- The reference page is written as a GFM table, with `|` escaped.
- `guide_html.rs` (338 lines) is replaced by a small adapter: a
  `Highlighter` over `ScriptColours`, the palette, and Markdown for each page
  whose examples end in an `[Insert in script](insert:N)` link, read back by
  `example_of`.
- `script_widgets.rs` swaps `BlitzView` for `MarkdownView`, and drops
  `GuideFrame` and `CopyRequested`, because the view copies by itself.
- `xui-blitz` leaves `apps/editor/Cargo.toml`.

**LazyOS Docs**

- `lib.rs` loses the HTML writer and its CSS/DARK_CSS strings; the parsing
  moves into this crate.
- `links.rs` stays as it is and becomes the `on_link` closure.
  `Link::Anchor` is now handled by the view.
- The Docs window shows a `MarkdownView` instead of a `LightView`, so Docs no
  longer needs `webd`.
- Images: alt text until phase 4, then `.image_resolver(|src| …)` reads them
  through the existing `mimed` / file path rules.

## Phases

1. **Render.** Model, parser, style, layout (fast path, breaker, tables,
   code), paint, scrolling, links and anchors, theming, snapshots.
2. **Select and copy.** Mouse, keys, plain-text copy, and an `on_context`
   hook for the app's own context menu (as `ListView` has), which calls the
   view's `copy()` and `select_all()`. Done. Moving the selection with
   Shift+arrows is not done.
3. **miniscent.** The xui side is done: the `Highlighter` trait and a
   `CodePalette`. Still to do: the guide migration, removing `xui-blitz`, and
   recording the size saving in this plan. `code_action` was dropped: an
   example's "Insert in script" is an ordinary Markdown link
   (`[Insert in script](insert:N)`), which `on_link` hands to the guide's
   existing `example_of`.
4. **LazyOS Docs.** Migration, an image resolver (`Fn(&str) -> Option<Image>`,
   decoded by the app), and find-in-page (`find(&str)` → highlighted matches).
5. **Later.**
   - Copy as Markdown.
   - Horizontal scrolling in code blocks.
   - Layout on a worker thread (`TextShaper` is `Send + Sync`).
   - `TextShaper::layout_runs(text, &[(Range, FontSpec)], width)`, with a
     default implementation in `xui-core` that uses this crate's breaker,
     overridden by DirectWrite and cosmic-text so a mixed-style paragraph
     becomes one native layout.
   - A UI Automation text pattern.

## Open questions

- Should copy put Markdown on the clipboard as a second format when the
  backend supports more than one format? Today `set_clipboard_text` is
  text only.
- Are images needed in miniscent's guide? (It has none today.) If not, they
  stay in phase 4.
- Should the shape cache be shared between views in a window (a guide and a
  tooltip, say), or stay one per view? Phase 1 uses one per view, capped by
  entry count.
