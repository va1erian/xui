#![forbid(unsafe_code)]

//! Code blocks: unwrapped lines on a shaded box, coloured by the app's
//! highlighter when it gave one.
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
    let mut y = inset;
    let mut at = block.text.start;
    let mut state = 0;
    for line in doc.block_text(block).split('\n') {
        let spans = match highlighter {
            Some(h) => {
                let (spans, next) = h.line(lang, line, state);
                state = next;
                spans
            }
            None => Vec::new(),
        };
        if !line.trim_end().is_empty() {
            let pen = (left + inset, y);
            if highlighter.is_some() {
                highlighted(ctx, line, at, &spans, plain, pen, out);
            } else {
                let shaped = Shaped::new(ctx.shaper.layout(
                    &expand(line),
                    &ctx.style.spec(font),
                    f32::INFINITY,
                    ctx.dpi,
                ));
                out.pieces.push(Piece {
                    layout: shaped.layout,
                    x: pen.0,
                    y,
                    range: at..at + line.len() as u32,
                    tabs: line.contains('\t'),
                    role: plain,
                });
            }
        }
        y += line_height;
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

/// The pieces of one line starting at byte `at` of the document: each valid
/// span, and the trimmed plain text between them.
fn highlighted(
    ctx: &mut Ctx,
    text: &str,
    at: u32,
    spans: &[CodeSpan],
    plain: Role,
    pen: (f32, f32),
    out: &mut BlockLayout,
) {
    let font = ctx.style.font(Inline::NONE, None, true);
    let whole = ctx
        .shaper
        .layout(&expand(text), &ctx.style.spec(font), f32::INFINITY, ctx.dpi);
    let line = Line {
        text,
        at,
        whole,
        font,
        plain,
        pen,
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
