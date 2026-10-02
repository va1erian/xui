//! The vertical scrollbar of [`HtmlWidget`](super::HtmlWidget), drawn inside
//! the view's own node along its right edge.
//!
//! The view is a single custom-painted node, so it cannot host xui-core's bar
//! node; it uses the same geometry and painter instead
//! ([`xui_core::widget::scrollbar`]), so it looks and behaves like every other
//! xui scrollbar: drag the thumb, click the track to page.

use xui_core::backend::Canvas;
use xui_core::geometry::{Point as PxPoint, Rect as PxRect};
use xui_core::theme::Theme;
use xui_core::widget::scrollbar::{self, Orientation, Scroll, THICKNESS, ThumbState, TrackHit};

use super::HtmlWidget;

/// The bar's width in device pixels at `dpi`.
pub(super) fn width(dpi: u32) -> i32 {
    THICKNESS.to_px(dpi).value()
}

/// The bar's track along the right edge of `bounds` (node-local device pixels).
pub(super) fn track(bounds: PxRect, dpi: u32) -> PxRect {
    let left = (bounds.right - width(dpi)).max(bounds.left);
    PxRect::new(left, bounds.top, bounds.right, bounds.bottom)
}

/// The widget's scroll state (device-independent pixels) as the bar's device
/// pixels.
fn to_bar(viewport: f32, content: f32, offset: f32, scale: f32) -> Scroll {
    let px = |v: f32| (v * scale).round() as i32;
    Scroll {
        viewport: px(viewport),
        content: px(content),
        offset: px(offset),
    }
}

impl HtmlWidget {
    /// The scroll state the bar draws and drags.
    fn bar_scroll(&self) -> Scroll {
        to_bar(
            self.viewport_height.get(),
            self.content_height(),
            self.scroll.get(),
            self.scale.get(),
        )
    }

    /// Whether the node-local device pixel `(x, y)` is on the bar.
    pub(super) fn on_bar(&self, x: i32, y: i32) -> bool {
        let track = self.bar_track.get();
        !track.is_empty() && track.contains(PxPoint::new(x, y))
    }

    /// Paints the bar; the thumb shows only when the page overflows.
    pub(super) fn paint_bar(&self, canvas: &mut dyn Canvas, theme: Theme) {
        let state = if self.bar_drag.get().is_some() {
            ThumbState::Pressed
        } else {
            ThumbState::Normal
        };
        scrollbar::paint_state(
            canvas,
            self.bar_track.get(),
            self.bar_scroll(),
            Orientation::Vertical,
            theme,
            state,
        );
    }

    /// A left press at node-local `y` on the bar: grabs the thumb or pages.
    /// Returns whether the thumb was grabbed (the drag needs the capture).
    pub(super) fn press_bar(&self, y: i32) -> bool {
        let scroll = self.bar_scroll();
        let dpi = self.dpi();
        match scrollbar::hit(self.bar_track.get(), scroll, Orientation::Vertical, y, dpi) {
            Some(TrackHit::Thumb) => {
                self.bar_drag.set(Some((y, scroll.offset)));
                return true;
            }
            Some(TrackHit::Before) => self.scroll_by(-self.viewport_height.get()),
            Some(TrackHit::After) => self.scroll_by(self.viewport_height.get()),
            None => {}
        }
        false
    }

    /// Follows a thumb drag to node-local `y`. Returns false when no drag is
    /// in progress.
    pub(super) fn drag_bar(&self, y: i32) -> bool {
        let Some((start, start_offset)) = self.bar_drag.get() else {
            return false;
        };
        let offset = scrollbar::offset_from_drag(
            self.bar_track.get(),
            self.bar_scroll(),
            Orientation::Vertical,
            start_offset,
            start,
            y,
            self.dpi(),
        );
        self.scroll.set(offset as f32 / self.scale.get());
        true
    }

    /// Ends a thumb drag. Returns whether one was in progress.
    pub(super) fn release_bar(&self) -> bool {
        self.bar_drag.take().is_some()
    }

    /// The window's dpi, from the scale the last paint saw.
    fn dpi(&self) -> u32 {
        (self.scale.get() * 96.0).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_hugs_the_right_edge() {
        let t = track(PxRect::new(0, 0, 400, 300), 96);
        assert_eq!(t, PxRect::new(400 - width(96), 0, 400, 300));
        // Twice the dpi, twice the width.
        assert_eq!(width(192), 2 * width(96));
    }

    #[test]
    fn track_never_leaves_a_narrow_view() {
        let t = track(PxRect::new(0, 0, 5, 300), 96);
        assert_eq!(t.left, 0);
        assert_eq!(t.right, 5);
    }

    #[test]
    fn scroll_state_is_scaled_to_device_pixels() {
        let s = to_bar(300.0, 1200.0, 150.0, 1.5);
        assert_eq!(
            s,
            Scroll {
                viewport: 450,
                content: 1800,
                offset: 225
            }
        );
        assert!(s.overflows());
        assert!(!to_bar(300.0, 200.0, 0.0, 1.0).overflows());
    }
}
