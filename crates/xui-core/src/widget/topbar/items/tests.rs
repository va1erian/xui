#![forbid(unsafe_code)]

use super::*;

fn item(kind: Kind) -> Item {
    Item {
        id: TopBarId::new(0),
        enabled: true,
        tooltip: None,
        width: None,
        kind,
    }
}

fn with_width(mut item: Item, width: Width) -> Item {
    item.width = Some(width);
    item
}

fn rects(items: &[Item], bounds: Rect, dpi: u32) -> Vec<Rect> {
    let mut out = Vec::new();
    each_rect(items, bounds, dpi, |_, rect| out.push(rect));
    out
}

#[test]
fn fixed_items_keep_their_width_and_ignore_surplus() {
    let items = [
        item(Kind::Icon(Glyph::Menu)),
        item(Kind::Label("ab".into())),
    ];
    let bounds = Rect::new(0, 0, 200, 24);
    let cells = rects(&items, bounds, 96);
    assert_eq!(cells[0], Rect::new(0, 0, 36, 24));
    assert_eq!(cells[1].left, 36, "the label starts after the icon");
    assert_eq!(cells[1].width(), 7 * 2 + 12);
}

#[test]
fn spacers_share_the_leftover_width() {
    let items = [
        item(Kind::Icon(Glyph::Menu)),
        item(Kind::Spacer(1)),
        item(Kind::Icon(Glyph::Close)),
        item(Kind::Spacer(1)),
    ];
    let bounds = Rect::new(0, 0, 200, 24);
    let cells = rects(&items, bounds, 96);
    assert_eq!(cells[0], Rect::new(0, 0, 36, 24));
    assert_eq!(cells[1], Rect::new(36, 0, 100, 24));
    assert_eq!(cells[2], Rect::new(100, 0, 136, 24));
    assert_eq!(cells[3], Rect::new(136, 0, 200, 24));
}

#[test]
fn weighted_spacers_split_proportionally() {
    let items = [
        item(Kind::Spacer(1)),
        item(Kind::Icon(Glyph::Menu)),
        item(Kind::Spacer(3)),
    ];
    let bounds = Rect::new(0, 0, 136, 24);
    let cells = rects(&items, bounds, 96);
    assert_eq!(cells[0].width(), 25);
    assert_eq!(cells[2].width(), 75);
    assert_eq!(cells[0].left + cells[0].width(), 25);
    assert_eq!(cells[2].right, 136);
}

#[test]
fn hit_testing_skips_labels_and_spacers() {
    let items = [
        item(Kind::Spacer(1)),
        item(Kind::Label("hi".into())),
        item(Kind::Icon(Glyph::Search)),
    ];
    let bounds = Rect::new(0, 0, 120, 24);
    let label = item_rect(&items, bounds, 96, 1).unwrap();
    assert_eq!(item_at(&items, bounds, 96, label.left + 1), Some(1));
    assert_eq!(hit_interactive(&items, bounds, 96, label.left + 1), None);
    let icon = item_rect(&items, bounds, 96, 2).unwrap();
    assert_eq!(hit_interactive(&items, bounds, 96, icon.left + 1), Some(2));
}

#[test]
fn a_fixed_width_overrides_the_natural_width() {
    let items = [
        item(Kind::Icon(Glyph::Menu)),
        with_width(
            item(Kind::Slider {
                min: 0.0,
                max: 1.0,
                value: 0.0,
            }),
            Width::Fixed(Dip(60.0)),
        ),
    ];
    let cells = rects(&items, Rect::new(0, 0, 200, 24), 96);
    assert_eq!(cells[1], Rect::new(36, 0, 96, 24), "the slider is 60 wide");
}

#[test]
fn an_expanding_item_absorbs_the_leftover_width() {
    let items = [
        item(Kind::Icon(Glyph::Previous)),
        with_width(
            item(Kind::Slider {
                min: 0.0,
                max: 1.0,
                value: 0.0,
            }),
            Width::Expand(1),
        ),
        item(Kind::Icon(Glyph::Next)),
    ];
    let cells = rects(&items, Rect::new(0, 0, 200, 24), 96);
    assert_eq!(cells[0], Rect::new(0, 0, 36, 24));
    assert_eq!(
        cells[1],
        Rect::new(36, 0, 164, 24),
        "the expanding slider fills the middle"
    );
    assert_eq!(cells[2], Rect::new(164, 0, 200, 24));
}

#[test]
fn expanding_items_share_the_leftover_by_weight() {
    let items = [
        with_width(
            item(Kind::Slider {
                min: 0.0,
                max: 1.0,
                value: 0.0,
            }),
            Width::Expand(1),
        ),
        with_width(
            item(Kind::Slider {
                min: 0.0,
                max: 1.0,
                value: 0.0,
            }),
            Width::Expand(3),
        ),
    ];
    let cells = rects(&items, Rect::new(0, 0, 200, 24), 96);
    assert_eq!(cells[0].width(), 50);
    assert_eq!(cells[1].width(), 150);
    assert_eq!(cells[1].right, 200);
}
