#![forbid(unsafe_code)]

//! Rows and columns: items along one axis, aligned across it.

use super::{
    Align, Constraints, Group, Item, LeafFn, Out, Sizing, align_span, clamp_to_caps, cross, main,
};
use crate::geometry::{Rect, Size};
use crate::layout::{Stack, StackDirection, StackSlot, pack};
use crate::units::Px;

/// The constraints an item of a `direction` group is measured in: the group's
/// cross extent bounds it, the main axis is open.
fn item_constraints(direction: StackDirection, cross_extent: Option<i32>, dpi: u32) -> Constraints {
    let open = Constraints::unbounded(dpi);
    match (direction, cross_extent) {
        (_, None) => open,
        (StackDirection::Vertical, Some(width)) => open.with_width(width),
        (StackDirection::Horizontal, Some(height)) => open.with_height(height),
    }
}

/// The group's inner cross extent within `constraints`, if bounded.
fn inner_cross<K: Copy>(
    group: &Group<K>,
    direction: StackDirection,
    constraints: Constraints,
) -> Option<i32> {
    let px = |value: crate::units::Dip| value.to_px(constraints.dpi).value();
    let margins = group.margins;
    match direction {
        StackDirection::Vertical => constraints
            .max_width
            .map(|w| (w - px(margins.left) - px(margins.right)).max(0)),
        StackDirection::Horizontal => constraints
            .max_height
            .map(|h| (h - px(margins.top) - px(margins.bottom)).max(0)),
    }
}

/// The stack slot `item` takes along `direction`.
fn slot<K: Copy>(
    item: &Item<K>,
    direction: StackDirection,
    constraints: Constraints,
    leaf: LeafFn<'_, K>,
) -> StackSlot {
    match item.sizing {
        Sizing::Fill(weight) => StackSlot::Fill(weight),
        Sizing::Fixed(size) => StackSlot::Fixed(size),
        Sizing::Min(size) => StackSlot::Min(size),
        Sizing::Width(size) if direction == StackDirection::Horizontal => StackSlot::Fixed(size),
        Sizing::Height(size) if direction == StackDirection::Vertical => StackSlot::Fixed(size),
        // A named-axis size on the cross axis leaves the main axis at its
        // natural size.
        Sizing::Auto | Sizing::Width(_) | Sizing::Height(_) => {
            let natural = item.natural(direction, constraints, leaf);
            StackSlot::FixedPx(Px(main(direction, natural).max(0)))
        }
    }
}

/// The cross extent `item` asked for by name, if any.
fn named_cross<K: Copy>(item: &Item<K>, direction: StackDirection) -> Option<crate::units::Dip> {
    match (item.sizing, direction) {
        (Sizing::Width(size), StackDirection::Vertical) => Some(size),
        (Sizing::Height(size), StackDirection::Horizontal) => Some(size),
        _ => None,
    }
}

/// Places `group`'s items along `direction` inside `rect`.
pub(super) fn place<K: Copy>(
    group: &Group<K>,
    direction: StackDirection,
    rect: Rect,
    dpi: u32,
    leaf: LeafFn<'_, K>,
    out: &mut Out<K>,
) {
    let visible = group.visible_items(leaf, dpi);
    if visible.is_empty() {
        return;
    }
    let inner = group.margins.apply(rect, dpi);
    let cross_room = cross(direction, inner.size());
    let constraints = item_constraints(direction, Some(cross_room), dpi);
    let slots: Vec<StackSlot> = visible
        .iter()
        .map(|item| slot(item, direction, constraints, leaf))
        .collect();
    let fills = slots.iter().any(|s| matches!(s, StackSlot::Fill(_)));
    let stack = slots.iter().fold(
        match direction {
            StackDirection::Horizontal => Stack::horizontal(),
            StackDirection::Vertical => Stack::vertical(),
        }
        .spacing(group.spacing)
        .margins(group.margins),
        |stack, slot| stack.push(*slot),
    );
    let mut areas = stack.split(rect, dpi);
    if !fills {
        justify(&mut areas, inner, direction, group.justify);
    }
    for (item, area) in visible.iter().zip(areas) {
        let align = item.align.unwrap_or(group.align);
        let area = cross_place(item, area, direction, align, dpi, leaf);
        let area = clamp_to_caps(item, area, align, dpi);
        item.place(area, dpi, leaf, out);
    }
}

/// Shifts packed `areas` along the main axis so they sit where `justify`
/// says inside `inner`.
fn justify(areas: &mut [Rect], inner: Rect, direction: StackDirection, justify: Align) {
    let Some(last) = areas.last() else {
        return;
    };
    let leftover = match direction {
        StackDirection::Horizontal => inner.right - last.right,
        StackDirection::Vertical => inner.bottom - last.bottom,
    };
    let shift = match justify {
        Align::Center => leftover / 2,
        Align::End => leftover,
        Align::Start | Align::Stretch => 0,
    };
    if shift <= 0 {
        return;
    }
    for area in areas {
        *area = match direction {
            StackDirection::Horizontal => {
                Rect::new(area.left + shift, area.top, area.right + shift, area.bottom)
            }
            StackDirection::Vertical => {
                Rect::new(area.left, area.top + shift, area.right, area.bottom + shift)
            }
        };
    }
}

/// Narrows `area` across `direction` to the item's named or natural cross
/// extent, placed by `align`. A stretched item with no named extent keeps the
/// whole area.
fn cross_place<K: Copy>(
    item: &Item<K>,
    area: Rect,
    direction: StackDirection,
    align: Align,
    dpi: u32,
    leaf: LeafFn<'_, K>,
) -> Rect {
    let extent = match (named_cross(item, direction), align) {
        (Some(size), _) => size.to_px(dpi).value(),
        (None, Align::Stretch) => return area,
        (None, _) => {
            // Measured with the main extent the item was given as its bound.
            let given = main(direction, area.size());
            let constraints = match direction {
                StackDirection::Horizontal => Constraints::unbounded(dpi).with_width(given),
                StackDirection::Vertical => Constraints::unbounded(dpi).with_height(given),
            };
            cross(direction, item.natural(direction, constraints, leaf))
        }
    };
    let align = if align == Align::Stretch {
        Align::Start
    } else {
        align
    };
    match direction {
        StackDirection::Horizontal => {
            let (top, bottom) = align_span(area.top, area.bottom, extent, align);
            Rect::new(area.left, top, area.right, bottom)
        }
        StackDirection::Vertical => {
            let (left, right) = align_span(area.left, area.right, extent, align);
            Rect::new(left, area.top, right, area.bottom)
        }
    }
}

/// The natural size of `group`'s items along `direction` within
/// `constraints`: the margins plus each visible item's natural extent plus
/// one gap between neighbours; across it the margins plus the largest natural
/// cross extent.
pub(super) fn measure<K: Copy>(
    group: &Group<K>,
    direction: StackDirection,
    constraints: Constraints,
    leaf: LeafFn<'_, K>,
) -> Size {
    let inner = item_constraints(
        direction,
        inner_cross(group, direction, constraints),
        constraints.dpi,
    );
    let naturals: Vec<Size> = group
        .visible_items(leaf, constraints.dpi)
        .iter()
        .map(|item| item.natural(direction, inner, leaf))
        .collect();
    pack::stack_size(
        direction,
        group.margins,
        group.spacing,
        &naturals,
        constraints.dpi,
    )
}
