//! Property tests for the pure icon-view geometry: a tile's centre always hits
//! that tile, and `visible_range` is exactly the set of tiles intersecting the
//! scrolled viewport.

use proptest::prelude::*;

use super::super::IconSize;
use super::super::layout;
use super::super::metrics::Metrics;
use crate::geometry::Point;

fn size_strategy() -> impl Strategy<Value = IconSize> {
    prop_oneof![
        Just(IconSize::Small),
        Just(IconSize::Medium),
        Just(IconSize::Large),
    ]
}

fn dpi_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![Just(96u32), Just(120), Just(144), Just(192)]
}

fn metrics_strategy() -> impl Strategy<Value = Metrics> {
    (size_strategy(), dpi_strategy()).prop_map(|(size, dpi)| Metrics::of(size, dpi))
}

proptest! {
    #[test]
    fn a_tile_centre_hits_that_tile(
        metrics in metrics_strategy(),
        columns in 1usize..8,
        len in 1usize..2000,
        index in 0usize..2000,
    ) {
        prop_assume!(index < len);
        let rect = layout::tile_rect(index, columns, metrics);
        let point = Point::new(rect.left + rect.width() / 2, rect.top + rect.height() / 2);
        prop_assert_eq!(layout::item_at(point.x, point.y, columns, metrics, len), Some(index));
    }

    #[test]
    fn visible_range_is_exactly_the_intersecting_tiles(
        metrics in metrics_strategy(),
        columns in 1usize..6,
        len in 0usize..200,
        scroll in 0i32..5000,
        viewport in 0i32..400,
    ) {
        let range = layout::visible_range(scroll, viewport, columns, metrics, len);
        let bottom = scroll + viewport;
        for index in 0..len {
            let rect = layout::tile_rect(index, columns, metrics);
            let intersects = viewport > 0 && rect.top < bottom && rect.bottom > scroll;
            prop_assert_eq!(
                range.contains(&index),
                intersects,
                "index {} of {} at scroll {} viewport {} (tile {:?})",
                index, len, scroll, viewport, rect
            );
        }
    }
}
