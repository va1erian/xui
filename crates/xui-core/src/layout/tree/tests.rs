//! Pure tree-to-rects tests: no backend and no widget, only keyed leaves.

use super::*;
use crate::units::dip;

/// Leaf `n`'s natural size and visibility, from a small table.
fn leaf(
    natural: &'static [(u32, i32, i32)],
    hidden: &'static [u32],
) -> impl Fn(&u32, Constraints) -> Leaf {
    move |key, _| {
        let (width, height) = natural
            .iter()
            .find(|(k, ..)| k == key)
            .map_or((0, 0), |&(_, w, h)| (w, h));
        Leaf {
            visible: !hidden.contains(key),
            ..Leaf::new(Size::new(width, height))
        }
    }
}

fn rects(placed: &[(u32, Rect)]) -> Vec<Rect> {
    placed.iter().map(|(_, rect)| *rect).collect()
}

#[test]
fn column_uses_natural_size_and_fills_the_rest() {
    let tree = Group::column()
        .push(Item::leaf(0))
        .push(Item::leaf(1).sized(Sizing::Fill(1)));
    let placed = tree.compute(Rect::new(0, 0, 100, 60), 96, &leaf(&[(0, 0, 20)], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(0, 0, 100, 20), Rect::new(0, 20, 100, 60)]
    );
}

#[test]
fn row_shares_leftover_by_weight() {
    let tree = Group::row()
        .push(Item::leaf(0).sized(Sizing::Fixed(dip(30.0))))
        .push(Item::leaf(1).sized(Sizing::Fill(1)))
        .push(Item::leaf(2).sized(Sizing::Fill(2)));
    let placed = tree.compute(Rect::new(0, 0, 90, 40), 96, &leaf(&[], &[]));
    assert_eq!(
        rects(&placed),
        [
            Rect::new(0, 0, 30, 40),
            Rect::new(30, 0, 50, 40),
            Rect::new(50, 0, 90, 40)
        ]
    );
}

#[test]
fn design_values_scale_with_dpi() {
    let tree = Group::row()
        .push(Item::leaf(0).sized(Sizing::Fixed(dip(30.0))))
        .push(Item::leaf(1).sized(Sizing::Fill(1)));
    let placed = tree.compute(Rect::new(0, 0, 200, 40), 192, &leaf(&[], &[]));
    assert_eq!(placed[0].1, Rect::new(0, 0, 60, 40));
}

#[test]
fn spacing_and_margins_apply() {
    let tree = Group::column()
        .margins(Insets::all(dip(4.0)))
        .spacing(dip(2.0))
        .push(Item::leaf(0).sized(Sizing::Fixed(dip(10.0))))
        .push(Item::leaf(1).sized(Sizing::Fill(1)));
    let placed = tree.compute(Rect::new(0, 0, 100, 100), 96, &leaf(&[], &[]));
    assert_eq!(
        rects(&placed),
        [Rect::new(4, 4, 96, 14), Rect::new(4, 16, 96, 96)]
    );
}

#[test]
fn nested_groups_recurse_and_fill_when_asked() {
    let inner = Group::row()
        .push(Item::leaf(1).sized(Sizing::Fixed(dip(20.0))))
        .push(Item::leaf(2).sized(Sizing::Fill(1)));
    let tree = Group::column()
        .push(Item::leaf(0))
        .push(Item::group(inner).sized(Sizing::Fill(1)));
    let placed = tree.compute(Rect::new(0, 0, 100, 60), 96, &leaf(&[(0, 0, 10)], &[]));
    assert_eq!(
        rects(&placed),
        [
            Rect::new(0, 0, 100, 10),
            Rect::new(0, 10, 20, 60),
            Rect::new(20, 10, 100, 60)
        ]
    );
}

#[test]
fn hidden_leaves_take_no_space() {
    let tree = Group::column()
        .push(Item::leaf(0).sized(Sizing::Fixed(dip(10.0))))
        .push(Item::leaf(1))
        .push(Item::leaf(2).sized(Sizing::Fill(1)));
    let placed = tree.compute(Rect::new(0, 0, 100, 60), 96, &leaf(&[(1, 0, 50)], &[1]));
    assert_eq!(placed.len(), 2, "the hidden leaf must be dropped");
    assert_eq!(placed[0].1, Rect::new(0, 0, 100, 10));
    assert_eq!(placed[1].1, Rect::new(0, 10, 100, 60));
}

#[test]
fn a_group_of_only_hidden_leaves_collapses() {
    let hidden_row = Group::row().push(Item::leaf(0).sized(Sizing::Fill(1)));
    let tree = Group::column()
        .push(Item::group(hidden_row))
        .push(Item::leaf(1).sized(Sizing::Fill(1)));
    let placed = tree.compute(Rect::new(0, 0, 100, 60), 96, &leaf(&[], &[0]));
    assert_eq!(placed, [(1, Rect::new(0, 0, 100, 60))]);
}

#[test]
fn width_and_height_follow_their_axis() {
    // In a row, `Height` constrains the cross axis; the main axis stays natural.
    let row = Group::row()
        .push(Item::leaf(0).sized(Sizing::Height(dip(15.0))))
        .push(Item::leaf(1).sized(Sizing::Fill(1)));
    let placed = row.compute(Rect::new(0, 0, 100, 40), 96, &leaf(&[(0, 30, 0)], &[]));
    assert_eq!(placed[0].1, Rect::new(0, 0, 30, 15));
    assert_eq!(placed[1].1, Rect::new(30, 0, 100, 40));

    // In a column, `Width` constrains the cross axis.
    let column = Group::column()
        .push(Item::leaf(0).sized(Sizing::Width(dip(25.0))))
        .push(Item::leaf(1).sized(Sizing::Fill(1)));
    let placed = column.compute(Rect::new(0, 0, 100, 40), 96, &leaf(&[(0, 0, 10)], &[]));
    assert_eq!(placed[0].1, Rect::new(0, 0, 25, 10));
    assert_eq!(placed[1].1, Rect::new(0, 10, 100, 40));
}

#[test]
fn natural_sizes_are_re_read_on_every_pass() {
    // The callback is asked each time, so a leaf whose content grew is placed
    // at its new size, and shrinking under overflow cannot feed back.
    let tree = Group::column()
        .push(Item::leaf(0))
        .push(Item::leaf(1).sized(Sizing::Fixed(dip(30.0))))
        .push(Item::leaf(2).sized(Sizing::Fixed(dip(30.0))));
    let natural = leaf(&[(0, 0, 100)], &[]);
    let first = tree.compute(Rect::new(0, 0, 50, 80), 96, &natural);
    let second = tree.compute(Rect::new(0, 0, 50, 80), 96, &natural);
    assert_eq!(first, second, "an over-subscribed column is stable");
}

#[test]
fn preferred_size_sums_naturals_margins_and_spacing() {
    let tree = Group::column()
        .margins(Insets::all(dip(4.0)))
        .spacing(dip(2.0))
        .push(Item::leaf(0))
        .push(Item::leaf(1))
        .push(Item::leaf(2).sized(Sizing::Fill(1)));
    let size = tree.preferred_size(96, &leaf(&[(0, 30, 20), (1, 50, 10), (2, 99, 99)], &[]));
    // Heights 20 + 10 + 0 (a fill slot has no natural extent), two gaps of 2,
    // margins 8. Width is the widest natural (a fill leaf still reports its
    // cross extent, 99) plus margins.
    assert_eq!(size, Size::new(107, 42));
}

#[test]
fn preferred_size_honours_fixed_min_and_width() {
    let tree = Group::row()
        .push(Item::leaf(0).sized(Sizing::Fixed(dip(40.0))))
        .push(Item::leaf(1).sized(Sizing::Min(dip(50.0))))
        .push(Item::leaf(2).sized(Sizing::Width(dip(15.0))));
    let size = tree.preferred_size(96, &leaf(&[(0, 5, 10), (1, 20, 12), (2, 99, 8)], &[]));
    assert_eq!(size, Size::new(40 + 50 + 15, 12));
}

#[test]
fn a_nested_group_takes_its_natural_size_by_default() {
    let inner = Group::row()
        .push(Item::leaf(1))
        .push(Item::leaf(2).sized(Sizing::Fill(1)));
    let tree = Group::column().push(Item::group(inner)).push(Item::leaf(0));
    let placed = tree.compute(
        Rect::new(0, 0, 100, 60),
        96,
        &leaf(&[(0, 0, 10), (1, 30, 25), (2, 0, 12)], &[]),
    );
    assert_eq!(
        rects(&placed),
        [
            Rect::new(0, 0, 30, 25),
            Rect::new(30, 0, 100, 25),
            Rect::new(0, 25, 100, 35)
        ]
    );
}

#[test]
fn a_column_measures_its_leaves_at_its_own_width() {
    // Leaf 0 wraps: its height is 1000 / the width it is given.
    let wrapping = |key: &u32, constraints: Constraints| {
        let width = constraints.max_width.unwrap_or(1000);
        let height = if *key == 0 { 1000 / width.max(1) } else { 5 };
        Leaf::new(Size::new(width.min(1000), height))
    };
    let tree = Group::column().push(Item::leaf(0)).push(Item::leaf(1));
    let placed = tree.compute(Rect::new(0, 0, 100, 60), 96, &wrapping);
    assert_eq!(placed[0].1, Rect::new(0, 0, 100, 10));
    assert_eq!(placed[1].1, Rect::new(0, 10, 100, 15));
}

#[test]
fn a_nested_grid_measures_its_fill_column_at_the_width_it_will_get() {
    // Leaf 1 wraps: its height is 1000 / the width it is given.
    let wrapping = |key: &u32, constraints: Constraints| match key {
        1 => {
            let width = constraints.max_width.unwrap_or(1000).max(1);
            Leaf::new(Size::new(width.min(1000), 1000 / width))
        }
        _ => Leaf::new(Size::new(50, 2)),
    };
    let grid = Group::grid(vec![Track::Auto, Track::Fill(1)])
        .push(Item::leaf(0))
        .push(Item::leaf(1));
    assert_eq!(grid.preferred_size(96, &wrapping), Size::new(50, 2));

    let tree = Group::column().push(Item::group(grid)).push(Item::leaf(2));
    let placed = tree.compute(Rect::new(0, 0, 300, 100), 96, &wrapping);
    assert_eq!(placed[1].1, Rect::new(50, 0, 300, 4), "250 wide, so 4 tall");
    assert_eq!(placed[2].1.top, 4, "the column reserved the same height");
}
