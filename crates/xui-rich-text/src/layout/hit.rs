#![forbid(unsafe_code)]

//! Hit testing: points to positions, positions to caret and selection boxes,
//! and the image under a point.

use unicode_segmentation::GraphemeCursor;
use xui_core::geometry::{Point, Rect};

use super::{Layout, Line, ParaLayout, PlacedItem, PlacedKind};
use crate::model::{Affinity, DocPos, Document, ObjectId};

/// How much wider than the text a selected paragraph break is drawn, as a
/// fraction of the line height.
const BREAK_STUB: f32 = 0.35;

/// `byte` moved back to the nearest grapheme boundary of `text`.
fn snap_grapheme(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    match cursor.is_boundary(text, 0) {
        Ok(true) | Err(_) => byte,
        Ok(false) => cursor.prev_boundary(text, 0).ok().flatten().unwrap_or(0),
    }
}

/// The byte nearest local `x` inside `item`.
fn byte_in_item(item: &PlacedItem, text: &str, x: f32) -> usize {
    let start = item.range.start;
    match &item.kind {
        PlacedKind::Text(layout) => {
            let at = layout
                .hit_test_point(x, layout.height() / 2.0)
                .byte_index
                .min(item.range.len());
            snap_grapheme(text, start + at)
        }
        PlacedKind::Space { unit } => {
            let mut best = (start, f32::INFINITY);
            let mut edge = 0.0;
            for (offset, c) in text[item.range.clone()].char_indices() {
                if (x - edge).abs() < best.1 {
                    best = (start + offset, (x - edge).abs());
                }
                edge += unit * if c == '\t' { 4.0 } else { 1.0 };
            }
            if (x - edge).abs() < best.1 {
                best.0 = item.range.end;
            }
            best.0
        }
        PlacedKind::Object { .. } if x >= item.width / 2.0 => item.range.end,
        _ => start,
    }
}

/// The x of `byte` inside `item`.
fn x_in_item(item: &PlacedItem, text: &str, byte: usize) -> f32 {
    let offset = byte.saturating_sub(item.range.start);
    if offset == 0 {
        return item.x;
    }
    match &item.kind {
        PlacedKind::Text(layout) if offset < item.range.len() => {
            let right = layout
                .selection_rects(0, offset)
                .iter()
                .map(|r| r.right)
                .max()
                .unwrap_or(0);
            item.x + right as f32
        }
        PlacedKind::Space { unit } => {
            let upto = &text[item.range.start..byte.min(item.range.end)];
            item.x
                + upto
                    .chars()
                    .map(|c| if c == '\t' { 4.0 } else { 1.0 })
                    .sum::<f32>()
                    * unit
        }
        _ => item.x + item.width,
    }
}

impl Line {
    /// The x of `byte` on the line.
    fn caret_x(&self, text: &str, byte: usize) -> f32 {
        for item in &self.items {
            if byte < item.range.end {
                return x_in_item(item, text, byte);
            }
        }
        self.items.last().map_or(self.x, |i| i.x + i.width)
    }

    /// The byte at the end of the text a click right of the line lands on:
    /// before hanging spaces and a forced break, except on the last line.
    fn end_byte(&self, last: bool) -> usize {
        if last {
            return self.range.end;
        }
        self.items
            .iter()
            .rposition(|i| !matches!(i.kind, PlacedKind::Space { .. } | PlacedKind::Break))
            .map_or(self.range.start, |at| self.items[at].range.end)
    }
}

impl ParaLayout {
    fn line_at(&self, y: f32) -> Option<usize> {
        let last = self.lines.len().checked_sub(1)?;
        let at = self.lines.partition_point(|l| self.y + l.y + l.height <= y);
        Some(at.min(last))
    }
}

impl Layout {
    fn para_at(&self, y: f32) -> Option<usize> {
        let last = self.paras.len().checked_sub(1)?;
        let at = self.paras.partition_point(|p| p.bottom() <= y);
        Some(at.min(last))
    }

    /// The position nearest `point` (area pixels). Needs a laid-out document.
    pub fn pos_at(&self, doc: &Document, point: Point) -> DocPos {
        let (x, y) = (point.x as f32, point.y as f32);
        let Some(index) = self.para_at(y) else {
            return DocPos::default();
        };
        let para = &self.paras[index];
        let text = doc.paragraphs()[index].text();
        let Some(li) = para.line_at(y) else {
            return DocPos::new(index, 0);
        };
        let line = &para.lines[li];
        let byte = line
            .items
            .iter()
            .find(|item| x < item.x + item.width)
            .map_or_else(
                || line.end_byte(li + 1 == para.lines.len()),
                |item| {
                    if x < item.x {
                        item.range.start
                    } else {
                        byte_in_item(item, text, x - item.x)
                    }
                },
            );
        DocPos::new(index, byte)
    }

    /// The caret box at `pos` (a one-pixel-wide rectangle as tall as the line),
    /// leaning downstream at a soft wrap.
    pub fn caret_rect(&self, doc: &Document, pos: DocPos) -> Rect {
        self.caret_rect_with(doc, pos, Affinity::Downstream)
    }

    /// The caret box at `pos` with the given affinity.
    pub fn caret_rect_with(&self, doc: &Document, pos: DocPos, affinity: Affinity) -> Rect {
        let index = pos.para;
        if index >= self.paras.len() {
            return Rect::default();
        }
        let para = &self.paras[index];
        let text = doc.paragraphs()[index].text();
        let byte = pos.byte.min(text.len());
        let Some(last) = para.lines.len().checked_sub(1) else {
            return Rect::default();
        };
        let mut li = para
            .lines
            .iter()
            .position(|l| byte < l.range.end)
            .unwrap_or(last);
        if affinity == Affinity::Upstream && li > 0 && para.lines[li].range.start == byte {
            li -= 1;
        }
        let line = &para.lines[li];
        let x = line.caret_x(text, byte).round() as i32;
        let top = (para.y + line.y).round() as i32;
        Rect::new(
            x,
            top,
            x + 1,
            (para.y + line.y + line.height).round() as i32,
        )
    }

    /// The boxes covering the text from `a` to `b`, one per line; a selected
    /// paragraph break is drawn as a short stub past the line's end.
    pub fn selection_rects(&self, doc: &Document, a: DocPos, b: DocPos) -> Vec<Rect> {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let mut out = Vec::new();
        for index in a.para..=b.para.min(self.paras.len().saturating_sub(1)) {
            let para = &self.paras[index];
            let text = doc.paragraphs()[index].text();
            let lo = if index == a.para { a.byte } else { 0 };
            let hi = if index == b.para { b.byte } else { text.len() };
            let breaks = index < b.para;
            for (li, line) in para.lines.iter().enumerate() {
                let from = lo.max(line.range.start);
                let to = hi.min(line.range.end);
                let stub = breaks && li + 1 == para.lines.len();
                if to <= from && !stub {
                    continue;
                }
                let left = line.caret_x(text, from.min(to));
                let mut right = if to > from {
                    line.caret_x(text, to)
                } else {
                    left
                };
                if stub {
                    right += line.height * BREAK_STUB;
                }
                out.push(Rect::new(
                    left.round() as i32,
                    (para.y + line.y).round() as i32,
                    right.round() as i32,
                    (para.y + line.y + line.height).round() as i32,
                ));
            }
        }
        out
    }

    /// The image under `point`: a float's image or an inline image.
    pub fn object_at(&self, point: Point) -> Option<ObjectId> {
        let (x, y) = (point.x as f32, point.y as f32);
        let hit = |left: f32, top: f32, right: f32, bottom: f32| {
            x >= left && x < right && y >= top && y < bottom
        };
        for para in &self.paras {
            for f in &para.floats {
                let r = f.rect.shifted(para.y);
                if hit(r.left, r.top, r.right, r.bottom) {
                    return Some(f.id);
                }
            }
        }
        let para = &self.paras[self.para_at(y)?];
        let line = &para.lines[para.line_at(y)?];
        line.items.iter().find_map(|item| match item.kind {
            PlacedKind::Object { id, height }
                if hit(
                    item.x,
                    para.y + line.baseline - height,
                    item.x + item.width,
                    para.y + line.baseline,
                ) =>
            {
                Some(id)
            }
            _ => None,
        })
    }
}
