#![forbid(unsafe_code)]

//! Code blocks: lines on a shaded box, coloured by the app's highlighter when
//! it gave one. A line wider than the box wraps onto more rows
//! ([`MarkdownStyle::wrap_code`](crate::MarkdownStyle::wrap_code)), or is
//! clipped when wrapping is off.
//!
//! Tabs are shaped as four spaces, whatever the column, so a piece's shaped
//! text is its [`Piece::range`] with each tab widened. A hit at byte `e` of a
//! piece's layout maps back to the document with
//! `piece.range.start + original_offset(&doc.text[piece.range], e)`; a hit
//! inside a tab's four spaces lands on the nearer side of the tab.

use std::borrow::Cow;
use std::ops::Range;

use super::cache::Shaped;
use super::geom::{BlockLayout, Ctx, Decor, FRect, Piece};
use super::rows;
use crate::highlight::CodeSpan;
use crate::model::{Block, Inline};
use crate::style::{FontKey, Role};

/// What a tab is shaped as.
const TAB: &str = "    ";

/// `text` as it is shaped: tabs widened.
fn expand(text: &str) -> Cow<'_, str> {
    if text.contains('\t') {
        Cow::Owned(text.replace('\t', TAB))
    } else {
        Cow::Borrowed(text)
    }
}

/// The byte of `expand(line)` that byte `at` of `line` becomes.
pub(crate) fn expanded_offset(line: &str, at: usize) -> usize {
    let tabs = line.as_bytes()[..at]
        .iter()
        .filter(|&&b| b == b'\t')
        .count();
    at + tabs * (TAB.len() - 1)
}

/// The byte of `text` that byte `shaped` of `expand(text)` stands for. Inside
/// a tab's spaces, the nearer edge of the tab.
pub(crate) fn original_offset(text: &str, shaped: usize) -> usize {
    let mut start = 0;
    for (at, c) in text.char_indices() {
        let width = if c == '\t' { TAB.len() } else { c.len_utf8() };
        if shaped < start + width {
            let past_middle = c == '\t' && shaped - start >= TAB.len() / 2;
            return at + usize::from(past_middle);
        }
        start += width;
    }
    text.len()
}

/// Lays out a code block; returns its height.
pub(super) fn layout(
    ctx: &mut Ctx,
    block: &Block,
    left: f32,
    width: f32,
    out: &mut BlockLayout,
) -> f32 {
    let doc = ctx.doc;
    let highlighter = ctx.highlighter;
    let lang = match &block.kind {
        crate::model::BlockKind::Code { lang } => lang.as_ref(),
        _ => "",
    };
    let inset = ctx.px(ctx.style.inset);
    let font = ctx.style.font(Inline::NONE, None, true);
    let line_height = ctx
        .cache
        .metrics(ctx.shaper, ctx.style, font, ctx.dpi)
        .height;
    let plain = if block.quote > 0 {
        Role::Weak
    } else {
        Role::Text
    };
    let avail = if ctx.style.wrap_code {
        width - 2.0 * inset
    } else {
        f32::INFINITY
    };
    let pen_x = left + inset;
    let mut y = inset;
    let mut at = block.text.start;
    let mut state = 0;
    for line in doc.block_text(block).split('\n') {
        let spans = match highlighter {
            Some(h) => {
                let (spans, next) = h.line(lang, line, state);
                state = next;
                Some(spans)
            }
            None => None,
        };
        if line.trim_end().is_empty() {
            y += line_height;
            at += line.len() as u32 + 1;
            continue;
        }
        let spec = ctx.style.spec(font);
        let whole = ctx
            .shaper
            .layout(&expand(line), &spec, f32::INFINITY, ctx.dpi);
        let rows = rows::rows(&*whole, line, avail);
        let mut whole = (rows.len() == 1).then_some(whole);
        for range in rows {
            let text = &line[range.clone()];
            // A line on one row reuses its shape; a wrapped row is shaped alone.
            let shaped = whole.take().unwrap_or_else(|| {
                ctx.shaper
                    .layout(&expand(text), &spec, f32::INFINITY, ctx.dpi)
            });
            let row = Row {
                text,
                at: at + range.start as u32,
                pen: (pen_x, y),
            };
            let spans = spans.as_ref().map(|s| clip_spans(s, line, &range));
            row.lay(ctx, shaped, spans.as_deref(), plain, out);
            y += line_height;
        }
        at += line.len() as u32 + 1;
    }
    let height = y + inset;
    out.decor.insert(
        0,
        Decor::Fill(FRect::new(left, 0.0, width, height), Role::Surface),
    );
    out.clip = Some(FRect::new(
        left + inset,
        0.0,
        (width - 2.0 * inset).max(0.0),
        height,
    ));
    height
}

/// One line of a block being cut into pieces.
struct Line<'a> {
    text: &'a str,
    /// Its first byte in [`Document::text`](crate::model::Document::text).
    at: u32,
    /// The whole line shaped, for the position of its parts.
    whole: Box<dyn xui_core::backend::TextLayout>,
    font: FontKey,
    plain: Role,
    pen: (f32, f32),
}

impl Line<'_> {
    /// A piece showing `range` of the line.
    fn piece(&self, ctx: &mut Ctx, range: Range<usize>, role: Role, out: &mut BlockLayout) {
        // The line's own geometry places each segment: a segment's width
        // cannot be trusted, since backends may leave trailing whitespace
        // out of it.
        let from = expanded_offset(self.text, range.start);
        let to = expanded_offset(self.text, range.end);
        let x = self
            .whole
            .selection_rects(from, to)
            .iter()
            .map(|r| r.left as f32)
            .reduce(f32::min)
            .unwrap_or(0.0);
        let text = expand(&self.text[range.clone()]);
        let shaped = ctx
            .cache
            .word(ctx.shaper, ctx.style, self.font, &text, ctx.dpi);
        out.pieces.push(Piece {
            layout: shaped.layout,
            x: self.pen.0 + x,
            y: self.pen.1,
            range: self.at + range.start as u32..self.at + range.end as u32,
            tabs: self.text[range.clone()].contains('\t'),
            role,
        });
    }

    /// The plain text of `range`, without its surrounding whitespace.
    fn plain(&self, ctx: &mut Ctx, range: Range<usize>, out: &mut BlockLayout) {
        let text = &self.text[range.clone()];
        let trimmed = text.trim_start();
        let start = range.start + (text.len() - trimmed.len());
        let trimmed = trimmed.trim_end();
        if !trimmed.is_empty() {
            self.piece(ctx, start..start + trimmed.len(), self.plain, out);
        }
    }
}

/// One row of a code line: its text, its first byte in the document and
/// where it is drawn.
struct Row<'a> {
    text: &'a str,
    at: u32,
    pen: (f32, f32),
}

impl Row<'_> {
    /// Lays the row out from `shaped` (its text, unwrapped): one piece, or
    /// with `spans` (relative to the row) each valid span and the trimmed
    /// plain text between them.
    fn lay(
        &self,
        ctx: &mut Ctx,
        shaped: Box<dyn xui_core::backend::TextLayout>,
        spans: Option<&[CodeSpan]>,
        plain: Role,
        out: &mut BlockLayout,
    ) {
        if self.text.trim_end().is_empty() {
            return;
        }
        match spans {
            Some(spans) => highlighted(ctx, self, shaped, spans, plain, out),
            None => out.pieces.push(Piece {
                layout: Shaped::new(shaped).layout,
                x: self.pen.0,
                y: self.pen.1,
                range: self.at..self.at + self.text.len() as u32,
                tabs: self.text.contains('\t'),
                role: plain,
            }),
        }
    }
}

/// The parts of `spans` (relative to `line`) inside `row`, relative to it. A
/// span that is not valid for the whole line is dropped first, as it would be
/// unwrapped, rather than made valid by the cut.
fn clip_spans(spans: &[CodeSpan], line: &str, row: &Range<usize>) -> Vec<CodeSpan> {
    spans
        .iter()
        .filter(|span| {
            let r = &span.range;
            r.start < r.end
                && r.end <= line.len()
                && line.is_char_boundary(r.start)
                && line.is_char_boundary(r.end)
        })
        .filter_map(|span| {
            let start = span.range.start.max(row.start);
            let end = span.range.end.min(row.end);
            (start < end).then(|| CodeSpan {
                range: start - row.start..end - row.start,
                class: span.class,
            })
        })
        .collect()
}

/// The pieces of one row: each valid span, and the trimmed plain text
/// between them. `whole` is the row shaped, for the position of its parts.
fn highlighted(
    ctx: &mut Ctx,
    row: &Row,
    whole: Box<dyn xui_core::backend::TextLayout>,
    spans: &[CodeSpan],
    plain: Role,
    out: &mut BlockLayout,
) {
    let font = ctx.style.font(Inline::NONE, None, true);
    let text = row.text;
    let line = Line {
        text,
        at: row.at,
        whole,
        font,
        plain,
        pen: row.pen,
    };
    let mut pos = 0;
    for span in spans {
        let r = &span.range;
        let valid = r.start >= pos
            && r.start < r.end
            && r.end <= text.len()
            && text.is_char_boundary(r.start)
            && text.is_char_boundary(r.end);
        if !valid {
            continue;
        }
        line.plain(ctx, pos..r.start, out);
        line.piece(ctx, r.clone(), Role::Code(span.class), out);
        pos = r.end;
    }
    line.plain(ctx, pos..text.len(), out);
}
