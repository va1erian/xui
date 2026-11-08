# xui-markdown

A fast, small, read-only Markdown view for [xui](../../README.md).

It is **portable**: text is shaped only through `xui-core`'s `TextShaper` and
painted only through the `Canvas`, so the same widget runs on every backend, in
light and dark mode.

## Using it

The crate is opt-in; nothing pulls it in by default. Either depend on it
directly:

```toml
xui-markdown = { path = "crates/xui-markdown" }
```

or enable the umbrella crate's `markdown` feature and use `xui::xui_markdown`.

```rust,ignore
use xui_core::{Rect, app::Ui};
use xui_markdown::MarkdownView;

fn build(ui: &Ui<Msg>) -> xui_core::Result<MarkdownView<Msg>> {
    MarkdownView::new(ui, Rect::new(0, 0, 640, 480))?
        .markdown("# Hello\n\nSome *styled* text and a [link](next.md).")
        .on_link(|href| Some(Msg::Open(href.to_owned())))
}
```

`set_markdown` swaps the text later; `document` / `set_document` take a
`Document` parsed elsewhere (on a worker thread, say). `scroll_to_anchor`
and `set_scroll` move the view. A `#slug` link to one of the document's
headings scrolls there and is not reported to `on_link`.

## Selecting and copying

Text is selected the usual ways:
- **Mouse.** Drag to select; past the view's top or bottom edge it scrolls
  toward the pointer, and keeps scrolling while the pointer is held there.
  Shift+click extends the selection, a double-click selects a word and a
  triple-click a block.
- **Links.** A press on a link follows it when released there; moving the
  pointer first selects the link's text instead.
- **Keys.** Ctrl+A selects everything, Ctrl+C (or Ctrl+Insert) copies, and
  Escape clears.

Copied text is plain text that pastes well anywhere:
- a blank line between paragraphs;
- list items one per line with their markers (`- `, `1. `, `- [x] `), when
  the selection spans more than one item (text from inside one item copies
  without its marker);
- code exactly as written, tabs included;
- table rows as tab-separated cells, which a spreadsheet splits into columns.

For a context menu, `on_context` maps a right-click (with its point) to a
message, and the app's menu calls `copy()` and `select_all()`.
`selected_text()`, `has_selection()` and `clear_selection()` complete the
API.

## Highlighting code

The crate has no lexer of its own: an app adapts the one it has by
implementing `Highlighter`, which cuts one line of a code block into
`CodeSpan`s (`Keyword`, `Function`, `String`, `Comment`, `Number`, `Type`),
carrying a state from line to line for multi-line comments or strings.

```rust,ignore
struct Script;

impl Highlighter for Script {
    fn line(&self, lang: &str, line: &str, state: u32) -> (Vec<CodeSpan>, u32) {
        // `lang` is the fence's info string ("rust" in ```rust).
        my_lexer::lex(line, state)
    }
}

let view = MarkdownView::new(ui, bounds)?
    .highlighter(Script)
    .code_palette(my_palette); // optional
```

Without a `CodePalette`, colours come from the theme's tokens
(`CodePalette::from_theme`): keywords, functions and types in the accent
colour, strings in the warning colour, numbers in the danger colour, and
comments in secondary text. A palette has a light and a dark set, and the view
picks the one for the live theme, so switching theme only repaints.

## What it renders

CommonMark plus the GFM extensions tables, strike-through and task lists:
headings, paragraphs, emphasis, strong text, inline code, links, block quotes,
bullet and numbered lists, code blocks and tables (with column alignment).
Raw HTML is shown as literal text; an image is shown as its alt text for now.

## Styling

`MarkdownStyle` holds the fonts, sizes and spacing as design values (`Dip`):
body and code families and sizes, heading scale, padding, block gap, list and
quote indents, and the inset of code blocks and table cells. Pass it with
`.style(..)` or `set_style`. Colours are not part of it: text is painted in
semantic roles resolved against the live theme, so a theme change repaints
without laying anything out again.

## How it stays fast

* **Viewport first.** The blocks on screen are laid out first; the rest is laid
  out in slices on a timer, so a long document opens at once. (A backend
  without timers, like the offscreen one, lays out only what it paints.)
* **Single-call fast path.** Text in one style is shaped in one call and wrapped
  by the backend. Only mixed-style text is cut into words at Unicode line-break
  opportunities and packed onto lines.
* **Shape cache.** Words are shaped once per font and reused; lookups borrow the
  word, so a hit allocates nothing.
* **Nothing shaped or allocated to redraw.** Drawing a laid-out block uses
  only its cached layouts; a paint shapes text only for blocks just coming
  into view (the first paint, a resize, scrolling onto new text).

## Limits

* Selection is by pointer and Ctrl+A only: Shift+arrows don't move it yet.
* In a highlighted code block, the selection band skips a line's leading
  indentation and blank lines (the copied text keeps them).
* A triple-click is a press within 500 ms of a double-click; the system's
  double-click time is not read.
* No images (alt text only).
* No bidirectional reordering across styled pieces.
* Code blocks clip long lines rather than wrapping them.
* A view with no room for text (narrower than its padding and scrollbar,
  about 51 px at 96 DPI) draws none; one squeezed that far keeps its last
  layout.
* A table measures all its rows once any of them shows, to size its columns.
* A list item that starts with a code block or a quote shows its marker on a
  line of its own, and an image with empty alt text shows nothing.

See [`docs/plans/markdown-view.md`](../../docs/plans/markdown-view.md) for the
design.

## Examples

```text
cargo run -p xui-markdown --example viewer [-- file.md]
cargo run -p xui-markdown --example snapshot
```

`viewer` opens a window on the given file, or on the sample document, and
prints the destination of every link clicked. Set `XUI_DEMO_AUTOCLOSE_MS` to
make it quit itself. `snapshot` renders the sample headlessly, light and dark,
to `target/snapshots/markdown-{light,dark}.png`.
