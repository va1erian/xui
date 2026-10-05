//! Wraps, layered and absolute groups, and the trace.

use super::{leaf, rects};
use crate::geometry::{Rect, Size};
use crate::layout::tree::{Align, Group, GroupKind, Item, Sizing, TraceNode, Warning};
use crate::layout::{Anchor, Insets};
use crate::units::dip;

#[test]
fn a_wrap_breaks_lines_when_the_next_item_does_not_fit() {
    let tree = Group::wrap()
        .spacing(dip(10.0))
        .push(Item::leaf(0))
        .push(Item::leaf(1))
        .push(Item::leaf(2));
    let natural = leaf(&[(0, 40, 20), (1, 40, 30), (2, 40, 10)], &[]);
    let placed = tree.compute(Rect::new(0, 0, 100, 100), 96, &natural);
    assert_eq!(
        rects(&placed),
        [
            Rect::new(0, 0, 40, 30),
            Rect::new(50, 0, 90, 30),
            Rect::new(0, 40, 40, 50),
        ],
        "two fit on the first line, which is as tall as its tallest"
    );
}

#[test]
fn a_wrap_measures_one_line_unbounded_and_its_lines_at_a_width() {
    let tree = Group::wrap()
        .spacing(dip(10.0))
        .margins(Insets::all(dip(5.0)))
        .push(Item::leaf(0))
        .push(Item::leaf(1))
        .push(Item::leaf(2));
    let natural = leaf(&[(0, 40, 20), (1, 40, 30), (2, 40, 10)], &[]);
    assert_eq!(tree.preferred_size(96, &natural), Size::new(150, 40));
    let bounded = crate::layout::Constraints::unbounded(96).with_width(110);
    assert_eq!(tree.measure(bounded, &natural), Size::new(100, 60));
}

#[test]
fn a_wrap_justifies_and_aligns_each_line() {
    let tree = Group::wrap()
        .justify(Align::End)
        .align(Align::End)
        .push(Item::leaf(0))
        .push(Item::leaf(1));
    let natural = leaf(&[(0, 30, 20), (1, 30, 10)], &[]);
    let placed = tree.compute(Rect::new(0, 0, 100, 50), 96, &natural);
    assert_eq!(
        rects(&placed),
        [Rect::new(40, 0, 70, 20), Rect::new(70, 10, 100, 20)]
    );
}

#[test]
fn a_layered_group_stacks_items_over_its_whole_area() {
    let tree = Group::layered()
        .push(Item::leaf(0))
        .push(Item::leaf(1).align(Align::Center))
        .push(Item::leaf(2).align(Align::End));
    let natural = leaf(&[(0, 10, 10), (1, 20, 10), (2, 30, 20)], &[]);
    let placed = tree.compute(Rect::new(0, 0, 100, 60), 96, &natural);
    assert_eq!(
        rects(&placed),
        [
            Rect::new(0, 0, 100, 60),
            Rect::new(40, 25, 60, 35),
            Rect::new(70, 40, 100, 60),
        ]
    );
    assert_eq!(tree.preferred_size(96, &natural), Size::new(30, 20));
}

#[test]
fn an_absolute_group_places_items_where_asked_and_follows_their_anchors() {
    let tree = Group::absolute()
        .design_size(dip(200.0), dip(100.0))
        .push(Item::leaf(0).at(dip(10.0), dip(10.0), dip(50.0), dip(20.0)))
        .push(
            Item::leaf(1)
                .at(dip(120.0), dip(70.0), dip(70.0), dip(20.0))
                .anchor(Anchor::BottomRight),
        )
        .push(
            Item::leaf(2)
                .at(dip(10.0), dip(40.0), dip(180.0), dip(20.0))
                .anchor(Anchor::StretchHorizontal),
        );
    let natural = leaf(&[], &[]);
    let at_design = tree.compute(Rect::new(0, 0, 200, 100), 96, &natural);
    assert_eq!(at_design[1].1, Rect::new(120, 70, 190, 90));

    let grown = tree.compute(Rect::new(0, 0, 300, 150), 96, &natural);
    assert_eq!(
        rects(&grown),
        [
            Rect::new(10, 10, 60, 30),
            Rect::new(220, 120, 290, 140),
            Rect::new(10, 40, 290, 60),
        ]
    );
    assert_eq!(tree.preferred_size(96, &natural), Size::new(200, 100));
}

#[test]
fn an_absolute_group_without_a_design_size_holds_its_items() {
    let tree = Group::absolute()
        .push(Item::leaf(0).at(dip(10.0), dip(10.0), dip(50.0), dip(20.0)))
        .push(Item::leaf(1));
    let natural = leaf(&[(1, 30, 40)], &[]);
    assert_eq!(tree.preferred_size(96, &natural), Size::new(60, 40));
    let placed = tree.compute(Rect::new(5, 5, 105, 105), 96, &natural);
    assert_eq!(
        placed[1].1,
        Rect::new(5, 5, 35, 45),
        "unpositioned: top-left"
    );
}

#[test]
fn trace_lists_every_node_with_its_depth() {
    let inner = Group::row().push(Item::leaf(1).sized(Sizing::Fill(1)));
    let tree = Group::column().push(Item::leaf(0)).push(Item::group(inner));
    let trace = tree.trace(
        Rect::new(0, 0, 100, 60),
        96,
        &leaf(&[(0, 10, 20), (1, 0, 15)], &[]),
    );
    let nodes: Vec<(usize, TraceNode<u32>)> = trace.iter().map(|t| (t.depth, t.node)).collect();
    assert_eq!(
        nodes,
        [
            (0, TraceNode::Group(GroupKind::Column)),
            (1, TraceNode::Leaf(0)),
            (1, TraceNode::Group(GroupKind::Row)),
            (2, TraceNode::Leaf(1)),
        ]
    );
    assert!(trace.iter().all(|t| t.warnings.is_empty()));
}

#[test]
fn trace_warns_of_zero_size_and_overflow() {
    // Two fixed rows that cannot both fit: the stack shrinks them, and a leaf
    // with no natural size is zero-sized.
    let tree = Group::column()
        .push(Item::leaf(0).sized(Sizing::Fixed(dip(80.0))))
        .push(Item::leaf(1));
    let trace = tree.trace(Rect::new(0, 0, 100, 60), 96, &leaf(&[(0, 10, 10)], &[]));
    assert_eq!(trace[2].warnings, [Warning::ZeroSize]);
}

#[test]
fn trace_reports_overlap_only_inside_disjoint_groups() {
    let layered = Group::layered().push(Item::leaf(0)).push(Item::leaf(1));
    let trace = layered.trace(Rect::new(0, 0, 50, 50), 96, &leaf(&[], &[]));
    assert!(
        trace
            .iter()
            .all(|t| !t.warnings.iter().any(|w| matches!(w, Warning::Overlaps(_))))
    );

    // A framed leaf's content is checked against the frame, not its siblings.
    let framed = Group::column().push(Item::framed(
        0,
        Group::absolute().push(Item::leaf(1).at(dip(0.0), dip(0.0), dip(10.0), dip(10.0))),
    ));
    let trace = framed.trace(Rect::new(0, 0, 50, 50), 96, &leaf(&[(0, 50, 5)], &[]));
    assert!(trace.iter().all(|t| t.warnings.is_empty()), "{trace:?}");
}

#[test]
fn a_grid_item_aligns_each_axis_on_its_own() {
    use crate::layout::tree::Track;
    let tree = Group::grid(vec![Track::Fill(1)])
        .push(Item::leaf(0).align_x(Align::Start).align_y(Align::Center));
    let placed = tree.compute(Rect::new(0, 0, 100, 40), 96, &leaf(&[(0, 30, 10)], &[]));
    assert_eq!(placed[0].1.left, 0);
    let tall = Group::grid(vec![Track::Fill(1)]).push(
        Item::leaf(0)
            .sized(Sizing::Fill(1))
            .align_x(Align::Start)
            .align_y(Align::Center),
    );
    let placed = tall.compute(Rect::new(0, 0, 100, 40), 96, &leaf(&[(0, 30, 10)], &[]));
    assert_eq!(placed[0].1, Rect::new(0, 15, 30, 25));
}

#[test]
fn an_item_takes_an_exact_width_and_height_at_once() {
    let tree = Group::column().push(Item::leaf(0).size(Some(dip(40.0)), Some(dip(20.0))));
    let placed = tree.compute(Rect::new(0, 0, 100, 100), 96, &leaf(&[(0, 5, 5)], &[]));
    assert_eq!(placed[0].1, Rect::new(0, 0, 40, 20), "not stretched across");
    assert_eq!(
        tree.preferred_size(96, &leaf(&[(0, 5, 5)], &[])),
        Size::new(40, 20)
    );
}
