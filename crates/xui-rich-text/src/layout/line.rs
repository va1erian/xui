#![forbid(unsafe_code)]

//! Greedy line breaking in the intervals the floats leave free.
//!
//! Items are grouped into atoms (runs with no break opportunity between them).
//! A line takes atoms while their content fits; a taller atom re-queries the
//! interval; a line that cannot fit even one atom drops below the float that
//! narrows it, or splits the atom at a grapheme when nothing narrows it.

mod finish;
mod split;

use super::floats::FloatCtx;
use super::items::{Item, ItemKind, ShapeCtx};
use super::resolve::Spacing;
use super::segment::Brk;
use super::{Line, PlacedFloat};
use crate::model::{Align, CharStyleId, Wrap};

/// Slack for float rounding when testing whether content fits.
const EPS: f32 = 0.1;

/// How one paragraph's lines are broken.
pub(crate) struct Params {
    /// The width of the area.
    pub area: f32,
    pub left: f32,
    pub right: f32,
    pub first: f32,
    pub align: Align,
    pub spacing: Spacing,
    /// The style of an empty paragraph, for its line's height.
    pub empty_style: CharStyleId,
}

/// The lines and floats of one paragraph, in area coordinates.
pub(crate) struct Broken {
    pub lines: Vec<Line>,
    pub floats: Vec<PlacedFloat>,
    /// Where the next paragraph starts.
    pub end: f32,
}

/// Breaks `items` into lines starting at `y`, placing floats into `floats`.
pub(crate) fn break_lines(
    text: &str,
    items: Vec<Item>,
    params: &Params,
    floats: &mut FloatCtx,
    y: f32,
    shape: &mut ShapeCtx<'_>,
) -> Broken {
    let mut breaker = Breaker {
        text,
        items,
        p: params,
        ctx: floats,
        shape,
        y,
        i: 0,
        first: true,
        force: (0.0, 0.0),
        covered: 0,
        lines: Vec::new(),
        placed: Vec::new(),
        need_empty: false,
    };
    while breaker.i < breaker.items.len() || breaker.lines.is_empty() || breaker.need_empty {
        breaker.need_empty = false;
        breaker.line();
    }
    if let Some(last) = breaker.lines.last_mut() {
        last.range.end = text.len();
    }
    Broken {
        lines: breaker.lines,
        floats: breaker.placed,
        end: breaker.y,
    }
}

/// The index just past the atom starting at `j`.
fn atom_end(items: &[Item], j: usize) -> usize {
    items[j..]
        .iter()
        .position(|item| item.brk != Brk::None)
        .map_or(items.len(), |at| j + at + 1)
}

/// The items accepted on the line being built.
struct Acc {
    items: Vec<Item>,
    used: f32,
    ascent: f32,
    descent: f32,
    content: usize,
}

impl Acc {
    fn new(force: (f32, f32)) -> Acc {
        Acc {
            items: Vec::new(),
            used: 0.0,
            ascent: force.0,
            descent: force.1,
            content: 0,
        }
    }

    fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    fn push(&mut self, item: &Item) {
        self.used += item.width;
        if !matches!(item.kind, ItemKind::Float { .. }) {
            self.content += 1;
            self.ascent = self.ascent.max(item.ascent);
            self.descent = self.descent.max(item.descent);
        }
        self.items.push(item.clone());
    }
}

struct Breaker<'a, 'b> {
    text: &'a str,
    items: Vec<Item>,
    p: &'a Params,
    ctx: &'a mut FloatCtx,
    shape: &'a mut ShapeCtx<'b>,
    y: f32,
    i: usize,
    first: bool,
    /// A line height a restart must keep (a taller atom was seen).
    force: (f32, f32),
    /// The byte the last line ended at.
    covered: usize,
    lines: Vec<Line>,
    placed: Vec<PlacedFloat>,
    need_empty: bool,
}

impl Breaker<'_, '_> {
    fn spaced(&self, height: f32) -> f32 {
        match self.p.spacing {
            Spacing::Multiple(m) => height * m,
            Spacing::Exactly(h) => h,
        }
    }

    fn bounds(&self) -> (f32, f32) {
        let lo = (self.p.left + if self.first { self.p.first } else { 0.0 }).max(0.0);
        (lo, (self.p.area - self.p.right).max(lo))
    }

    /// Lays out one line, restarting it when a float or a taller atom changes
    /// the interval it was measured in.
    fn line(&mut self) {
        'attempt: loop {
            let start = self.i;
            let (lo, hi) = self.bounds();
            let mut acc = Acc::new(self.force);
            let iv = self.ctx.interval(self.y, self.spaced(acc.height()), lo, hi);
            let mut j = start;
            let mut mandatory = false;
            while j < self.items.len() {
                if let ItemKind::Float { id, size, wrap } = self.items[j].kind.clone() {
                    if self.placed.iter().any(|f| f.id == id) {
                        let marker = self.items[j].clone();
                        acc.push(&marker);
                        j += 1;
                        continue;
                    }
                    match wrap {
                        Wrap::TopAndBottom { .. } if acc.content > 0 => break,
                        Wrap::TopAndBottom { margin } => {
                            self.place_band(j, id, size, margin.0 * self.shape.scale);
                            self.i = j + 1;
                            self.force = (0.0, 0.0);
                            if self.i >= self.items.len() && !self.lines.is_empty() {
                                return;
                            }
                            continue 'attempt;
                        }
                        Wrap::Square { side, margin } => {
                            let size = self.clamp(size);
                            let rect = self.ctx.place_square(
                                self.p.area,
                                self.y,
                                size,
                                margin.0 * self.shape.scale,
                                side,
                            );
                            self.placed.push(PlacedFloat {
                                id,
                                byte: self.items[j].range.start,
                                rect,
                            });
                            self.i = start;
                            continue 'attempt;
                        }
                        Wrap::Inline => {}
                    }
                }
                let end = atom_end(&self.items, j);
                let (content, ascent, descent) = measure(&self.items[j..end]);
                let (asc, desc) = (acc.ascent.max(ascent), acc.descent.max(descent));
                if asc + desc > acc.height() + EPS {
                    let grown = self.ctx.interval(self.y, self.spaced(asc + desc), lo, hi);
                    if grown != iv {
                        if acc.used + content <= grown.width() + EPS || acc.content == 0 {
                            self.force = (asc, desc);
                            continue 'attempt;
                        }
                        break;
                    }
                }
                if acc.used + content <= iv.width() + EPS {
                    for item in &self.items[j..end] {
                        acc.push(item);
                    }
                    j = end;
                    if self.items[end - 1].brk == Brk::Mandatory {
                        mandatory = true;
                        break;
                    }
                    continue;
                }
                if acc.content > 0 {
                    break;
                }
                let h = self.spaced(acc.height().max(asc + desc));
                if iv.width() + EPS < hi - lo
                    && let Some(bottom) = self.ctx.next_bottom(self.y, h)
                {
                    self.y = bottom;
                    self.force = (0.0, 0.0);
                    continue 'attempt;
                }
                j = self.overflow(&mut acc, j, end, iv.width());
                break;
            }
            self.finish(start, acc, j, mandatory);
            return;
        }
    }

    fn clamp(&self, size: (f32, f32)) -> (f32, f32) {
        if size.0 > self.p.area && size.0 > 0.0 {
            (self.p.area, size.1 * self.p.area / size.0)
        } else {
            size
        }
    }

    fn place_band(&mut self, j: usize, id: crate::model::ObjectId, size: (f32, f32), margin: f32) {
        let size = self.clamp(size);
        let x = match self.p.align {
            Align::Center => (self.p.area - size.0) / 2.0,
            Align::Right => self.p.area - size.0,
            _ => 0.0,
        };
        let rect = self.ctx.place_band(self.p.area, self.y, size, margin, x);
        self.placed.push(PlacedFloat {
            id,
            byte: self.items[j].range.start,
            rect,
        });
        self.y = rect.bottom + margin;
    }
}

/// The content width (trailing spaces excluded), ascent and descent of an atom.
fn measure(atom: &[Item]) -> (f32, f32, f32) {
    let hang = atom.iter().rev().take_while(|i| i.is_space()).count();
    let content: f32 = atom[..atom.len() - hang].iter().map(|i| i.width).sum();
    let (ascent, descent) = atom.iter().fold((0.0f32, 0.0f32), |(a, d), i| {
        (a.max(i.ascent), d.max(i.descent))
    });
    (content, ascent, descent)
}
