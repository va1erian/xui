#![forbid(unsafe_code)]

//! Scrollbar geometry and bar-node tests.

use super::*;
use crate::backend::WidgetId;
use crate::geometry::Rect;
use crate::theme::Theme;

#[test]
fn fitting_content_has_no_thumb() {
    let track = Rect::new(0, 0, 12, 100);
    let fit = Scroll {
        viewport: 100,
        content: 100,
        offset: 0,
    };
    assert!(thumb(track, fit, Orientation::Vertical, 96).is_none());
}

#[test]
fn the_thumb_shrinks_and_tracks_the_offset() {
    let track = Rect::new(0, 0, 12, 100);
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 0,
    };
    let top = thumb(track, scroll, Orientation::Vertical, 96).unwrap();
    assert!(top.height() < 100);
    assert_eq!(top.top, 0);
    let bottom = thumb(
        track,
        Scroll {
            offset: 300,
            ..scroll
        },
        Orientation::Vertical,
        96,
    )
    .unwrap();
    assert_eq!(bottom.bottom, 100);
    assert_eq!(top.height(), bottom.height());
}

#[test]
fn a_horizontal_thumb_runs_along_x() {
    let track = Rect::new(0, 0, 100, 12);
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 150,
    };
    let thumb = thumb(track, scroll, Orientation::Horizontal, 96).unwrap();
    assert!(thumb.width() < 100);
    assert!(thumb.left > 0);
    assert_eq!(thumb.height(), 8);
}

#[test]
fn dragging_maps_pointer_travel_to_offset() {
    let track = Rect::new(0, 0, 12, 100);
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 0,
    };
    // A quarter-height thumb has 75px of travel for 300px of content.
    assert_eq!(
        offset_from_drag(track, scroll, Orientation::Vertical, 0, 0, 75, 96),
        300
    );
    assert_eq!(
        offset_from_drag(track, scroll, Orientation::Vertical, 100, 10, 30, 96),
        180
    );
}

#[test]
fn a_press_hits_the_thumb_or_either_side_of_it() {
    let track = Rect::new(0, 0, 12, 100);
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 150,
    };
    let thumb = thumb(track, scroll, Orientation::Vertical, 96).unwrap();
    let hit = |pointer| hit(track, scroll, Orientation::Vertical, pointer, 96);
    assert_eq!(hit(thumb.top - 1), Some(TrackHit::Before));
    assert_eq!(hit(thumb.top), Some(TrackHit::Thumb));
    assert_eq!(hit(thumb.bottom - 1), Some(TrackHit::Thumb));
    assert_eq!(hit(thumb.bottom), Some(TrackHit::After));
    let fit = Scroll {
        content: 100,
        ..scroll
    };
    assert_eq!(hit_fit(track, fit), None);
}

fn hit_fit(track: Rect, scroll: Scroll) -> Option<TrackHit> {
    hit(track, scroll, Orientation::Vertical, 10, 96)
}

#[test]
fn paging_clamps_at_both_ends() {
    let scroll = Scroll {
        viewport: 100,
        content: 250,
        offset: 20,
    };
    assert_eq!(paged_offset(scroll, -1, 80), 0);
    assert_eq!(paged_offset(scroll, 1, 80), 100);
    assert_eq!(
        paged_offset(
            Scroll {
                offset: 140,
                ..scroll
            },
            1,
            80
        ),
        150
    );
    assert_eq!(paged_offset(scroll, 1, -5), 20);
}

#[test]
fn the_thumb_colour_differs_per_state_in_both_themes() {
    for theme in [Theme::light(), Theme::dark()] {
        let normal = thumb_color(theme, ThumbState::Normal);
        let hover = thumb_color(theme, ThumbState::Hover);
        let pressed = thumb_color(theme, ThumbState::Pressed);
        assert_ne!(normal, hover);
        assert_ne!(hover, pressed);
    }
}

#[test]
fn a_degenerate_track_has_no_thumb_and_a_drag_maps_to_zero() {
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 0,
    };
    let empty = Rect::new(0, 0, 12, 0);
    assert!(thumb(empty, scroll, Orientation::Vertical, 96).is_none());
    assert_eq!(
        offset_from_drag(empty, scroll, Orientation::Vertical, 5, 0, 50, 96),
        0
    );
}

#[test]
fn dragging_clamps_at_both_ends() {
    let track = Rect::new(0, 0, 12, 100);
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 100,
    };
    assert_eq!(
        offset_from_drag(track, scroll, Orientation::Vertical, 100, 0, 5000, 96),
        300
    );
    assert_eq!(
        offset_from_drag(track, scroll, Orientation::Vertical, 100, 50, -5000, 96),
        0
    );
}

#[test]
fn huge_content_does_not_overflow() {
    let scroll = Scroll {
        viewport: 800,
        content: 20_000_000,
        offset: 19_999_000,
    };
    let track = Rect::new(0, 0, 12, 800);
    let bar = thumb(track, scroll, Orientation::Vertical, 96).expect("scrolls");
    assert!(bar.bottom <= 800, "the thumb stays in the track");
    let max = scroll.max_offset();
    // Dragging far past the end clamps instead of overflowing.
    let dragged = offset_from_drag(track, scroll, Orientation::Vertical, 0, 0, 100_000, 96);
    assert_eq!(dragged, max);
}

#[test]
fn a_bar_node_has_a_travel_range() {
    let bar = ScrollBar::new(WidgetId::NONE);
    bar.set_track(12, 100);
    let scroll = |offset| Scroll {
        viewport: 100,
        content: 400,
        offset,
    };
    // A 100px body over 400px of content is a quarter-height thumb, so it
    // has 75px of travel between the top and bottom offsets.
    assert_eq!(bar.thumb(scroll(0), 96).unwrap().top, 0);
    assert_eq!(bar.thumb(scroll(300), 96).unwrap().top, 75);
}

#[test]
fn a_horizontal_bar_node_measures_along_x() {
    let bar = ScrollBar::horizontal(WidgetId::NONE);
    bar.set_track(100, 12);
    let scroll = Scroll {
        viewport: 100,
        content: 400,
        offset: 300,
    };
    let thumb = bar.thumb(scroll, 96).unwrap();
    assert_eq!((thumb.left, thumb.right), (75, 100));
}
