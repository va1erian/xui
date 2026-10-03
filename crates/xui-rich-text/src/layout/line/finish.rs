#![forbid(unsafe_code)]

//! Closing a line: its height, alignment and justification.

use super::{Acc, Breaker};
use crate::layout::items::{Item, ItemKind};
use crate::layout::resolve::Spacing;
use crate::layout::{Line, PlacedItem, PlacedKind};
use crate::model::Align;

impl Breaker<'_, '_> {
    /// Closes the line made of `acc`, aligning it and advancing `y`.
    pub(super) fn finish(&mut self, start: usize, acc: Acc, j: usize, mandatory: bool) {
        let (mut ascent, mut descent) = (acc.ascent, acc.descent);
        if acc.content == 0 {
            let style = self
                .items
                .get(start)
                .or_else(|| self.items.last())
                .map_or(self.p.empty_style, |item| item.style);
            let (a, d) = self.shape.strut(style);
            ascent = ascent.max(a);
            descent = descent.max(d);
        }
        let natural = ascent + descent;
        let height = self.spaced(natural);
        let ascent = match self.p.spacing {
            Spacing::Multiple(_) => ascent + (height - natural),
            Spacing::Exactly(h) => (h - descent).max(0.0),
        };
        let (lo, hi) = self.bounds();
        let iv = self.ctx.interval(self.y, height, lo, hi);
        let last = j >= self.items.len() || mandatory;
        let (items, x) = self.align(acc.items, iv.lo, iv.width(), last);
        let range = self.covered..items.last().map_or(self.covered, |i| i.range.end);
        self.covered = range.end;
        self.lines.push(Line {
            range,
            y: self.y,
            height,
            baseline: self.y + ascent,
            x,
            items,
        });
        self.y += height;
        self.i = j;
        self.first = false;
        self.force = (0.0, 0.0);
        self.need_empty = mandatory && j >= self.items.len();
    }

    /// Positions `items` in `[lo, lo + avail]` per the paragraph's alignment.
    fn align(&self, items: Vec<Item>, lo: f32, avail: f32, last: bool) -> (Vec<PlacedItem>, f32) {
        let hang = items.iter().rev().take_while(|i| i.is_space()).count();
        let body = items.len() - hang;
        let used: f32 = items[..body].iter().map(|i| i.width).sum();
        let gaps = items[..body].iter().filter(|i| i.is_space()).count();
        let stretch = if self.p.align == Align::Justify && !last && gaps > 0 {
            ((avail - used) / gaps as f32).max(0.0)
        } else {
            0.0
        };
        let used = used + stretch * gaps as f32;
        let mut x = match self.p.align {
            Align::Center => lo + ((avail - used) / 2.0).max(0.0),
            Align::Right => lo + (avail - used).max(0.0),
            _ => lo,
        };
        let origin = x;
        let placed = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let mut width = item.width;
                if stretch > 0.0 && index < body && item.is_space() {
                    width += stretch;
                }
                let kind = match item.kind {
                    ItemKind::Text(layout) => PlacedKind::Text(layout),
                    ItemKind::Space { unit } if item.width > 0.0 => PlacedKind::Space {
                        unit: unit * width / item.width,
                    },
                    ItemKind::Space { unit } => PlacedKind::Space { unit },
                    ItemKind::Object { id, height } => PlacedKind::Object { id, height },
                    ItemKind::Float { id, .. } => PlacedKind::Float(id),
                    ItemKind::Break => PlacedKind::Break,
                };
                let placed = PlacedItem {
                    x,
                    width,
                    range: item.range,
                    kind,
                    style: item.style,
                    dy: item.dy,
                    size: item.size,
                };
                x += width;
                placed
            })
            .collect();
        (placed, origin)
    }
}
