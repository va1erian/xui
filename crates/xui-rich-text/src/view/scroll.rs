#![forbid(unsafe_code)]

//! The vertical scrollbar: wheel, track paging and thumb dragging, through the
//! toolkit's shared scrollbar geometry.

use xui_core::geometry::Point;
use xui_core::widget::scrollbar::{self, Orientation, Scroll, TrackHit};

use super::state::State;

impl State {
    /// The scroll state the bar draws and drags, in device pixels.
    pub fn bar_scroll(&self) -> Scroll {
        Scroll {
            viewport: self.viewport.round() as i32,
            content: self.content_height().ceil() as i32,
            offset: self.scroll.round() as i32,
        }
    }

    /// Whether the client pixel `(x, y)` is on the bar.
    pub fn on_bar(&self, x: i32, y: i32) -> bool {
        !self.track.is_empty() && self.track.contains(Point::new(x, y))
    }

    /// A press at client `y` on the bar: pages, or grabs the thumb. Returns
    /// whether the thumb was grabbed (the drag needs the mouse capture).
    pub fn press_bar(&mut self, y: i32) -> bool {
        let scroll = self.bar_scroll();
        match scrollbar::hit(self.track, scroll, Orientation::Vertical, y, self.dpi) {
            Some(TrackHit::Thumb) => {
                self.bar_drag = Some((y, scroll.offset));
                return true;
            }
            Some(TrackHit::Before) => self.scroll_by(-self.viewport),
            Some(TrackHit::After) => self.scroll_by(self.viewport),
            None => {}
        }
        false
    }

    /// Follows a thumb drag to client `y`. Returns whether one is in progress.
    pub fn drag_bar(&mut self, y: i32) -> bool {
        let Some((start, offset)) = self.bar_drag else {
            return false;
        };
        let to = scrollbar::offset_from_drag(
            self.track,
            self.bar_scroll(),
            Orientation::Vertical,
            offset,
            start,
            y,
            self.dpi,
        );
        self.scroll = to as f32;
        true
    }

    /// Ends a thumb drag. Returns whether one was in progress.
    pub fn release_bar(&mut self) -> bool {
        self.bar_drag.take().is_some()
    }

    /// Scrolls by `delta` device pixels, clamped to the content.
    pub fn scroll_by(&mut self, delta: f32) {
        self.scroll = (self.scroll + delta).clamp(0.0, self.max_scroll());
    }
}
