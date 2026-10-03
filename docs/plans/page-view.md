# Plan: page view for `xui-rich-text`

Status: implemented · Requested for LazyWriter (va1erian/lazyos)

## Goal

Show a document as the pages it would print on: white sheets of a chosen
paper size on a desk, the text inside the margins, lines and pictures that do
not fit moving to the next page. The continuous column stays as draft view.
Tables, which come next, must paginate through the same mechanism.

## Model

* **`PageSetup`** on `Document` (`doc.page()`): paper width and height and
  four margins, in dip. `PageSetup::a4()` (the default, 25 mm margins) and
  `letter()` (1 in margins), `with_margins`, `rotated`, `oriented`.
  `check()` refuses non-finite lengths, paper over about 2.6 m a side, and
  margins that leave less than half an inch of text.
* **`EditOp::SetPage(PageSetup)`**, its own inverse, so
  `Command::SetPageSetup` is one undo step. A refused page is
  `EditError::BadPage`.
* **`ParaStyle::page_break_before`** (and `ParaStylePatch::page_break_before`).
  `Command::InsertPageBreak` (Ctrl+Enter) splits the paragraph and sets it on
  the second half. A paragraph split off or typed after one does not inherit
  it (`SplitParagraph` and multi-line `InsertText` give their later
  paragraphs the style without it), so Enter does not start another page.
  Backspace at the start of such a paragraph clears it first, the way it
  leaves a list.
* **JSON**: an optional `"page": {"width", "height", "margins": [l, t, r, b]}`
  and an optional `page_break_before` per paragraph style (written only when
  set). The format stays version 1; files without them load on A4.
  Markdown export ignores both.

## Layout

* `Layout::set_pages(Option<Pages>)`. `Pages { content, pitch }` is in device
  pixels: page `k`'s text area runs from `k * pitch` to `k * pitch + content`,
  and the strip below it up to the next page's text (bottom margin, desk gap,
  top margin) is that page's *gap*. The flow stays one vertical space, so hit
  testing, caret geometry, selection and painting work unchanged.
* **Page gaps are virtual full-width exclusions** in `FloatCtx`: `interval`,
  `next_bottom`, `place_square` and `place_band` see the gap a span would run
  into. The existing breaker therefore drops a line that would cross a gap to
  the next page top, and places floats below gaps, with no second breaking
  algorithm. `finish` also pushes a line that only crosses at its final
  height (an empty paragraph's strut).
* **Termination**: a span starting at a page's top never runs into a gap, so a
  line or picture taller than a page overflows instead of moving forever.
  Floating pictures are clamped to the content height, keeping their aspect.
* **Page break before**: the paragraph's first line starts at the next page's
  top unless the paragraph already starts at one (its space before is dropped
  there, as in Word).
* **Incremental relayout**: a clean paragraph is reused as before when its
  entering floats and list number match, and on pages only when it has not
  moved, or when it starts no page and lies wholly inside one page's text
  area both where it was laid out (`laid_y`) and where it now goes. So an edit
  relays out the paragraphs that straddle a page edge below it, not the whole
  document. The viewport-first pass treats a visible paragraph that no longer
  fits where it was laid out like a dirty one.
* Queries: `page_count()`, `page_at(y)`, `pages()`.

### Tables

A table will be one more block in the flow, asking the same `FloatCtx` where
the next page gap is (`page_push`). A row that does not fit moves to the next
page; splitting a tall row across pages is the same question asked per line
of each cell.

## View

* `ViewMode::{Draft, Page}`; `RichTextEditor::view_mode`, `set_view_mode`,
  `current_view_mode`. Draft is the default, so existing apps are unchanged.
* The fixed text pad becomes an *origin* (`State::origin`): in page view the
  sheets are centred and the origin is the first sheet's text corner. All
  conversions (`to_view`, `to_layout`, `shift`, `ensure_visible`) go through
  it.
* **Zoom is a layout DPI**: when the view is narrower than a sheet plus the
  desk, the layout runs at `dpi * zoom` (at least 25%), so text is shaped at
  the size it is shown. Image resizing converts pointer deltas at that DPI.
* **Paint** (`view/sheets.rs`): the desk (the window background, a step
  darker in a light theme), each visible sheet with a soft shadow and a
  hairline edge, then the text in `Theme::light()` colours, because paper is
  printed dark on white whatever the window theme. Quote rules are drawn line
  by line in page view so they stop at a page edge. In draft view a page-break
  paragraph gets a dashed rule above it.
* `RichTextEditor::page_info()` returns the caret's page (from 0) and the page
  count, for a status bar.

## Tests

* `layout/pages.rs`: page index, gaps, break positions, counts.
* `layout/tests/pages.rs`: lines never cross a page edge; a long paragraph
  continues at the next page top; empty paragraphs never sit in a gap; page
  break before, and no extra page at a page top; square and Top-and-bottom
  floats move to the next page; a float taller than a page is shrunk; a line
  taller than a page terminates; edits above a page edge match a full layout;
  paragraphs inside a page are reused; viewport-first layout converges; turning
  pages off restores the continuous flow.
* `tests/edit_pages.rs`: Ctrl+Enter, Enter and paste not inheriting the break,
  Backspace, page setup undo, a refused page, JSON round trip, old files.
* `view/tests.rs`: centred full-size sheets, fit to a narrow view, page count
  and caret page, hit testing on a later page, back to draft.
* `tests/snapshots.rs`: white sheets with dark text in light and dark themes,
  and a sheet shrunk into a narrow window.

## Later

Headers and footers (they sit in the gap's margins), page numbers on the page,
widow and orphan control and keep-with-next (checks before a paragraph is
placed or reused), columns, explicit zoom levels, and printing.
