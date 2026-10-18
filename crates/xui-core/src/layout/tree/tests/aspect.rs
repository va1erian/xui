//! Aspect ratios: the cross extent follows the fixed or main one, within
//! caps and constraints, and an absolute group ignores the ratio.

use super::{leaf, rects};
use crate::geometry::{Rect, Size};
use crate::layout::Insets;
use crate::layout::tree::{Align, Group, Item, Sizing};
use crate::units::dip;

#[test]
fn a_fixed_main_extent_gives_the_cross_extent_the_ratio() {
    let tree = Group::row().push(Item::leaf(0).sized(Sizing::Fixed(dip(80.0))).aspect(2.0));
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(rects(&placed), [Rect::new(0, 0, 80, 40)]);
}

#[test]
fn a_column_fixed_item_derives_its_width_from_its_height() {
    let tree = Group::column().push(Item::leaf(0).sized(Sizing::Fixed(dip(50.0))).aspect(2.0));
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(rects(&placed), [Rect::new(0, 0, 100, 50)]);
}

#[test]
fn a_fill_item_takes_its_ratio_from_the_placed_area() {
    let tree = Group::column().push(Item::leaf(0).sized(Sizing::Fill(1)).aspect(2.0));
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(0, 0, 200, 100)],
        "the width exactly matches the ratio"
    );

    let narrow = tree.compute(Rect::new(0, 0, 100, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(
        rects(&narrow),
        [Rect::new(0, 0, 100, 50)],
        "a width bound shrinks the height to keep the ratio"
    );
}

#[test]
fn an_auto_item_derives_the_cross_extent_from_its_main_one() {
    let tree = Group::row().push(Item::leaf(0).aspect(3.0));
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 60, 20)], &[]));
    assert_eq!(rects(&placed), [Rect::new(0, 0, 60, 20)]);

    let tall = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 60, 100)], &[]));
    assert_eq!(
        rects(&tall),
        [Rect::new(0, 0, 60, 20)],
        "the natural height shrinks to the ratio"
    );

    let column = Group::column().push(Item::leaf(0).aspect(3.0));
    let placed = column.compute(Rect::new(0, 0, 400, 200), 96, &leaf(&[(0, 60, 100)], &[]));
    assert_eq!(rects(&placed), [Rect::new(0, 0, 300, 100)]);
}

#[test]
fn a_named_axis_wins_in_either_direction() {
    let tree = Group::column().push(Item::leaf(0).sized(Sizing::Width(dip(80.0))).aspect(2.0));
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(rects(&placed), [Rect::new(0, 0, 80, 40)]);
}

#[test]
fn caps_shrink_the_ratio_box_to_fit() {
    let tree = Group::row().push(
        Item::leaf(0)
            .sized(Sizing::Fixed(dip(80.0)))
            .max_height(dip(20.0))
            .aspect(2.0),
    );
    let natural = leaf(&[(0, 10, 10)], &[]);
    assert_eq!(
        tree.preferred_size(96, &natural),
        crate::geometry::Size::new(40, 20)
    );
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &natural);
    assert_eq!(rects(&placed), [Rect::new(0, 0, 40, 20)]);
}

#[test]
fn a_wrap_keeps_the_ratio_per_item() {
    let tree = Group::wrap()
        .spacing(dip(10.0))
        .push(Item::leaf(0).sized(Sizing::Width(dip(80.0))).aspect(2.0))
        .push(Item::leaf(1).sized(Sizing::Width(dip(80.0))).aspect(2.0));
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(0, 0, 80, 40), Rect::new(90, 0, 170, 40)]
    );
}

#[test]
fn a_box_fixed_on_both_axes_shrinks_to_the_ratio_within_them() {
    let tree = Group::row().push(
        Item::leaf(0)
            .size(Some(dip(100.0)), Some(dip(30.0)))
            .aspect(2.0),
    );
    let placed = tree.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(0, 0, 60, 30)],
        "the height cap shrinks the width to keep the ratio"
    );
}

#[test]
fn an_absolute_group_shapes_an_unpositioned_item_but_a_position_wins() {
    let unpositioned = Group::absolute().push(Item::leaf(0).aspect(2.0));
    let placed = unpositioned.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(0, 0, 20, 10)],
        "no position: the natural size follows the ratio"
    );

    let positioned = Group::absolute().push(
        Item::leaf(0)
            .at(dip(10.0), dip(10.0), dip(80.0), dip(80.0))
            .aspect(2.0),
    );
    let placed = positioned.compute(Rect::new(0, 0, 200, 100), 96, &leaf(&[], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(10, 10, 90, 90)],
        "at wins over the ratio"
    );
}

#[test]
fn a_ratio_that_is_not_finite_and_positive_is_ignored() {
    let tree = Group::row()
        .push(Item::leaf(0).aspect(0.0).align(Align::Start))
        .push(Item::leaf(1).aspect(f32::NAN).align(Align::Start))
        .push(Item::leaf(2).aspect(f32::NEG_INFINITY).align(Align::Start));
    let placed = tree.compute(
        Rect::new(0, 0, 200, 100),
        96,
        &leaf(&[(0, 30, 20), (1, 40, 10), (2, 10, 10)], &[]),
    );
    assert_eq!(
        rects(&placed),
        [
            Rect::new(0, 0, 30, 20),
            Rect::new(30, 0, 70, 10),
            Rect::new(70, 0, 80, 10)
        ]
    );
}

#[test]
fn an_extreme_ratio_saturates_without_panicking() {
    let tree = Group::row()
        .push(Item::leaf(0).aspect(1.0e-30).align(Align::Start))
        .push(Item::leaf(1).aspect(1.0e30).align(Align::Start))
        .push(Item::leaf(2).aspect(f32::INFINITY).align(Align::Start));
    let placed = tree.compute(
        Rect::new(0, 0, 200, 100),
        96,
        &leaf(&[(0, 10, 10), (1, 10, 10), (2, 10, 10)], &[]),
    );
    assert_eq!(placed.len(), 3);
    for (key, rect) in &placed {
        assert!(
            rect.left >= 0 && rect.top >= 0 && rect.right <= 200 && rect.bottom <= 100,
            "item {key} at {rect:?} escapes the area"
        );
        assert!(
            rect.width() >= 0 && rect.height() >= 0,
            "item {key}: {rect:?}"
        );
    }
    assert_eq!(
        placed[2].1,
        Rect::new(10, 0, 20, 10),
        "an infinite ratio is ignored: {placed:?}"
    );
}

#[test]
fn an_extreme_ratio_saturates_an_unbounded_measure() {
    // A ratio past the arithmetic saturates the measured extent; the sums that
    // add a container's margins must not overflow (a form's ratio is only
    // bounded as finite and positive).
    let margins = Insets::new(dip(4.0), dip(4.0), dip(4.0), dip(4.0));

    let row = Group::row().margins(margins).push(
        Item::leaf(0)
            .sized(Sizing::Width(dip(10.0)))
            .aspect(1.0e-30),
    );
    assert_eq!(
        row.preferred_size(96, &leaf(&[(0, 10, 10)], &[])),
        Size::new(8, i32::MAX),
        "a tiny ratio saturates the cross extent"
    );

    let layered = Group::layered().margins(margins).push(
        Item::leaf(0)
            .sized(Sizing::Height(dip(10.0)))
            .aspect(1.0e30),
    );
    assert_eq!(
        layered.preferred_size(96, &leaf(&[(0, 10, 10)], &[])),
        Size::new(i32::MAX, 8),
        "a huge ratio saturates the main extent"
    );
}
