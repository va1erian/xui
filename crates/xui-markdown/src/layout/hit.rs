#![forbid(unsafe_code)]

//! Hit-testing: from a point to a position in the document, and from a
//! selected range to the rectangles that show it.
//!
//! A position is a byte offset into [`Document::text`](crate::Document::text).
//! Blocks own increasing ranges of that text, so one offset orders and
//! locates a position across the whole document.

use std::ops::Range;

use super::Layout;
use super::geom::{BlockLayout, FRect, Piece};

impl Layout {
    /// The link at `(x, y)` (content pixels, `x` from the text's left edge).
    pub fn link_at(&mut self, x: f32, y: f32) -> Option<u32> {
        let index = self.block_at(y);
        let top = self.top(index);
        let laid = self.block(index)?;
        laid.links
            .iter()
            .find(|(rect, _)| rect.contains(x, y - top))
            .map(|&(_, link)| link)
    }

    /// The document position nearest `(x, y)`: content pixels, `x` from the
    /// text's left edge. A point above or below a block's text goes to its
    /// start or end; one beside a line, to that line's nearest end.
    #[cfg(test)]
    pub fn pos_at(&mut self, x: f32, y: f32) -> u32 {
        self.hit(x, y).map_or(0, |(_, pos)| pos)
    }

    /// The block under `(x, y)` and the position [`pos_at`](Self::pos_at)
    /// gives there, or `None` for a document with no blocks. Two blocks meet
    /// at one offset (the end of one is the start of the next), so a caller
    /// that needs the block (a word or block selection) takes it from here
    /// rather than from the offset.
    pub fn hit(&mut self, x: f32, y: f32) -> Option<(usize, u32)> {
        if self.slots.is_empty() {
            return None;
        }
        let index = self.block_at(y);
        self.lay_out(index);
        let top = self.top(index);
        let block = &self.doc.blocks()[index];
        let (start, end) = (block.text.start, block.text.end);
        let Some(laid) = self.block(index) else {
            return Some((index, start));
        };
        let y = y - top;
        let pos = if y < 0.0 {
            start
        } else if y >= laid.height {
            end
        } else {
            match nearest(laid, x, y) {
                Some(piece) => {
                    let local = piece.layout.hit_test_point(x - piece.x, y - piece.y);
                    piece.offset(self.doc.text(), local.byte_index)
                }
                None => start,
            }
        };
        Some((index, pos))
    }

    /// The rectangles, in block `index`'s coordinates, that show the part of
    /// `selection` in it. Words on one line join across the spaces between
    /// them, so a selected line reads as one band.
    pub fn selection_rects(&self, index: usize, selection: &Range<u32>) -> Vec<FRect> {
        let Some(laid) = self.block(index) else {
            return Vec::new();
        };
        let mut rects: Vec<FRect> = Vec::new();
        for piece in text_pieces(laid) {
            let from = selection.start.max(piece.range.start);
            let to = selection.end.min(piece.range.end);
            if from >= to {
                continue;
            }
            let text = self.doc.text();
            let (a, b) = (piece.local(text, from), piece.local(text, to));
            rects.extend(piece.layout.selection_rects(a, b).iter().map(|r| {
                FRect::new(
                    piece.x + r.left as f32,
                    piece.y + r.top as f32,
                    r.width() as f32,
                    r.height() as f32,
                )
            }));
        }
        join(rects, self.style.size.0 * self.dpi as f32 / 96.0)
    }
}

/// The pieces that show document text (not list markers), in order.
fn text_pieces(laid: &BlockLayout) -> impl Iterator<Item = &Piece> {
    laid.pieces.iter().filter(|p| !p.range.is_empty())
}

/// The text piece nearest `(x, y)`: among those whose lines span `y`, the
/// one nearest along `x`; when none spans it, the vertically nearest.
fn nearest(laid: &BlockLayout, x: f32, y: f32) -> Option<&Piece> {
    let vertical = |p: &Piece| {
        let (top, bottom) = (p.y, p.y + p.layout.height());
        if y < top {
            top - y
        } else if y >= bottom {
            y - bottom
        } else {
            0.0
        }
    };
    let horizontal = |p: &Piece| {
        let (left, right) = (p.x, p.x + p.layout.width());
        if x < left {
            left - x
        } else if x >= right {
            x - right
        } else {
            0.0
        }
    };
    text_pieces(laid).min_by(|a, b| {
        (vertical(a), horizontal(a))
            .partial_cmp(&(vertical(b), horizontal(b)))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Joins rectangles on one line (overlapping by at least half the shorter's
/// height) when less than `gap` apart, into the band that spans them.
fn join(mut rects: Vec<FRect>, gap: f32) -> Vec<FRect> {
    rects.sort_by(|a, b| {
        (a.y, a.x)
            .partial_cmp(&(b.y, b.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<FRect> = Vec::with_capacity(rects.len());
    for r in rects {
        if let Some(last) = out.last_mut() {
            let overlap = (last.y + last.h).min(r.y + r.h) - last.y.max(r.y);
            let same_line = overlap >= 0.5 * last.h.min(r.h);
            let near = r.x - (last.x + last.w) < gap;
            if same_line && near && r.x >= last.x {
                let top = last.y.min(r.y);
                let bottom = (last.y + last.h).max(r.y + r.h);
                let right = (last.x + last.w).max(r.x + r.w);
                *last = FRect::new(last.x, top, right - last.x, bottom - top);
                continue;
            }
        }
        out.push(r);
    }
    out
}
