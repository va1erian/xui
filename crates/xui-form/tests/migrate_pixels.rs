//! A migrated form lands on the pixels its format-1 rectangles named: a grid
//! is only built where it reproduces them, and everything else stays
//! absolute. Each test lays the migrated form out offscreen and compares the
//! widgets' bounds with the v1 rectangles.

mod common;

use common::with_form;
use xui_core::geometry::Rect;
use xui_form::migrate::from_v1;
use xui_form::model::{Node, Track};
use xui_form::{BuildOptions, Form};

/// A format-1 form `width` by `height` holding `nodes`.
fn v1(width: i32, height: i32, nodes: &[String]) -> String {
    let mut text =
        format!("format = 1\n[window]\nname = \"f\"\nwidth = {width}\nheight = {height}\n");
    for node in nodes {
        text.push_str(node);
    }
    text
}

/// A format-1 node of `kind` named `name` at `(left, top, width, height)`.
fn node(
    kind: &str,
    name: &str,
    parent: Option<&str>,
    (left, top, width, height): (i32, i32, i32, i32),
    extra: &str,
) -> String {
    let parent = parent.map_or_else(String::new, |parent| format!("parent = \"{parent}\"\n"));
    format!(
        "[[node]]\nkind = \"{kind}\"\nname = \"{name}\"\n{parent}left = {left}\ntop = {top}\n\
         width = {width}\nheight = {height}\n{extra}"
    )
}

fn button(name: &str, parent: Option<&str>, rect: (i32, i32, i32, i32)) -> String {
    node("Button", name, parent, rect, "")
}

/// Converts `text` and lays the form out, returning it with the bounds of
/// each of `names`.
fn laid_out(text: &str, names: &[&str]) -> (Form, Vec<Rect>) {
    let form = from_v1(text).expect("the form converts").form;
    let bounds = with_form(&form, BuildOptions::default(), |live| {
        names
            .iter()
            .map(|name| live.bounds(name).expect("the widget exists"))
            .collect()
    });
    (form, bounds)
}

/// The rectangle `(left, top, width, height)` as pixel bounds.
fn px((left, top, width, height): (i32, i32, i32, i32)) -> Rect {
    Rect::new(left, top, left + width, top + height)
}

#[test]
fn a_grid_that_would_be_squeezed_stays_absolute() {
    // 72 + 8 + 56 fills the whole width, so a grid padded by 8 on each side
    // had squeezed the columns to 67 and 53.
    let (a, b) = ((8, 8, 72, 24), (88, 8, 56, 24));
    let text = v1(144, 40, &[button("a", None, a), button("b", None, b)]);
    let (form, bounds) = laid_out(&text, &["a", "b"]);
    assert!(matches!(form.root, Node::Absolute(_)), "{:?}", form.root);
    assert_eq!(bounds, [px(a), px(b)]);
}

#[test]
fn an_asymmetric_margin_that_fits_is_a_grid_on_the_same_pixels() {
    // The same buttons with room on the right: fixed columns, no stretching.
    let (a, b) = ((8, 8, 72, 24), (88, 8, 56, 24));
    let text = v1(160, 40, &[button("a", None, a), button("b", None, b)]);
    let (form, bounds) = laid_out(&text, &["a", "b"]);
    let Node::Grid(grid) = &form.root else {
        panic!("a grid that fits is a grid: {:?}", form.root);
    };
    assert_eq!(
        grid.columns,
        [Track::Fixed(72.0.into()), Track::Fixed(56.0.into())]
    );
    assert_eq!(bounds, [px(a), px(b)]);
}

#[test]
fn a_grid_must_also_leave_room_below() {
    let (a, b) = ((8, 8, 50, 20), (8, 36, 50, 20));
    // The last row ends at 56: a window of 60 leaves 4, less than the padding.
    let (form, bounds) = laid_out(
        &v1(100, 60, &[button("a", None, a), button("b", None, b)]),
        &["a", "b"],
    );
    assert!(matches!(form.root, Node::Absolute(_)), "{:?}", form.root);
    assert_eq!(bounds, [px(a), px(b)]);

    let (form, bounds) = laid_out(
        &v1(100, 64, &[button("a", None, a), button("b", None, b)]),
        &["a", "b"],
    );
    assert!(matches!(form.root, Node::Grid(_)), "{:?}", form.root);
    assert_eq!(bounds, [px(a), px(b)]);
}

#[test]
fn the_content_of_a_group_stays_absolute_and_keeps_its_sizes() {
    // Two 64-wide buttons 8 apart in a 160-wide group box: read against the
    // box's width they fit a grid padded by 8, but the box insets its content
    // by its frame, so that grid squeezed the buttons to 60.
    let (a, b) = ((8, 8, 64, 24), (80, 8, 64, 24));
    let text = v1(
        200,
        120,
        &[
            node(
                "GroupBox",
                "frame",
                None,
                (16, 16, 160, 80),
                "text = \"Options\"
",
            ),
            button("a", Some("frame"), a),
            button("b", Some("frame"), b),
        ],
    );
    let (form, bounds) = laid_out(&text, &["frame", "a", "b"]);
    let Node::Absolute(root) = &form.root else {
        panic!("one node, absolute: {:?}", form.root);
    };
    let Node::Group(group) = &root.children[0] else {
        panic!("the group box becomes a group");
    };
    assert!(
        matches!(&*group.content, Node::Absolute(_)),
        "{:?}",
        group.content
    );
    let (frame, first, second) = (bounds[0], bounds[1], bounds[2]);
    assert_eq!(frame, px((16, 16, 160, 80)));
    // The designed sizes and the designed offset between them survive (the
    // group's own inset moves both the same).
    assert_eq!((first.width(), first.height()), (64, 24));
    assert_eq!((second.width(), second.height()), (64, 24));
    assert_eq!(second.left - first.left, b.0 - a.0);
    assert_eq!(second.top, first.top);
}

#[test]
fn a_plain_panel_has_no_inset_so_its_grid_lands_on_the_v1_pixels() {
    let (a, b) = ((8, 8, 64, 24), (80, 8, 64, 24));
    let text = v1(
        200,
        100,
        &[
            node("Panel", "pane", None, (10, 10, 160, 60), ""),
            button("a", Some("pane"), a),
            button("b", Some("pane"), b),
        ],
    );
    let (form, bounds) = laid_out(&text, &["pane", "a", "b"]);
    let Node::Absolute(root) = &form.root else {
        panic!("one node, absolute: {:?}", form.root);
    };
    let Node::Panel(pane) = &root.children[0] else {
        panic!("a panel stays a panel");
    };
    assert!(
        matches!(&*pane.content, Node::Grid(_)),
        "{:?}",
        pane.content
    );
    // `bounds` are relative to the parent, and so are format-1 positions.
    assert_eq!(bounds[0], px((10, 10, 160, 60)));
    assert_eq!(bounds[1], px(a));
    assert_eq!(bounds[2], px(b));
}

#[test]
fn a_card_panel_gives_its_content_the_whole_rectangle_like_a_plain_one() {
    // A card only draws behind its content: the content is laid out in the
    // panel's own rectangle (`--report` says "layout at 0,0 size 160x60"), so
    // a grid designed against the panel's size lands on the v1 pixels.
    let (a, b) = ((8, 8, 64, 24), (80, 8, 64, 24));
    let text = v1(
        200,
        100,
        &[
            node("Panel", "pane", None, (10, 10, 160, 60), "card = true\n"),
            button("a", Some("pane"), a),
            button("b", Some("pane"), b),
        ],
    );
    let (form, bounds) = laid_out(&text, &["pane", "a", "b"]);
    let Node::Absolute(root) = &form.root else {
        panic!("one node, absolute: {:?}", form.root);
    };
    let Node::Panel(pane) = &root.children[0] else {
        panic!("a panel stays a panel");
    };
    assert!(pane.card, "the card survives the conversion");
    assert!(
        matches!(&*pane.content, Node::Grid(_)),
        "{:?}",
        pane.content
    );
    assert_eq!(bounds[0], px((10, 10, 160, 60)));
    assert_eq!(bounds[1], px(a));
    assert_eq!(bounds[2], px(b));
}
