#![forbid(unsafe_code)]

//! The shared vertical scrollbar: track/thumb geometry, painting and
//! drag/track-page input.
//!
//! [`ScrollView`](super::ScrollView), [`ListView`](super::ListView) and
//! [`TreeView`](super::TreeView) each own a bar child node along their trailing
//! edge. A widget reports its [`Metrics`] and a closure that applies a new
//! offset; the [`Bar`] does the rest, so all three look and behave alike.

use std::cell::Cell;

use crate::app::Ui;
use crate::backend::{Canvas, Event, WidgetId};
use crate::geometry::Rect;
use crate::message::MouseButton;
use crate::theme::Theme;
use crate::units::Dip;

/// The scrollbar's width.
pub(crate) const BAR: Dip = Dip(12.0);
/// The shortest the thumb may shrink to.
const MIN_THUMB: Dip = Dip(24.0);

/// The scroll state a bar draws and drags, in device pixels.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    /// The scrollable body's height.
    pub(crate) viewport: i32,
    /// The content's total height.
    pub(crate) content: i32,
    /// The current scroll offset.
    pub(crate) offset: i32,
}

/// A vertical scrollbar's geometry and drag, in the bar's own coordinates.
pub(crate) struct Bar {
    id: WidgetId,
    track: Cell<Rect>,
    /// The pointer coordinate a thumb drag started at, while dragging.
    drag: Cell<Option<i32>>,
    /// The offset when the drag started.
    drag_offset: Cell<i32>,
}

impl Bar {
    /// A bar over the node `id`.
    pub(crate) fn new(id: WidgetId) -> Bar {
        Bar {
            id,
            track: Cell::new(Rect::default()),
            drag: Cell::new(None),
            drag_offset: Cell::new(0),
        }
    }

    /// The bar node's identity.
    pub(crate) fn id(&self) -> WidgetId {
        self.id
    }

    /// The track in the bar's own coordinates.
    #[cfg(test)]
    pub(crate) fn track(&self) -> Rect {
        self.track.get()
    }

    /// Records the track's size (in the bar's own coordinates) after a layout.
    pub(crate) fn set_track(&self, width: i32, height: i32) {
        self.track.set(Rect::new(0, 0, width, height));
    }

    /// The thumb's rectangle, or `None` when nothing scrolls.
    pub(crate) fn thumb(&self, metrics: Metrics, dpi: u32) -> Option<Rect> {
        thumb_rect(self.track.get(), metrics, dpi)
    }

    /// Handles a pointer event on the bar, calling `set` with each offset to
    /// scroll to.
    pub(crate) fn handle<M: 'static>(
        &self,
        ui: &Ui<M>,
        metrics: Metrics,
        set: impl Fn(i32),
        event: &Event,
    ) {
        if ui.is_design_mode() && event.is_input() {
            return;
        }
        let dpi = ui.dpi();
        match event {
            Event::MouseDown {
                y,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(thumb) = self.thumb(metrics, dpi) {
                    if *y >= thumb.top && *y < thumb.bottom {
                        self.drag.set(Some(*y));
                        self.drag_offset.set(metrics.offset);
                        ui.set_capture(self.id);
                    } else {
                        let page = metrics.viewport.max(1);
                        let target = if *y < thumb.top {
                            metrics.offset - page
                        } else {
                            metrics.offset + page
                        };
                        set(target);
                    }
                }
            }
            Event::MouseMove { y, .. } => {
                if let Some(start) = self.drag.get()
                    && let Some(thumb) = self.thumb(metrics, dpi)
                {
                    let travel = (self.track.get().height() - thumb.height()).max(1);
                    let max = (metrics.content - metrics.viewport).max(0);
                    set(self.drag_offset.get() + (*y - start) * max / travel);
                }
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            }
            | Event::CaptureChanged => {
                self.drag.set(None);
                ui.release_capture();
            }
            _ => {}
        }
    }
}

/// The thumb's rectangle within `track`, or `None` when nothing scrolls.
pub(crate) fn thumb_rect(track: Rect, metrics: Metrics, dpi: u32) -> Option<Rect> {
    let viewport = metrics.viewport.max(0);
    if metrics.content <= viewport || track.height() <= 0 {
        return None;
    }
    let min = MIN_THUMB.to_px(dpi).value().min(track.height());
    let proportional =
        (i64::from(track.height()) * i64::from(viewport) / i64::from(metrics.content)) as i32;
    let thumb_height = proportional.clamp(min, track.height());
    let travel = track.height() - thumb_height;
    let max_offset = (metrics.content - viewport).max(0);
    let y = if max_offset > 0 {
        track.top + travel * metrics.offset.clamp(0, max_offset) / max_offset
    } else {
        track.top
    };
    Some(Rect::new(
        track.left + 2,
        y,
        track.right - 2,
        y + thumb_height,
    ))
}

/// Paints the track and, when the content overflows, the thumb. The canvas's
/// bounds are the track.
pub(crate) fn paint(canvas: &mut dyn Canvas, metrics: Metrics, theme: Theme) {
    let track = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);
    canvas.fill_rect(track, theme.scrollbar_track);
    if let Some(thumb) = thumb_rect(track, metrics, dpi) {
        let radius = thumb.width() as f32 / 2.0;
        canvas.fill_rounded_rect(thumb, radius, theme.scrollbar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(viewport: i32, content: i32, offset: i32) -> Metrics {
        Metrics {
            viewport,
            content,
            offset,
        }
    }

    #[test]
    fn a_fitting_content_has_no_thumb() {
        let track = Rect::new(0, 0, 12, 100);
        assert!(thumb_rect(track, metrics(100, 100, 0), 96).is_none());
        assert!(thumb_rect(track, metrics(100, 50, 0), 96).is_none());
    }

    #[test]
    fn the_thumb_shrinks_and_tracks_the_offset() {
        let track = Rect::new(0, 0, 12, 100);
        let top = thumb_rect(track, metrics(100, 400, 0), 96).unwrap();
        let bottom = thumb_rect(track, metrics(100, 400, 300), 96).unwrap();
        assert!(top.height() < 100, "the thumb is proportional to the body");
        assert_eq!(top.top, 0, "at the top offset the thumb is at the top");
        assert_eq!(bottom.bottom, 100, "at the max offset it is at the bottom");
        assert_eq!(top.height(), bottom.height(), "the thumb keeps its size");
        assert_eq!(top.left, 2, "the thumb is inset from the track's left edge");
    }

    #[test]
    fn the_thumb_has_a_travel_range() {
        let bar = Bar::new(WidgetId::NONE);
        bar.set_track(12, 100);
        // A 100px body over 400px of content is a quarter-height thumb, so it
        // has 75px of travel between the top and bottom offsets.
        assert_eq!(bar.thumb(metrics(100, 400, 0), 96).unwrap().top, 0);
        assert_eq!(bar.thumb(metrics(100, 400, 300), 96).unwrap().top, 75);
    }
}
