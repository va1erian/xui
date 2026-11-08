#![forbid(unsafe_code)]

//! Inline flow: a run of styled spans wrapped into lines.
//!
//! Text in one style is shaped in a single call and wrapped by the backend
//! (the fast path, which most paragraphs, headings and cells take). Mixed
//! text is cut into words at Unicode line-break opportunities and at style
//! changes, each word shaped through the [`ShapeCache`](super::cache), and
//! the words packed greedily onto lines that share a baseline.

use unicode_linebreak::{BreakOpportunity, linebreaks};

use super::cache::Shaped;
use super::geom::{BlockLayout, Ctx, Decor, FRect, Piece};
use super::line::Lines;
use crate::model::{Align, Inline, Span};
use crate::style::{FontKey, Role};

/// Slack for float rounding when testing whether a word fits.
pub(super) const EPS: f32 = 0.5;

/// What to flow and how.
pub(crate) struct Flow<'s> {
    /// The spans, which partition the text to flow.
    pub spans: &'s [Span],
    /// The heading level, for its font.
    pub heading: Option<u8>,
    /// Formatting added to every span (a header cell's bold).
    pub extra: Inline,
    /// Whether the text is in a block quote (drawn weak).
    pub quoted: bool,
    pub align: Align,
}

/// The size of flowed text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Flowed {
    pub height: f32,
    /// The widest line.
    pub width: f32,
    /// The first line's baseline, from the top.
    pub baseline: f32,
}

impl Flow<'_> {
    pub(super) fn font(&self, ctx: &Ctx, span: &Span) -> FontKey {
        ctx.style
            .font(span.style.with(self.extra), self.heading, false)
    }

    pub(super) fn role(&self, span: &Span) -> Role {
        Role::of(span.style, span.link.is_some(), self.quoted)
    }
}

/// Flows `flow` into `out` at `(x, y)`, wrapping at `width` (infinite for
/// none).
pub(crate) fn flow(
    ctx: &mut Ctx,
    flow: &Flow,
    (x, y): (f32, f32),
    width: f32,
    out: &mut BlockLayout,
) -> Flowed {
    let (Some(first), Some(last)) = (flow.spans.first(), flow.spans.last()) else {
        let font = ctx.style.font(flow.extra, flow.heading, false);
        let metrics = ctx.cache.metrics(ctx.shaper, ctx.style, font, ctx.dpi);
        return Flowed {
            height: metrics.height,
            width: 0.0,
            baseline: metrics.baseline,
        };
    };
    let (start, end) = (first.range.start, last.range.end);
    let text = &ctx.doc.text[start as usize..end as usize];
    let uniform = flow
        .spans
        .iter()
        .all(|s| s.style == first.style && s.link == first.link);
    if uniform && !text.contains('\n') {
        return single(ctx, flow, (x, y), width, out);
    }
    let mut lines = Lines::new(x, y, width);
    let mut span = 0;
    let mut from = 0;
    for (to, kind) in linebreaks(text) {
        let atoms = atoms(ctx, flow, start, from, to, &mut span);
        lines.place(ctx, flow, atoms, out);
        if kind == BreakOpportunity::Mandatory && to < text.len() {
            lines.finish(ctx, flow, out);
        }
        from = to;
    }
    lines.finish(ctx, flow, out);
    lines.flowed()
}

/// The fast path: one shaped layout, wrapped by the backend.
fn single(
    ctx: &mut Ctx,
    flow: &Flow,
    (x, y): (f32, f32),
    width: f32,
    out: &mut BlockLayout,
) -> Flowed {
    let span = &flow.spans[0];
    let (start, end) = (span.range.start, flow.spans[flow.spans.len() - 1].range.end);
    let text = &ctx.doc.text[start as usize..end as usize];
    let spec = ctx.style.spec(flow.font(ctx, span));
    let shaped = Shaped::new(ctx.shaper.layout(text, &spec, width, ctx.dpi));
    let rects = shaped.layout.selection_rects(0, text.len());
    let dx = if rects.len() <= 1 && width.is_finite() {
        offset(flow.align, width - shaped.width)
    } else {
        0.0
    };
    for r in &rects {
        let rect = FRect::new(
            r.left as f32 + x + dx,
            r.top as f32 + y,
            r.width() as f32,
            r.height() as f32,
        );
        decorate(ctx, out, rect, shaped.baseline, span);
    }
    out.pieces.push(Piece {
        layout: shaped.layout,
        x: x + dx,
        y,
        range: start..end,
        tabs: false,
        role: flow.role(span),
    });
    Flowed {
        height: shaped.height,
        width: shaped.width,
        baseline: shaped.baseline,
    }
}

/// Where a line `slack` narrower than its box starts.
pub(super) fn offset(align: Align, slack: f32) -> f32 {
    match align {
        Align::Center => (slack / 2.0).max(0.0),
        Align::Right => slack.max(0.0),
        Align::None | Align::Left => 0.0,
    }
}

/// Underlines a link, strikes struck text and shades inline code in `rect`.
pub(super) fn decorate(ctx: &Ctx, out: &mut BlockLayout, rect: FRect, baseline: f32, span: &Span) {
    let thin = ctx.scale().round().max(1.0);
    if span.style.contains(Inline::CODE) {
        let pad = 2.0 * ctx.scale();
        out.decor.push(Decor::Fill(
            FRect::new(rect.x - pad, rect.y, rect.w + 2.0 * pad, rect.h),
            Role::Surface,
        ));
    }
    if let Some(link) = span.link {
        let under = FRect::new(rect.x, rect.y + baseline + thin, rect.w, thin);
        out.decor.push(Decor::Fill(under, Role::Link));
        out.links.push((rect, link));
    }
    if span.style.contains(Inline::STRIKE) {
        let mid = FRect::new(rect.x, rect.y + baseline * 0.68, rect.w, thin);
        out.decor.push(Decor::Fill(mid, Role::Text));
    }
}

/// A piece of a segment in one span: the word (trailing space trimmed) and
/// the space after it.
pub(super) struct Atom {
    pub range: (u32, u32),
    pub span: usize,
    pub shaped: Option<Shaped>,
    pub tail: f32,
}

/// The atoms of the segment `from..to` (relative to `start`), advancing
/// `span` to the span holding its end.
fn atoms(
    ctx: &mut Ctx,
    flow: &Flow,
    start: u32,
    from: usize,
    to: usize,
    span: &mut usize,
) -> Vec<Atom> {
    let mut atoms = Vec::new();
    let (mut at, end) = (start + from as u32, start + to as u32);
    while at < end {
        while flow.spans[*span].range.end <= at {
            *span += 1;
        }
        let s = &flow.spans[*span];
        let stop = s.range.end.min(end);
        let piece = &ctx.doc.text[at as usize..stop as usize];
        let word = piece.trim_end();
        let font = flow.font(ctx, s);
        let spaces = piece[word.len()..]
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .count();
        let tail = if spaces > 0 {
            spaces as f32
                * ctx
                    .cache
                    .metrics(ctx.shaper, ctx.style, font, ctx.dpi)
                    .space
        } else {
            0.0
        };
        let shaped =
            (!word.is_empty()).then(|| ctx.cache.word(ctx.shaper, ctx.style, font, word, ctx.dpi));
        atoms.push(Atom {
            range: (at, at + word.len() as u32),
            span: *span,
            shaped,
            tail,
        });
        at = stop;
    }
    atoms
}
