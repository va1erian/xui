#![forbid(unsafe_code)]

//! Layered groups: every item in the group's whole inner area, the later
//! above the earlier.

use super::{Constraints, Group, LeafFn, Out, align_both, clamp_to_caps};
use crate::geometry::{Rect, Size};
use crate::layout::StackDirection;

/// Places each of `group`'s items in its inner area, by its alignment.
pub(super) fn place<K: Copy>(
    group: &Group<K>,
    rect: Rect,
    dpi: u32,
    leaf: LeafFn<'_, K>,
    out: &mut Out<K>,
) {
    let inner = group.margins.apply(rect, dpi);
    let constraints = Constraints::unbounded(dpi)
        .with_width(inner.width())
        .with_height(inner.height());
    for item in group.visible_items(leaf, dpi) {
        let align = item.align.unwrap_or(group.align);
        let size = item.own_size(StackDirection::Vertical, constraints, leaf);
        let area = clamp_to_caps(item, align_both(inner, size, align), align, dpi);
        item.place(area, dpi, leaf, out);
    }
}

/// The largest of the items' sizes within `constraints`, plus the margins.
pub(super) fn measure<K: Copy>(
    group: &Group<K>,
    constraints: Constraints,
    leaf: LeafFn<'_, K>,
) -> Size {
    let dpi = constraints.dpi;
    let margins = group.margin_size(dpi);
    let inner = Constraints {
        max_width: constraints
            .max_width
            .map(|width| (width - margins.width).max(0)),
        max_height: constraints
            .max_height
            .map(|height| (height - margins.height).max(0)),
        dpi,
    };
    let size = group
        .visible_items(leaf, dpi)
        .iter()
        .map(|item| item.own_size(StackDirection::Vertical, inner, leaf))
        .fold(Size::new(0, 0), |acc, size| {
            Size::new(acc.width.max(size.width), acc.height.max(size.height))
        });
    Size::new(size.width + margins.width, size.height + margins.height)
}
