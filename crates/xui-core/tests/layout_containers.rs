//! Property tests for the layout tree's containers: whatever the items ask
//! for, a container never places one outside its own area; a disjoint
//! container (row, column, grid, wrap) never overlaps two of them; and a
//! layered container keeps them in tree order, the later above the earlier.

use proptest::prelude::*;

use xui_core::geometry::{Rect, Size};
use xui_core::layout::{
    Align, Anchor, Constraints, Group, Insets, Item, Leaf, Sizing, Track, Warning,
};
use xui_core::units::dip;

/// One item's natural size, sizing and alignment.
#[derive(Clone, Debug)]
struct Spec {
    natural: (i32, i32),
    sizing: Sizing,
    align: Option<Align>,
    at: (f32, f32, f32, f32),
    anchor: Anchor,
}

fn align_strategy() -> impl Strategy<Value = Option<Align>> {
    prop_oneof![
        Just(None),
        Just(Some(Align::Start)),
        Just(Some(Align::Center)),
        Just(Some(Align::End)),
        Just(Some(Align::Stretch)),
    ]
}

fn sizing_strategy() -> impl Strategy<Value = Sizing> {
    prop_oneof![
        Just(Sizing::Auto),
        (0.0f32..200.0).prop_map(|v| Sizing::Fixed(dip(v))),
        (0.0f32..200.0).prop_map(|v| Sizing::Min(dip(v))),
        (0u32..4).prop_map(Sizing::Fill),
        (0.0f32..200.0).prop_map(|v| Sizing::Width(dip(v))),
        (0.0f32..200.0).prop_map(|v| Sizing::Height(dip(v))),
    ]
}

fn anchor_strategy() -> impl Strategy<Value = Anchor> {
    prop_oneof![
        Just(Anchor::TopLeft),
        Just(Anchor::Center),
        Just(Anchor::BottomRight),
        Just(Anchor::StretchHorizontal),
        Just(Anchor::StretchVertical),
        Just(Anchor::Fill),
    ]
}

fn spec_strategy() -> impl Strategy<Value = Spec> {
    (
        (0..300i32, 0..200i32),
        sizing_strategy(),
        align_strategy(),
        (
            -50.0f32..400.0,
            -50.0f32..300.0,
            0.0f32..300.0,
            0.0f32..200.0,
        ),
        anchor_strategy(),
    )
        .prop_map(|(natural, sizing, align, at, anchor)| Spec {
            natural,
            sizing,
            align,
            at,
            anchor,
        })
}

fn area_strategy() -> impl Strategy<Value = Rect> {
    (-100..100i32, -100..100i32, 0..900i32, 0..700i32)
        .prop_map(|(left, top, width, height)| Rect::new(left, top, left + width, top + height))
}

fn dpi_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![Just(96u32), Just(144), Just(192)]
}

/// The group with every spec pushed as a leaf keyed by its index.
fn fill(group: Group<usize>, specs: &[Spec], gap: f32, margin: f32) -> Group<usize> {
    specs.iter().enumerate().fold(
        group.spacing(dip(gap)).margins(Insets::all(dip(margin))),
        |group, (key, spec)| {
            let (x, y, w, h) = spec.at;
            let mut item = Item::leaf(key)
                .sized(spec.sizing)
                .at(dip(x), dip(y), dip(w), dip(h))
                .anchor(spec.anchor);
            if let Some(align) = spec.align {
                item = item.align(align);
            }
            group.push(item)
        },
    )
}

/// The leaves' rectangles, each checked to lie inside `area`.
fn placed(group: &Group<usize>, specs: &[Spec], area: Rect, dpi: u32) -> Vec<(usize, Rect)> {
    let leaf = |key: &usize, _: Constraints| {
        let (width, height) = specs[*key].natural;
        Leaf::new(Size::new(width, height))
    };
    group.compute(area, dpi, &leaf)
}

fn inside(rect: Rect, area: Rect) -> bool {
    rect.left >= area.left
        && rect.top >= area.top
        && rect.right <= area.right
        && rect.bottom <= area.bottom
        && rect.width() >= 0
        && rect.height() >= 0
}

fn overlap(a: Rect, b: Rect) -> bool {
    a.left.max(b.left) < a.right.min(b.right) && a.top.max(b.top) < a.bottom.min(b.bottom)
}

/// Asserts every leaf is inside `area` and, for a disjoint group, that no two
/// overlap; the trace must agree.
fn check(group: Group<usize>, specs: &[Spec], area: Rect, dpi: u32, disjoint: bool) {
    let rects = placed(&group, specs, area, dpi);
    assert_eq!(rects.len(), specs.len(), "every item is visible");
    for (key, rect) in &rects {
        assert!(
            inside(*rect, area),
            "item {key} at {rect:?} escapes {area:?}"
        );
    }
    if disjoint {
        for (n, (a, ra)) in rects.iter().enumerate() {
            for (b, rb) in &rects[n + 1..] {
                assert!(
                    !overlap(*ra, *rb),
                    "items {a} {ra:?} and {b} {rb:?} overlap"
                );
            }
        }
    }
    let leaf = |key: &usize, _: Constraints| {
        let (width, height) = specs[*key].natural;
        Leaf::new(Size::new(width, height))
    };
    let trace = group.trace(area, dpi, &leaf);
    assert!(
        trace.iter().all(|node| node
            .warnings
            .iter()
            .all(|w| !matches!(w, Warning::Outside | Warning::Overlaps(_)))),
        "the trace reports no escape or overlap"
    );
}

proptest! {
    #[test]
    fn rows_and_columns_keep_items_inside_and_apart(
        specs in prop::collection::vec(spec_strategy(), 0..8),
        area in area_strategy(),
        dpi in dpi_strategy(),
        gap in 0.0f32..20.0,
        margin in 0.0f32..20.0,
        column in any::<bool>(),
    ) {
        let group = if column { Group::column() } else { Group::row() };
        check(fill(group, &specs, gap, margin), &specs, area, dpi, true);
    }

    #[test]
    fn grids_keep_items_inside_and_apart(
        specs in prop::collection::vec(spec_strategy(), 0..10),
        area in area_strategy(),
        dpi in dpi_strategy(),
        gap in 0.0f32..20.0,
        margin in 0.0f32..20.0,
        columns in prop::collection::vec(prop_oneof![
            Just(Track::Auto),
            (0.0f32..150.0).prop_map(|v| Track::Fixed(dip(v))),
            (1u32..3).prop_map(Track::Fill),
        ], 1..4),
    ) {
        check(fill(Group::grid(columns), &specs, gap, margin), &specs, area, dpi, true);
    }

    #[test]
    fn wraps_keep_items_inside_and_apart(
        specs in prop::collection::vec(spec_strategy(), 0..10),
        area in area_strategy(),
        dpi in dpi_strategy(),
        gap in 0.0f32..20.0,
        margin in 0.0f32..20.0,
    ) {
        check(fill(Group::wrap(), &specs, gap, margin), &specs, area, dpi, true);
    }

    #[test]
    fn layered_groups_keep_items_inside_in_tree_order(
        specs in prop::collection::vec(spec_strategy(), 0..8),
        area in area_strategy(),
        dpi in dpi_strategy(),
        margin in 0.0f32..20.0,
    ) {
        let group = fill(Group::layered(), &specs, 0.0, margin);
        check(group.clone(), &specs, area, dpi, false);
        // Z-order is tree order: the later item is placed (and so created and
        // painted) above the earlier.
        let keys: Vec<usize> = placed(&group, &specs, area, dpi).iter().map(|(k, _)| *k).collect();
        prop_assert_eq!(keys, (0..specs.len()).collect::<Vec<_>>());
    }

    #[test]
    fn absolute_groups_keep_items_inside(
        specs in prop::collection::vec(spec_strategy(), 0..8),
        area in area_strategy(),
        dpi in dpi_strategy(),
        margin in 0.0f32..20.0,
        design in prop::option::of((0.0f32..600.0, 0.0f32..500.0)),
    ) {
        let mut group = Group::absolute();
        if let Some((width, height)) = design {
            group = group.design_size(dip(width), dip(height));
        }
        check(fill(group, &specs, 0.0, margin), &specs, area, dpi, false);
    }
}
