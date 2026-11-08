#![forbid(unsafe_code)]

//! Packing words onto lines: a line takes words while they fit, cuts a word
//! wider than a whole line, and sets its words on one shared baseline.

use super::cache::Shaped;
use super::flow::{Atom, EPS, Flow, Flowed, decorate, offset};
use super::geom::{BlockLayout, Ctx, FRect, Piece};

/// A word placed on the line being built.
struct Placed {
    shaped: Shaped,
    x: f32,
    span: usize,
    /// The space after it, up to the next word.
    tail: f32,
}

/// The lines being built.
pub(super) struct Lines {
    x: f32,
    y: f32,
    width: f32,
    /// The top of the line being built, relative to the flow.
    top: f32,
    widest: f32,
    baseline: Option<f32>,
    line: Vec<Placed>,
    cursor: f32,
}

impl Lines {
    /// No lines yet, at `(x, y)`, wrapping at `width`.
    pub fn new(x: f32, y: f32, width: f32) -> Lines {
        Lines {
            x,
            y,
            width,
            top: 0.0,
            widest: 0.0,
            baseline: None,
            line: Vec::new(),
            cursor: 0.0,
        }
    }

    /// The size of the finished lines.
    pub fn flowed(&self) -> Flowed {
        Flowed {
            height: self.top,
            width: self.widest,
            baseline: self.baseline.unwrap_or(0.0),
        }
    }

    /// Places a segment's atoms, starting a new line first if it does not fit
    /// and cutting words that are wider than a whole line.
    pub fn place(&mut self, ctx: &mut Ctx, flow: &Flow, atoms: Vec<Atom>, out: &mut BlockLayout) {
        let mut need = 0.0;
        for (i, atom) in atoms.iter().enumerate() {
            need += atom.shaped.as_ref().map_or(0.0, |s| s.width);
            if i + 1 < atoms.len() {
                need += atom.tail;
            }
        }
        if !self.line.is_empty() && self.cursor + need > self.width + EPS {
            self.finish(ctx, flow, out);
        }
        let split = need > self.width + EPS;
        for atom in atoms {
            let Some(mut shaped) = atom.shaped else {
                self.cursor += atom.tail;
                continue;
            };
            let (mut from, end) = atom.range;
            while split && self.cursor + shaped.width > self.width + EPS {
                if !self.line.is_empty() {
                    self.finish(ctx, flow, out);
                    continue;
                }
                let text = &ctx.doc.text[from as usize..end as usize];
                let hit = shaped.layout.hit_test_point(self.width, 0.0).byte_index;
                let first = text.chars().next().map_or(0, char::len_utf8);
                let cut = floor_char(text, hit).max(first);
                if cut >= text.len() {
                    break;
                }
                let font = flow.font(ctx, &flow.spans[atom.span]);
                let head = ctx
                    .cache
                    .word(ctx.shaper, ctx.style, font, &text[..cut], ctx.dpi);
                self.push(head, atom.span, 0.0);
                self.finish(ctx, flow, out);
                from += cut as u32;
                shaped = ctx
                    .cache
                    .word(ctx.shaper, ctx.style, font, &text[cut..], ctx.dpi);
            }
            self.push(shaped, atom.span, atom.tail);
        }
    }

    fn push(&mut self, shaped: Shaped, span: usize, tail: f32) {
        let x = self.cursor;
        self.cursor += shaped.width + tail;
        self.line.push(Placed {
            shaped,
            x,
            span,
            tail,
        });
    }

    /// Ends the line: aligns its words on one baseline and emits them.
    pub fn finish(&mut self, ctx: &mut Ctx, flow: &Flow, out: &mut BlockLayout) {
        let (height, baseline) = if self.line.is_empty() {
            let font = ctx.style.font(flow.extra, flow.heading, false);
            let m = ctx.cache.metrics(ctx.shaper, ctx.style, font, ctx.dpi);
            (m.height, m.baseline)
        } else {
            let base = self
                .line
                .iter()
                .map(|p| p.shaped.baseline)
                .fold(0.0, f32::max);
            let below = self
                .line
                .iter()
                .map(|p| p.shaped.height - p.shaped.baseline)
                .fold(0.0, f32::max);
            (base + below, base)
        };
        let used = self.line.last().map_or(0.0, |p| p.x + p.shaped.width);
        let dx = if self.width.is_finite() {
            offset(flow.align, self.width - used)
        } else {
            0.0
        };
        let count = self.line.len();
        for (i, p) in self.line.drain(..).enumerate() {
            let span = &flow.spans[p.span];
            let top = self.y + self.top + baseline - p.shaped.baseline;
            let left = self.x + p.x + dx;
            // A run continues across the space to the next word of the same
            // span, so its underline or shading has no gaps.
            let joined = i + 1 < count && p.tail > 0.0;
            let w = p.shaped.width + if joined { p.tail } else { 0.0 };
            decorate(
                ctx,
                out,
                FRect::new(left, top, w, p.shaped.height),
                p.shaped.baseline,
                span,
            );
            out.pieces.push(Piece {
                layout: p.shaped.layout,
                x: left,
                y: top,
                role: flow.role(span),
            });
        }
        self.baseline.get_or_insert(baseline);
        self.widest = self.widest.max(used);
        self.top += height;
        self.cursor = 0.0;
    }
}

/// The largest char boundary of `text` at or before `at`.
fn floor_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
