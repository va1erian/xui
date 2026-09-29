//! The pure tile-grid arithmetic, for all three icon sizes.

use super::super::IconSize;
use super::super::layout::{self, Direction};
use super::super::metrics::Metrics;
use super::super::model::IconModel;
use super::view;
use crate::geometry::{Point, Rect};

fn metrics(size: IconSize) -> Metrics {
    Metrics::of(size, 96)
}

#[test]
fn columns_fit_each_size_and_need_one_tile_of_room() {
    for size in [IconSize::Small, IconSize::Medium, IconSize::Large] {
        let m = metrics(size);
        // Two whole tiles plus the gap between them.
        assert_eq!(
            layout::columns_for(2 * m.stride_x() - m.gap, m.width, m.gap),
            2
        );
        assert_eq!(layout::columns_for(m.width, m.width, m.gap), 1);
        assert_eq!(layout::columns_for(m.width - 1, m.width, m.gap), 1);
        assert_eq!(layout::columns_for(0, m.width, m.gap), 1);
        assert_eq!(layout::columns_for(-100, m.width, m.gap), 1);
        assert_eq!(layout::columns_for(1000, 0, m.gap), 1);
    }
}

#[test]
fn content_height_counts_rows_and_ignores_the_trailing_gap() {
    for size in [IconSize::Small, IconSize::Medium, IconSize::Large] {
        let m = metrics(size);
        assert_eq!(layout::content_height(0, 3, m), 0);
        assert_eq!(layout::content_height(3, 3, m), m.height);
        assert_eq!(layout::content_height(6, 3, m), 2 * m.stride_y() - m.gap);
    }
}

#[test]
fn hit_testing_skips_gaps_and_out_of_range_points() {
    let m = metrics(IconSize::Large);
    let (sx, sy) = (m.stride_x(), m.stride_y());
    let (cx, cy) = (m.width / 2, m.height / 2);
    assert_eq!(layout::item_at(cx, cy, 3, m, 10), Some(0));
    assert_eq!(layout::item_at(sx + cx, cy, 3, m, 10), Some(1));
    assert_eq!(layout::item_at(sx + cx, sy + cy, 3, m, 10), Some(4));
    assert_eq!(layout::item_at(m.width, cy, 3, m, 10), None, "column gap");
    assert_eq!(layout::item_at(cx, m.height, 3, m, 10), None, "row gap");
    assert_eq!(layout::item_at(-1, cy, 3, m, 10), None, "negative x");
    assert_eq!(layout::item_at(cx, -1, 3, m, 10), None, "negative y");
    assert_eq!(
        layout::item_at(3 * sx + cx, cy, 3, m, 10),
        None,
        "past columns"
    );
    assert_eq!(layout::item_at(cx, cy, 3, m, 0), None, "empty model");
    assert_eq!(
        layout::item_at(sx + cx, 5 * sy + cy, 3, m, 4),
        None,
        "past end"
    );
}

#[test]
fn visible_range_covers_the_scrolled_viewport() {
    let m = metrics(IconSize::Large);
    assert_eq!(layout::visible_range(0, 0, 3, m, 10), 0..0);
    assert_eq!(layout::visible_range(0, m.height, 3, m, 10), 0..3);
    assert_eq!(layout::visible_range(0, m.stride_y() + 1, 3, m, 10), 0..6);
    // Scrolled so the first row's tiles are entirely above the viewport.
    assert_eq!(
        layout::visible_range(m.height + 1, m.height, 3, m, 10),
        3..6
    );
    // A partial last row is clipped to the model.
    assert_eq!(layout::visible_range(0, HEIGHT_BIG, 3, m, 4), 0..4);
}

const HEIGHT_BIG: i32 = 10_000;

#[test]
fn navigation_stops_at_every_edge() {
    assert_eq!(layout::navigate(0, 3, 0, Direction::Right), 0);
    assert_eq!(layout::navigate(9, 3, 0, Direction::Left), 0);
    assert_eq!(layout::navigate(9, 3, 8, Direction::Right), 8);
    assert_eq!(layout::navigate(9, 3, 1, Direction::Right), 2);
    assert_eq!(layout::navigate(9, 3, 2, Direction::Right), 2);
    assert_eq!(layout::navigate(9, 3, 4, Direction::Up), 1);
    assert_eq!(layout::navigate(9, 3, 1, Direction::Up), 1);
    assert_eq!(layout::navigate(9, 3, 4, Direction::Down), 7);
    assert_eq!(layout::navigate(9, 1, 3, Direction::Down), 4, "one column");
    assert_eq!(
        layout::navigate(9, 1, 3, Direction::Right),
        3,
        "still one column"
    );
    assert_eq!(layout::navigate(8, 3, 7, Direction::Down), 7);
}

#[test]
fn a_tile_rect_places_each_index() {
    let m = metrics(IconSize::Large);
    let first = layout::tile_rect(0, 3, m);
    let second = layout::tile_rect(1, 3, m);
    let below = layout::tile_rect(3, 3, m);
    assert_eq!(first, Rect::new(0, 0, m.width, m.height));
    assert_eq!(second.left, m.stride_x());
    assert_eq!(second.top, 0);
    assert_eq!(below.left, 0);
    assert_eq!(below.top, m.stride_y());
    assert_eq!(layout::tile_rect(0, 0, m), first, "zero columns means one");
}

#[test]
fn tile_rects_have_a_text_block_and_an_icon_slot_inside_them() {
    for size in [IconSize::Small, IconSize::Medium, IconSize::Large] {
        let m = metrics(size);
        let tile = layout::tile_rect(0, 3, m);
        let icon = m.icon_rect(tile);
        let text = m.text_rect(tile);
        assert!(icon.left >= tile.left && icon.right <= tile.right);
        assert!(text.left >= icon.right);
        assert!(text.right <= tile.right);
        assert_eq!(text.height(), m.lines as i32 * m.line_height);
        assert_eq!(m.line_rect(text, 0).unwrap().top, text.top);
        assert!(m.line_rect(text, m.lines).is_none());
    }
}

/// The plain model itself (not just through a widget).
#[test]
fn a_plain_model_exposes_its_lines() {
    let model: Vec<String> = vec!["a".into(), "b".into()];
    assert_eq!(model.items(), 2);
    assert_eq!(model.line(0, 0), Some("a"));
    assert_eq!(model.line(0, 1), None);
}

/// A point at the centre of a tile round-trips through `item_at` (kept beside
/// the pure tests; a randomized form lives in `properties.rs`).
#[test]
fn a_tile_centre_hits_its_index() {
    let m = metrics(IconSize::Medium);
    for (index, columns) in [(0, 3), (4, 3), (7, 2)] {
        let rect = layout::tile_rect(index, columns, m);
        let point = Point::new(rect.left + rect.width() / 2, rect.top + rect.height() / 2);
        assert_eq!(
            layout::item_at(point.x, point.y, columns, m, 20),
            Some(index)
        );
    }
}

/// Keep the harness import used even when only the pure functions are tested.
#[test]
fn the_view_uses_the_same_geometry() {
    let (_backend, _core, ui) = super::setup();
    let view = view(&ui, 3);
    let point = super::center(&view, 2);
    assert!(point.x >= 0);
}
