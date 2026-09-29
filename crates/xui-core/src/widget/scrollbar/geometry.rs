#![forbid(unsafe_code)]

//! Scrollbar geometry: the thumb, drag and page maths shared by every bar.
//!
//! Everything here is pure, over the rectangle of the track being drawn (the
//! caller picks the coordinate space: node-local for a widget's own bar), so an
//! app that paints its own bar gets the same look and behaviour as the toolkit
//! widgets.

use crate::Color;
use crate::geometry::Rect;
use crate::theme::Theme;
use crate::units::Dip;
use crate::widget::Orientation;

/// The scrollbar's thickness.
pub const THICKNESS: Dip = Dip(12.0);
/// The shortest the thumb may shrink to.
const MIN_THUMB: Dip = Dip(24.0);

/// The scroll state a bar draws and drags, in device pixels along its axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroll {
    /// The visible extent.
    pub viewport: i32,
    /// The content's total extent.
    pub content: i32,
    /// The current offset.
    pub offset: i32,
}

impl Scroll {
    /// The largest useful offset.
    pub fn max_offset(self) -> i32 {
        (self.content - self.viewport.max(0)).max(0)
    }

    /// Whether the content overflows the viewport.
    pub fn overflows(self) -> bool {
        self.content > self.viewport.max(0)
    }
}

/// The track's length along `orientation`.
fn length(track: Rect, orientation: Orientation) -> i32 {
    match orientation {
        Orientation::Vertical => track.height(),
        Orientation::Horizontal => track.width(),
    }
}

/// The thumb's rectangle within `track`, or `None` when nothing scrolls.
pub fn thumb(track: Rect, scroll: Scroll, orientation: Orientation, dpi: u32) -> Option<Rect> {
    let length = length(track, orientation);
    if !scroll.overflows() || length <= 0 {
        return None;
    }
    let min = MIN_THUMB.to_px(dpi).value().min(length);
    let proportional =
        (i64::from(length) * i64::from(scroll.viewport.max(0)) / i64::from(scroll.content)) as i32;
    let thumb_len = proportional.clamp(min, length);
    let travel = length - thumb_len;
    let max = scroll.max_offset();
    let pos = if max > 0 {
        // i64: a long document's offset times the track can exceed i32.
        (i64::from(travel) * i64::from(scroll.offset.clamp(0, max)) / i64::from(max)) as i32
    } else {
        0
    };
    Some(match orientation {
        Orientation::Vertical => Rect::new(
            track.left + 2,
            track.top + pos,
            track.right - 2,
            track.top + pos + thumb_len,
        ),
        Orientation::Horizontal => Rect::new(
            track.left + pos,
            track.top + 2,
            track.left + pos + thumb_len,
            track.bottom - 2,
        ),
    })
}

/// The offset a thumb drag to `pointer` reaches, given where the drag began
/// (`start_offset` at `start_pointer`, both along the bar's axis).
pub fn offset_from_drag(
    track: Rect,
    scroll: Scroll,
    orientation: Orientation,
    start_offset: i32,
    start_pointer: i32,
    pointer: i32,
    dpi: u32,
) -> i32 {
    let Some(thumb) = thumb(track, scroll, orientation, dpi) else {
        return 0;
    };
    let thumb_len = length(thumb, orientation);
    let travel = (length(track, orientation) - thumb_len).max(1);
    let max = scroll.max_offset();
    // i64 throughout: the drag distance times a long document's range can
    // exceed i32; the clamped result fits.
    let delta =
        (i64::from(pointer) - i64::from(start_pointer)) * i64::from(max) / i64::from(travel);
    (i64::from(start_offset) + delta).clamp(0, i64::from(max)) as i32
}

/// The thumb's interaction state, for its colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThumbState {
    /// Neither hovered nor dragged.
    #[default]
    Normal,
    /// The pointer is over the bar.
    Hover,
    /// The thumb is being dragged.
    Pressed,
}

/// The thumb colour for `state`: the theme's thumb, drawn towards the text
/// colour when hovered and further when pressed, so it works in both themes.
pub fn thumb_color(theme: Theme, state: ThumbState) -> Color {
    match state {
        ThumbState::Normal => theme.scrollbar,
        ThumbState::Hover => theme.scrollbar.lerp(theme.text, 0.25),
        ThumbState::Pressed => theme.scrollbar.lerp(theme.text, 0.5),
    }
}

/// Where a press on a bar landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackHit {
    /// On the thumb: start a drag.
    Thumb,
    /// On the track before the thumb (above or left of it): page back.
    Before,
    /// On the track after the thumb: page forward.
    After,
}

/// What a press at `pointer` (along the bar's axis) hits, or `None` when
/// nothing scrolls.
pub fn hit(
    track: Rect,
    scroll: Scroll,
    orientation: Orientation,
    pointer: i32,
    dpi: u32,
) -> Option<TrackHit> {
    let thumb = thumb(track, scroll, orientation, dpi)?;
    let (start, end) = match orientation {
        Orientation::Vertical => (thumb.top, thumb.bottom),
        Orientation::Horizontal => (thumb.left, thumb.right),
    };
    Some(if pointer < start {
        TrackHit::Before
    } else if pointer >= end {
        TrackHit::After
    } else {
        TrackHit::Thumb
    })
}

/// The offset after paging in `direction` (-1 back, 1 forward) by `page`,
/// clamped to the scrollable range.
pub fn paged_offset(scroll: Scroll, direction: i32, page: i32) -> i32 {
    (scroll.offset + direction.signum() * page.max(0)).clamp(0, scroll.max_offset())
}
