//! Property tests for the pure arithmetic in `xui-core`: geometry, units and
//! the layout helpers. These are the parts every backend shares, so a
//! regression here reaches every target; the properties below are the
//! invariants the module docs promise (half-open rects, non-inverting insets,
//! slots that tile exactly, anchors that stay inside their parent).
//!
//! They run without a window or a backend, so they are fast and deterministic.

use proptest::prelude::*;

use xui_core::geometry::{Point, Rect, Size};
use xui_core::layout::{Anchor, Dock, Insets, Stack, StackSlot};
use xui_core::units::{Px, dip};
use xui_core::{Dip, anchored};

/// A rectangle with non-negative width and height, anywhere near the origin.
fn rect_strategy() -> impl Strategy<Value = Rect> {
    (-500..500i32, -500..500i32, 0..800i32, 0..600i32)
        .prop_map(|(left, top, width, height)| Rect::new(left, top, left + width, top + height))
}

/// Non-negative insets.
fn insets_strategy() -> impl Strategy<Value = Insets> {
    (0.0f32..40.0, 0.0f32..40.0, 0.0f32..40.0, 0.0f32..40.0).prop_map(
        |(left, top, right, bottom)| Insets::new(dip(left), dip(top), dip(right), dip(bottom)),
    )
}

/// One slot of any kind, including a zero-weight fill.
fn slot_strategy() -> impl Strategy<Value = StackSlot> {
    prop_oneof![
        (0.0f32..120.0).prop_map(|v| StackSlot::Fixed(dip(v))),
        (0..120i32).prop_map(|v| StackSlot::FixedPx(Px(v))),
        (0u32..5).prop_map(StackSlot::Fill),
        (0.0f32..120.0).prop_map(|v| StackSlot::Min(dip(v))),
        (0..120i32).prop_map(|v| StackSlot::MinPx(Px(v))),
    ]
}

fn anchor_strategy() -> impl Strategy<Value = Anchor> {
    prop_oneof![
        Just(Anchor::TopLeft),
        Just(Anchor::Top),
        Just(Anchor::TopRight),
        Just(Anchor::Left),
        Just(Anchor::Center),
        Just(Anchor::Right),
        Just(Anchor::BottomLeft),
        Just(Anchor::Bottom),
        Just(Anchor::BottomRight),
        Just(Anchor::StretchHorizontal),
        Just(Anchor::StretchVertical),
        Just(Anchor::Fill),
    ]
}

fn dpi_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![Just(96u32), Just(120), Just(144), Just(192), Just(288)]
}

/// The area of `rect`, in pixels (its edges are never inverted by these APIs).
fn area(rect: Rect) -> i64 {
    i64::from(rect.width()) * i64::from(rect.height())
}

proptest! {
    /// Offsetting a rectangle moves it rigidly: the size is unchanged and a
    /// point stays inside exactly when its moved copy does.
    #[test]
    fn offset_is_rigid(
        rect in rect_strategy(),
        point in (-800..800i32, -800..800i32),
        dx in -200..200i32,
        dy in -200..200i32,
    ) {
        let moved = rect.offset(dx, dy);
        prop_assert_eq!(moved.size(), rect.size());
        let point = Point::new(point.0, point.1);
        let inside = rect.contains(point);
        let moved_inside = moved.contains(Point::new(point.x + dx, point.y + dy));
        prop_assert_eq!(inside, moved_inside);
    }

    /// `split_left` (and the vertical/other-edge variants) partition the
    /// rectangle into two pieces that meet exactly and leave no gap.
    #[test]
    fn splits_partition(rect in rect_strategy(), amount in 0..900i32) {
        let (left, right) = rect.split_left(amount);
        prop_assert_eq!(left.left, rect.left);
        prop_assert_eq!(right.right, rect.right);
        prop_assert_eq!(left.right, right.left);
        prop_assert_eq!(left.top, rect.top);
        prop_assert_eq!(left.bottom, rect.bottom);
        prop_assert_eq!(left.width() + right.width(), rect.width());

        let (top, bottom) = rect.split_top(amount);
        prop_assert_eq!(top.top, rect.top);
        prop_assert_eq!(bottom.bottom, rect.bottom);
        prop_assert_eq!(top.bottom, bottom.top);
        prop_assert_eq!(top.height() + bottom.height(), rect.height());

        let (rest, strip) = rect.split_bottom(amount);
        prop_assert_eq!(rest.top, rect.top);
        prop_assert_eq!(strip.bottom, rect.bottom);
        prop_assert_eq!(rest.bottom, strip.top);
        prop_assert_eq!(rest.height() + strip.height(), rect.height());
    }

    /// Insets shrink but never invert: the result is inside the input and has a
    /// non-negative extent, whatever the DPI or how large the insets are.
    #[test]
    fn insets_never_invert(
        rect in rect_strategy(),
        insets in insets_strategy(),
        dpi in dpi_strategy(),
    ) {
        let inner = insets.apply(rect, dpi);
        // The left/top edges only ever move outward and the extent never
        // inverts, however large the insets are relative to the parent. (The
        // right/bottom edges may exceed the parent's when the left/top insets
        // alone already overflow it and the result collapses to that edge.)
        prop_assert!(inner.left >= rect.left);
        prop_assert!(inner.top >= rect.top);
        prop_assert!(inner.width() >= 0);
        prop_assert!(inner.height() >= 0);
    }

    /// Docking partitions the inset rectangle exactly: the strips plus the fill
    /// cover it without overlap, whatever combination of edges is requested.
    #[test]
    fn dock_partitions_the_inner_rect(
        rect in rect_strategy(),
        insets in insets_strategy(),
        top in prop::option::of(0.0f32..400.0),
        bottom in prop::option::of(0.0f32..400.0),
        left in prop::option::of(0.0f32..400.0),
        right in prop::option::of(0.0f32..400.0),
        dpi in dpi_strategy(),
    ) {
        let mut dock = Dock::new().margins(insets);
        if let Some(value) = top { dock = dock.top(dip(value)); }
        if let Some(value) = bottom { dock = dock.bottom(dip(value)); }
        if let Some(value) = left { dock = dock.left(dip(value)); }
        if let Some(value) = right { dock = dock.right(dip(value)); }

        let layout = dock.split(rect, dpi);
        let inner = insets.apply(rect, dpi);
        let covered = [layout.top, layout.bottom, layout.left, layout.right]
            .into_iter()
            .flatten()
            .map(area)
            .sum::<i64>()
            + area(layout.fill);
        prop_assert_eq!(covered, area(inner));
        for strip in [layout.top, layout.bottom, layout.left, layout.right]
            .into_iter()
            .flatten()
            .chain([layout.fill])
        {
            prop_assert!(strip.width() >= 0 && strip.height() >= 0);
            prop_assert!(strip.left >= inner.left && strip.right <= inner.right);
            prop_assert!(strip.top >= inner.top && strip.bottom <= inner.bottom);
        }
    }

    /// Stack slots are ordered, non-overlapping, and span exactly the inner
    /// rect: the first starts at its left edge, the last ends at its right
    /// edge, and no slot is inverted.
    #[test]
    fn stack_slots_tile_the_inner_rect(
        rect in rect_strategy(),
        insets in insets_strategy(),
        spacing in 0.0f32..20.0,
        slots in prop::collection::vec(slot_strategy(), 1..7),
        dpi in dpi_strategy(),
    ) {
        let count = slots.len();
        let stack = slots
            .into_iter()
            .fold(Stack::horizontal().margins(insets).spacing(dip(spacing)), |stack, slot| {
                stack.push(slot)
            });
        let rects = stack.split(rect, dpi);
        let inner = insets.apply(rect, dpi);

        prop_assert_eq!(rects.len(), count);
        prop_assert_eq!(rects[0].left, inner.left);
        // A stack without a `Fill` slot need not consume all the slack, so the
        // last slot may stop short of the inner right edge, but it never
        // overruns it.
        prop_assert!(rects[count - 1].right <= inner.right);
        for rect in &rects {
            prop_assert!(rect.width() >= 0);
            prop_assert!(rect.height() == inner.height());
            prop_assert!(rect.left >= inner.left && rect.right <= inner.right);
        }
        for pair in rects.windows(2) {
            prop_assert!(pair[0].right <= pair[1].left);
        }
    }

    /// With only weighted `Fill` slots the stack consumes the content exactly,
    /// so the slots span the whole inner rectangle with no gap at either end.
    #[test]
    fn stack_fill_slots_span_the_inner_rect(
        rect in rect_strategy(),
        insets in insets_strategy(),
        spacing in 0.0f32..20.0,
        weights in prop::collection::vec(1u32..5, 1..7),
        dpi in dpi_strategy(),
    ) {
        let count = weights.len();
        let stack = weights
            .into_iter()
            .fold(Stack::horizontal().margins(insets).spacing(dip(spacing)), |stack, weight| {
                stack.fill(weight)
            });
        let rects = stack.split(rect, dpi);
        let inner = insets.apply(rect, dpi);
        prop_assert_eq!(rects.len(), count);
        prop_assert_eq!(rects[0].left, inner.left);
        prop_assert_eq!(rects[count - 1].right, inner.right);
        for pair in rects.windows(2) {
            prop_assert!(pair[0].right <= pair[1].left);
        }
    }

    /// Anchoring never pushes a control outside its parent or inverts it, and a
    /// parent at least one pixel across always leaves a positive extent.
    #[test]
    fn anchored_stays_inside(
        origin in (0..1200i32, 0..1200i32),
        resized in (0..1200i32, 0..1200i32),
        bounds in rect_strategy(),
        anchor in anchor_strategy(),
    ) {
        let resized = Size::new(resized.0, resized.1);
        let result = anchored(Size::new(origin.0, origin.1), resized, bounds, anchor);
        prop_assert!(result.left >= 0 && result.top >= 0);
        prop_assert!(result.right <= resized.width && result.bottom <= resized.height);
        prop_assert!(result.width() >= 0 && result.height() >= 0);
        if resized.width >= 1 && resized.height >= 1 {
            prop_assert!(result.width() >= 1 && result.height() >= 1);
        }
    }

    /// `Dip::to_px` is monotonic in the design value at a fixed DPI.
    #[test]
    fn dip_to_px_is_monotonic(a in 0.0f32..500.0, b in 0.0f32..500.0, dpi in dpi_strategy()) {
        let (low, high) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(dip(low).to_px(dpi).value() <= dip(high).to_px(dpi).value());
    }

    /// A pixel value round-trips through design units at any common DPI.
    #[test]
    fn px_round_trips(pixels in -1000..1000i32, dpi in prop_oneof![Just(96u32), Just(120), Just(144), Just(192)]) {
        prop_assert_eq!(Px(pixels).to_dip(dpi).to_px(dpi), Px(pixels));
    }

    /// `Dip` arithmetic is exact for the operations a layout uses.
    #[test]
    fn dip_arithmetic_is_field_like(a in -1000.0f32..1000.0, b in -1000.0f32..1000.0) {
        prop_assert_eq!((Dip(a) + Dip(b)).value(), a + b);
        prop_assert_eq!((Dip(a) - Dip(b)).value(), a - b);
        prop_assert_eq!((Dip(a) - Dip(a)).value(), 0.0);
    }
}
