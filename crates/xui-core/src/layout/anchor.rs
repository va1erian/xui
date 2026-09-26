#![forbid(unsafe_code)]

//! The pure anchor engine: where an absolutely positioned child lands when its
//! parent is resized.
//!
//! This is the classic nine-point anchor set plus the stretch/fill combinations
//! a resizable window needs. [`anchored`] is a pure function of *relative*
//! geometry — the parent's old and new size and the child's bounds — so it is
//! scale-equivariant: scaling every input by a DPI factor scales the result by
//! the same factor. A caller converts its `Dip` design values to device pixels
//! once with [`Dip::to_px`](crate::units::Dip::to_px) and the arithmetic never
//! sees a DPI, so a stale DPI cannot leak into the layout.
//!
//! ```
//! use xui_core::{Anchor, Rect, Size, anchored};
//!
//! // A control at (10, 20), 50x40, in a parent that grows 400x300 -> 500x400.
//! let before = Size::new(400, 300);
//! let after = Size::new(500, 400);
//! let control = Rect::new(10, 20, 60, 60);
//! assert_eq!(
//!     anchored(before, after, control, Anchor::BottomRight),
//!     Rect::new(110, 120, 160, 160),
//! );
//! ```

use crate::geometry::{Rect, Size};

/// The smallest a control may become, in device pixels. Anchor maths may shrink
/// a control toward this floor but never produces a zero/negative extent while
/// the parent still has room.
pub const MIN_ANCHOR_PX: i32 = 1;

/// How a control follows its parent when the parent is resized.
///
/// The nine point anchors translate the whole control (or keep it still) and
/// leave its size alone; [`Anchor::StretchHorizontal`] pins the left and right
/// edges so the width tracks the parent, [`Anchor::StretchVertical`] pins the
/// top and bottom edges, and [`Anchor::Fill`] pins all four.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// Pinned to the top-left corner: never moves.
    TopLeft,
    /// Centred horizontally, pinned to the top.
    Top,
    /// Pinned to the top-right corner.
    TopRight,
    /// Centred vertically, pinned to the left.
    Left,
    /// Kept at its offset from the parent's centre.
    Center,
    /// Centred vertically, pinned to the right.
    Right,
    /// Pinned to the bottom-left corner.
    BottomLeft,
    /// Centred horizontally, pinned to the bottom.
    Bottom,
    /// Pinned to the bottom-right corner.
    BottomRight,
    /// Left and right edges pinned: the width grows with the parent.
    StretchHorizontal,
    /// Top and bottom edges pinned: the height grows with the parent.
    StretchVertical,
    /// All four edges pinned: the control fills the parent's growth.
    Fill,
}

/// `bounds` repositioned and resized by a parent change from `origin` to
/// `resized`, under `anchor`.
///
/// A point anchor translates the control (or keeps it still) and leaves its size
/// alone; a stretch anchor moves the pinned edge(s) and leaves the opposite
/// offset alone, so the extent grows by the parent's delta. The result is
/// clamped positive and inside `resized`, so a control can never be pushed off
/// the parent or inverted when the parent shrinks past it.
pub fn anchored(origin: Size, resized: Size, bounds: Rect, anchor: Anchor) -> Rect {
    let dx = (resized.width - origin.width) as f32;
    let dy = (resized.height - origin.height) as f32;
    let x = bounds.left as f32;
    let y = bounds.top as f32;
    let w = bounds.width() as f32;
    let h = bounds.height() as f32;

    // The half-deltas are the only fractional values; rounding is deferred to
    // the end so centring a control twice (x and y) cannot drift apart.
    let (x, y, w, h) = match anchor {
        Anchor::TopLeft => (x, y, w, h),
        Anchor::Top => (x + dx / 2.0, y, w, h),
        Anchor::TopRight => (x + dx, y, w, h),
        Anchor::Left => (x, y + dy / 2.0, w, h),
        Anchor::Center => (x + dx / 2.0, y + dy / 2.0, w, h),
        Anchor::Right => (x + dx, y + dy / 2.0, w, h),
        Anchor::BottomLeft => (x, y + dy, w, h),
        Anchor::Bottom => (x + dx / 2.0, y + dy, w, h),
        Anchor::BottomRight => (x + dx, y + dy, w, h),
        Anchor::StretchHorizontal => (x, y, w + dx, h),
        Anchor::StretchVertical => (x, y, w, h + dy),
        Anchor::Fill => (x, y, w + dx, h + dy),
    };

    clamp(
        x.round() as i32,
        y.round() as i32,
        w.round() as i32,
        h.round() as i32,
        resized,
    )
}

/// Keeps a computed rectangle positive and inside `resized`.
///
/// Anchoring can push an edge past its opposite (a control wider than a parent
/// that shrank underneath it) or slide the whole control off the parent. The
/// clamp gives it a positive extent and slides it back in without changing
/// which edges were pinned. A parent smaller than one pixel has no room for a
/// positive control, so the extent collapses to the parent's.
fn clamp(x: i32, y: i32, width: i32, height: i32, resized: Size) -> Rect {
    let max_width = resized.width.max(0);
    let max_height = resized.height.max(0);
    let width = width.max(MIN_ANCHOR_PX).min(max_width);
    let height = height.max(MIN_ANCHOR_PX).min(max_height);
    let x = x.clamp(0, (max_width - width).max(0));
    let y = y.clamp(0, (max_height - height).max(0));
    Rect::new(x, y, x + width, y + height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: i32, height: i32) -> Size {
        Size::new(width, height)
    }

    fn bounds(left: i32, top: i32, width: i32, height: i32) -> Rect {
        Rect::new(left, top, left + width, top + height)
    }

    /// A 100x100 growth of a 400x300 parent; a (10, 20) 50x40 control.
    fn fixture(anchor: Anchor) -> Rect {
        anchored(
            size(400, 300),
            size(500, 400),
            bounds(10, 20, 50, 40),
            anchor,
        )
    }

    fn rect(rect: Rect) -> (i32, i32, i32, i32) {
        (rect.left, rect.top, rect.width(), rect.height())
    }

    #[test]
    fn every_scheme_has_its_documented_result() {
        let cases = [
            (Anchor::TopLeft, (10, 20, 50, 40)),
            (Anchor::Top, (60, 20, 50, 40)),
            (Anchor::TopRight, (110, 20, 50, 40)),
            (Anchor::Left, (10, 70, 50, 40)),
            (Anchor::Center, (60, 70, 50, 40)),
            (Anchor::Right, (110, 70, 50, 40)),
            (Anchor::BottomLeft, (10, 120, 50, 40)),
            (Anchor::Bottom, (60, 120, 50, 40)),
            (Anchor::BottomRight, (110, 120, 50, 40)),
            (Anchor::StretchHorizontal, (10, 20, 150, 40)),
            (Anchor::StretchVertical, (10, 20, 50, 140)),
            (Anchor::Fill, (10, 20, 150, 140)),
        ];
        for (anchor, expected) in cases {
            assert_eq!(rect(fixture(anchor)), expected, "{anchor:?}");
        }
    }

    #[test]
    fn a_sticky_control_keeps_its_left_edge_and_a_right_anchor_keeps_the_gap() {
        let sticky = fixture(Anchor::StretchHorizontal);
        assert_eq!(sticky.left, 10, "pinned left edge stays put");
        assert_eq!(
            sticky.right, 160,
            "the right edge follows the parent by exactly the delta"
        );
        assert_eq!(sticky.width(), 50 + 100);

        // A point Right anchor keeps its distance to the parent's right edge.
        let right = fixture(Anchor::Right);
        assert_eq!(500 - right.right, 340);
        assert_eq!(right.left, 10 + 100);
    }

    #[test]
    fn fill_grows_both_axes_and_center_stays_centred() {
        let fill = fixture(Anchor::Fill);
        assert_eq!((fill.width(), fill.height()), (150, 140));

        let center = fixture(Anchor::Center);
        assert_eq!(center.left + center.width() / 2, 85);
        assert_eq!(center.top + center.height() / 2, 90);
    }

    #[test]
    fn nothing_leaves_the_parent_when_it_shrinks_past_a_control() {
        // A BottomRight control and a Full control in a 400x300 parent that
        // shrinks to a size smaller than either control.
        let origin = size(400, 300);
        let tiny = size(60, 40);
        let cases = [
            (Anchor::BottomRight, bounds(300, 220, 80, 60)),
            (Anchor::Fill, bounds(0, 0, 380, 280)),
            (Anchor::Top, bounds(200, 5, 50, 40)),
        ];
        for (anchor, control) in cases {
            let r = anchored(origin, tiny, control, anchor);
            assert!(r.width() > 0 && r.height() > 0, "{anchor:?} positive");
            assert!(r.left >= 0 && r.top >= 0, "{anchor:?} origin");
            assert!(
                r.right <= tiny.width && r.bottom <= tiny.height,
                "{anchor:?} inside the shrunken parent: {r:?}"
            );
        }
    }

    #[test]
    fn the_engine_is_scale_equivariant() {
        // Every input scaled by the same factor yields the scaled result, which
        // is what makes the layout DPI-independent: the caller converts each
        // design value once, and the arithmetic then sees only ratios.
        let origin = size(400, 300);
        let resized = size(500, 400);
        let control = bounds(10, 20, 50, 40);
        for anchor in [
            Anchor::TopLeft,
            Anchor::Top,
            Anchor::TopRight,
            Anchor::Left,
            Anchor::Center,
            Anchor::Right,
            Anchor::BottomLeft,
            Anchor::Bottom,
            Anchor::BottomRight,
            Anchor::StretchHorizontal,
            Anchor::StretchVertical,
            Anchor::Fill,
        ] {
            let one = anchored(origin, resized, control, anchor);
            let two = anchored(
                size(origin.width * 2, origin.height * 2),
                size(resized.width * 2, resized.height * 2),
                bounds(
                    control.left * 2,
                    control.top * 2,
                    control.width() * 2,
                    control.height() * 2,
                ),
                anchor,
            );
            assert_eq!(
                rect(two),
                {
                    let r = rect(one);
                    (r.0 * 2, r.1 * 2, r.2 * 2, r.3 * 2)
                },
                "{anchor:?}"
            );
        }
    }

    #[test]
    fn centring_an_odd_delta_rounds_consistently() {
        // An odd parent growth halves to a .5, rounded away from zero, once.
        let r = anchored(
            size(101, 101),
            size(102, 102),
            bounds(0, 0, 10, 10),
            Anchor::Center,
        );
        assert_eq!((r.left, r.top), (1, 1));
    }
}
