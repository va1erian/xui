# xui-rich-text

An editable rich-text widget for [xui](../../README.md): styled runs (bold,
italic, underline, strike-through, colour, highlight, size, family, links,
super/subscript), paragraph formatting (alignment, indents, spacing, headings,
quotes, bullet and numbered lists) and images, inline or floating with text
flowing around them.

It is **portable**: text is shaped only through `xui-core`'s `TextShaper` and
painted only through the `Canvas`, so the same widget runs on the Win32 and the
canvas backends, in light and dark mode.

## Using it

The crate is opt-in; nothing pulls it in by default. Either depend on it
directly:

```toml
xui-rich-text = { path = "crates/xui-rich-text" }
```

or enable the umbrella crate's `rich-text` feature and use
`xui::xui_rich_text`.

```rust,ignore
use xui_core::{Rect, app::Ui};
use xui_rich_text::{RichTextEditor, model::Document};

fn build(ui: &Ui<Msg>) -> xui_core::Result<RichTextEditor<Msg>> {
    let editor = RichTextEditor::new(ui, Rect::new(0, 0, 640, 480))?
        .document(Document::from_plain_text("Hello,\nrich text."));
    editor.set_scroll(0.0);
    Ok(editor)
}
```

The widget paints, scrolls and lays out a document, and maps keyboard and
mouse input (typing, selection, clipboard, image resize and move) to commands
that the editing controller below runs. Toolbars and menus send the same
commands through `RichTextEditor::exec`; `on_change`, `on_selection` and
`on_link` report back to the app.

## Layers

Each layer is testable without a window.

* **`model`**: `Document` (paragraphs of interned styled spans, anchored
  images), invertible `EditOp`s applied with `Document::apply`, `History`
  (transactions, undo/redo, typing and backspace coalescing), `Selection`,
  grapheme and word movement, `Fragment` for copy and paste, and
  `Document::style_summary` for toolbar state.
* **`layout`**: the flow engine that breaks paragraphs into lines around
  floating images and answers hit-testing and caret geometry.
* **`format`**: the JSON save format and Markdown export (below).
* **`edit`**: the UI-free controller. `EditorState` owns the document, history
  and selection; `EditorState::exec(Command, &dyn LineNav, now, &dyn Clipboard)`
  runs one undoable `Command` (typing, deleting, motions, formatting, lists,
  images, resize/move/wrap, cut/copy/paste, undo/redo) and returns an `Effect`
  naming the paragraphs to lay out again. `edit::handles` holds the image resize
  handle geometry.

## Saving and exporting

* **Native JSON** (`serde` feature, on by default): `format::to_json(&doc)` and
  `format::from_json(&str)`. Versioned and lossless; images are embedded as
  base64 PNG. Loading validates every invariant and returns a typed
  `FormatError` instead of panicking on a bad file.
* **Markdown** (always available): `format::to_markdown(&doc, &ImageExport)`
  writes lossy GitHub Flavored Markdown (headings, quotes, bold, italic,
  strike-through, links, nested lists, line breaks, images). Pick
  `ImageExport::DataUri` for a self-contained file, or
  `ImageExport::Callback` to choose each image's target yourself (for example
  write `doc_images/1.png` beside the `.md`). The crate does no file I/O.

## What v1 does not do

* No page view or printing: the document is one continuous column.
* No IME composition.
* No bidirectional or vertical text.
* No HTML, RTF or DOCX import or export; no Markdown import.

## Snapshots

Render the sample document headlessly, light and dark, without opening a window:

```text
cargo run -p xui-rich-text --example snapshot
```

The PNGs land in `target/snapshots/rich-text-{light,dark}.png`.

See [`docs/plans/rich-text-editor.md`](../../docs/plans/rich-text-editor.md) for
the design.
