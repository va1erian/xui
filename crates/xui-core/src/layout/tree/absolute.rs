#![forbid(unsafe_code)]

//! Absolute groups: each item at the position it was designed at, moved and
//! stretched by its [`Anchor`](crate::layout::Anchor) as the group's size
//! differs from its design size. This is the one home for free positioning: a
//! form designer's surface and forms imported from absolute coordinates.

use super::{Constraints, Group, Item, LeafFn, Out};
use crate::geometry::{Rect, Size};
use crate::layout::{StackDirection, anchored};
use crate::units::Dip;

/// The item's design rectangle in device pixels, relative to the group's inner
/// top-left: its [`Item::at`] position, or its natural size at the origin.
fn design_rect<K: Copy>(item: &Item<K>, dpi: u32, leaf: LeafFn<'_, K>) -> Rect {
    let px = |value: Dip| value.to_px(dpi).value();
    match item.free_position().0 {
        Some([x, y, width, height]) => {
            let (x, y) = (px(x), px(y));
            Rect::new(x, y, x + px(width).max(0), y + px(height).max(0))
        }
        None => Rect::from_size(item.own_size(
            StackDirection::Vertical,
            Constraints::unbounded(dpi),
            leaf,
        )),
    }
}

/// The inner size the positions were designed at: `design`, or the smallest
/// that holds every visible item.
fn design_size<K: Copy>(
    group: &Group<K>,
    design: Option<(Dip, Dip)>,
    dpi: u32,
    leaf: LeafFn<'_, K>,
) -> Size {
    match design {
        Some((width, height)) => Size::new(
            width.to_px(dpi).value().max(0),
            height.to_px(dpi).value().max(0),
        ),
        None => group
            .visible_items(leaf, dpi)
            .iter()
            .map(|item| design_rect(item, dpi, leaf))
            .fold(Size::new(0, 0), |acc, rect| {
                Size::new(acc.width.max(rect.right), acc.height.max(rect.bottom))
            }),
    }
}

/// Places each of `group`'s items at its anchored design rectangle inside
/// `rect`.
pub(super) fn place<K: Copy>(
    group: &Group<K>,
    design: Option<(Dip, Dip)>,
    rect: Rect,
    dpi: u32,
    leaf: LeafFn<'_, K>,
    out: &mut Out<K>,
) {
    let inner = group.margins.apply(rect, dpi);
    let origin = design_size(group, design, dpi, leaf);
    for item in group.visible_items(leaf, dpi) {
        let placed = anchored(
            origin,
            inner.size(),
            design_rect(item, dpi, leaf),
            item.free_position().1,
        );
        item.place(placed.offset(inner.left, inner.top), dpi, leaf, out);
    }
}

/// The design size plus the margins; constraints do not move free items.
pub(super) fn measure<K: Copy>(
    group: &Group<K>,
    design: Option<(Dip, Dip)>,
    constraints: Constraints,
    leaf: LeafFn<'_, K>,
) -> Size {
    let dpi = constraints.dpi;
    let size = design_size(group, design, dpi, leaf);
    let margins = group.margin_size(dpi);
    Size::new(size.width + margins.width, size.height + margins.height)
}
