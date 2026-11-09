#![forbid(unsafe_code)]

//! Packing words onto lines: a line takes words while they fit, cuts a word
//! wider than a whole line, and sets its words on one shared baseline. An
//! image stands on the baseline, so a tall one makes its line tall.

use super::cache::Shaped;
use super::flow::{Atom, EPS, Flow, Flowed, decorate, offset};
use super::geom::{BlockLayout, Ctx, FRect, ImageBox, Piece};
use super::images::Picture;

/// What is placed: a shaped word or an image.
enum Item {
    Text(Shaped),
    Image(Picture),
}

impl Item {
    fn width(&self) -> f32 {
        match self {
            Item::Text(shaped) => shaped.width,
            Item::Image(picture) => picture.w,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Item::Text(shaped) => shaped.height,
            Item::Image(picture) => picture.h,
        }
    }

    /// From the top: an image's bottom edge sits on the baseline.
    fn baseline(&self) -> f32 {
        match self {
            Item::Text(shaped) => shaped.baseline,
            Item::Image(picture) => picture.h,
        }
    }
}

/// A word or image placed on the line being built.
struct Placed {
    item: Item,
    x: f32,
    /// The bytes of the document it shows.
    range: (u32, u32),
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
            need += atom.width();
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
                match atom.picture {
                    // Scaled to fit a line, it only moves to the next one.
                    Some(picture) => {
                        if !self.line.is_empty() && self.cursor + picture.w > self.width + EPS {
                            self.finish(ctx, flow, out);
                        }
                        self.push(Item::Image(picture), atom.range, atom.span, atom.tail);
                    }
                    None => self.cursor += atom.tail,
                }
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
                self.push(Item::Text(head), (from, from + cut as u32), atom.span, 0.0);
                self.finish(ctx, flow, out);
                from += cut as u32;
                shaped = ctx
                    .cache
                    .word(ctx.shaper, ctx.style, font, &text[cut..], ctx.dpi);
            }
            self.push(Item::Text(shaped), (from, end), atom.span, atom.tail);
        }
    }

    fn push(&mut self, item: Item, range: (u32, u32), span: usize, tail: f32) {
        let x = self.cursor;
        self.cursor += item.width() + tail;
        self.line.push(Placed {
            item,
            x,
            range,
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
                .map(|p| p.item.baseline())
                .fold(0.0, f32::max);
            let below = self
                .line
                .iter()
                .map(|p| p.item.height() - p.item.baseline())
                .fold(0.0, f32::max);
            (base + below, base)
        };
        let used = self.line.last().map_or(0.0, |p| p.x + p.item.width());
        let dx = if self.width.is_finite() {
            offset(flow.align, self.width - used)
        } else {
            0.0
        };
        let count = self.line.len();
        let across = if self.width.is_finite() {
            self.width
        } else {
            used
        };
        let line = FRect::new(self.x, self.y + self.top, across, height);
        for (i, p) in self.line.drain(..).enumerate() {
            let span = &flow.spans[p.span];
            let top = self.y + self.top + baseline - p.item.baseline();
            let left = self.x + p.x + dx;
            let shaped = match p.item {
                Item::Text(shaped) => shaped,
                Item::Image(picture) => {
                    let rect = FRect::new(left, top, picture.w, picture.h);
                    // A linked image is clicked anywhere on it, and not
                    // underlined.
                    if let Some(link) = span.link {
                        out.links.push((rect, link));
                    }
                    out.images.push(ImageBox {
                        rect,
                        image: picture.image,
                        range: p.range.0..p.range.1,
                        line,
                    });
                    continue;
                }
            };
            // A run continues across the space to the next word of the same
            // span, so its underline or shading has no gaps.
            let joined = i + 1 < count && p.tail > 0.0;
            let w = shaped.width + if joined { p.tail } else { 0.0 };
            decorate(
                ctx,
                out,
                FRect::new(left, top, w, shaped.height),
                shaped.baseline,
                span,
            );
            out.pieces.push(Piece {
                layout: shaped.layout,
                x: left,
                y: top,
                range: p.range.0..p.range.1,
                tabs: false,
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
