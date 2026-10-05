#![forbid(unsafe_code)]

//! Wraps: items left to right at their natural sizes, broken into lines.
//!
//! An item wider than the group gets the whole line, at the group's width.
//! Items keep their natural width; a `Fill` item is measured like an `Auto`
//! one, since a line has no leftover space to share.

use super::{Align, Constraints, Group, Item, LeafFn, Out, align_span, clamp_to_caps};
use crate::geometry::{Rect, Size};
use crate::layout::StackDirection;
use crate::units::Dip;

/// One line: its items with their sizes, and the line's width and height.
struct Line<'a, K> {
    items: Vec<(&'a Item<K>, Size)>,
    width: i32,
    height: i32,
}

/// Breaks `items` into lines no wider than `room` (unbounded when `None`).
fn lines<'a, K: Copy>(
    items: &[&'a Item<K>],
    room: Option<i32>,
    gap: i32,
    dpi: u32,
    leaf: LeafFn<'_, K>,
) -> Vec<Line<'a, K>> {
    let mut constraints = Constraints::unbounded(dpi);
    if let Some(room) = room {
        constraints = constraints.with_width(room);
    }
    let mut lines: Vec<Line<'a, K>> = Vec::new();
    for item in items {
        let mut size = item.own_size(StackDirection::Horizontal, constraints, leaf);
        if let Some(room) = room {
            size.width = size.width.min(room);
        }
        let fits =
            |line: &Line<'a, K>| room.is_none_or(|room| line.width + gap + size.width <= room);
        match lines.last_mut() {
            Some(line) if fits(line) => {
                line.width += gap + size.width;
                line.height = line.height.max(size.height);
                line.items.push((item, size));
            }
            _ => lines.push(Line {
                items: vec![(item, size)],
                width: size.width,
                height: size.height,
            }),
        }
    }
    lines
}

/// Places `group`'s items in lines inside `rect`.
pub(super) fn place<K: Copy>(
    group: &Group<K>,
    rect: Rect,
    dpi: u32,
    leaf: LeafFn<'_, K>,
    out: &mut Out<K>,
) {
    let visible = group.visible_items(leaf, dpi);
    let inner = group.margins.apply(rect, dpi);
    let gap = px(group.spacing, dpi);
    let mut top = inner.top;
    for line in lines(&visible, Some(inner.width()), gap, dpi, leaf) {
        let leftover = (inner.width() - line.width).max(0);
        let mut left = inner.left
            + match group.justify {
                Align::Center => leftover / 2,
                Align::End => leftover,
                Align::Start | Align::Stretch => 0,
            };
        // Lines past the bottom collapse onto it rather than escape.
        let line_top = top.min(inner.bottom);
        let bottom = (top + line.height).min(inner.bottom);
        for (item, size) in line.items {
            let aligns = item.aligns(group.align);
            let (y0, y1) = align_span(line_top, bottom, size.height, aligns.1);
            let right = (left + size.width).min(inner.right);
            let area = clamp_to_caps(item, Rect::new(left, y0, right, y1), aligns, dpi);
            item.place(area, dpi, leaf, out);
            left += size.width + gap;
        }
        top += line.height + gap;
    }
}

/// The wrap's natural size within `constraints`: its lines at the width
/// bound (or one line without one), plus the margins.
pub(super) fn measure<K: Copy>(
    group: &Group<K>,
    constraints: Constraints,
    leaf: LeafFn<'_, K>,
) -> Size {
    let dpi = constraints.dpi;
    let margins = group.margin_size(dpi);
    let room = constraints
        .max_width
        .map(|width| (width - margins.width).max(0));
    let gap = px(group.spacing, dpi);
    let visible = group.visible_items(leaf, dpi);
    let lines = lines(&visible, room, gap, dpi, leaf);
    let width = lines.iter().map(|line| line.width).max().unwrap_or(0);
    let height = lines.iter().map(|line| line.height).sum::<i32>()
        + gap * (lines.len().saturating_sub(1) as i32);
    Size::new(width + margins.width, height + margins.height)
}

fn px(value: Dip, dpi: u32) -> i32 {
    value.to_px(dpi).value().max(0)
}
