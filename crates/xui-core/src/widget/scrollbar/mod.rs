#![forbid(unsafe_code)]

//! The shared scrollbar: track/thumb geometry, painting and drag/track-page
//! input, for either axis.
//!
//! [`ScrollView`](super::ScrollView), [`ListView`](super::ListView) and
//! [`TreeView`](super::TreeView) each own a bar child node along an edge. A
//! widget reports its [`Scroll`] state and a closure that applies a new offset;
//! the [`ScrollBar`] does the rest, so all of them look and behave alike.
//!
//! The module is public so an app that draws its own bar inside a custom
//! widget (a code editor, a property grid) uses the same geometry
//! ([`thumb`], [`offset_from_drag`], [`hit`], [`paged_offset`]) and painter
//! ([`paint_state`]) instead of a copy.

mod geometry;
#[cfg(test)]
mod tests;

use std::cell::Cell;

use crate::app::Ui;
use crate::backend::{Canvas, Event, WidgetId};
use crate::geometry::Rect;
use crate::message::MouseButton;
use crate::theme::Theme;
pub use crate::widget::Orientation;

pub use geometry::{
    Scroll, THICKNESS, ThumbState, TrackHit, hit, offset_from_drag, paged_offset, thumb,
    thumb_color,
};

/// A scrollbar's geometry and drag, in the bar's own coordinates.
///
/// The bar owns a child node `id`; the widget lays that node out, calls
/// [`set_track`](ScrollBar::set_track) after each layout and forwards the
/// node's events to [`handle`](ScrollBar::handle).
pub struct ScrollBar {
    id: WidgetId,
    orientation: Orientation,
    track: Cell<Rect>,
    /// The pointer coordinate a thumb drag started at, while dragging.
    drag: Cell<Option<i32>>,
    /// The offset when the drag started.
    drag_offset: Cell<i32>,
}

impl ScrollBar {
    /// A vertical bar over the node `id`.
    pub fn new(id: WidgetId) -> ScrollBar {
        ScrollBar::with_orientation(id, Orientation::Vertical)
    }

    /// A horizontal bar over the node `id`.
    pub fn horizontal(id: WidgetId) -> ScrollBar {
        ScrollBar::with_orientation(id, Orientation::Horizontal)
    }

    /// A bar running along `orientation` over the node `id`.
    pub fn with_orientation(id: WidgetId, orientation: Orientation) -> ScrollBar {
        ScrollBar {
            id,
            orientation,
            track: Cell::new(Rect::default()),
            drag: Cell::new(None),
            drag_offset: Cell::new(0),
        }
    }

    /// The bar node's identity.
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// The axis the bar scrolls along.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// The track in the bar's own coordinates.
    #[cfg(test)]
    pub(crate) fn track(&self) -> Rect {
        self.track.get()
    }

    /// Records the track's size (in the bar's own coordinates) after a layout.
    pub fn set_track(&self, width: i32, height: i32) {
        self.track.set(Rect::new(0, 0, width, height));
    }

    /// The thumb's rectangle, or `None` when nothing scrolls.
    pub fn thumb(&self, scroll: Scroll, dpi: u32) -> Option<Rect> {
        thumb(self.track.get(), scroll, self.orientation, dpi)
    }

    /// Whether a thumb drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag.get().is_some()
    }

    /// The pointer coordinate along the bar's axis.
    fn along(&self, x: i32, y: i32) -> i32 {
        match self.orientation {
            Orientation::Vertical => y,
            Orientation::Horizontal => x,
        }
    }

    /// Handles a pointer event on the bar, calling `set` with each offset to
    /// scroll to.
    pub fn handle<M: 'static>(&self, ui: &Ui<M>, scroll: Scroll, set: impl Fn(i32), event: &Event) {
        if ui.is_design_mode() && event.is_input() {
            return;
        }
        let dpi = ui.dpi();
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let pointer = self.along(*x, *y);
                match hit(self.track.get(), scroll, self.orientation, pointer, dpi) {
                    Some(TrackHit::Thumb) => {
                        self.drag.set(Some(pointer));
                        self.drag_offset.set(scroll.offset);
                        ui.set_capture(self.id);
                    }
                    Some(TrackHit::Before) => set(paged_offset(scroll, -1, scroll.viewport.max(1))),
                    Some(TrackHit::After) => set(paged_offset(scroll, 1, scroll.viewport.max(1))),
                    None => {}
                }
            }
            Event::MouseMove { x, y, .. } => {
                if let Some(start) = self.drag.get() {
                    set(offset_from_drag(
                        self.track.get(),
                        scroll,
                        self.orientation,
                        self.drag_offset.get(),
                        start,
                        self.along(*x, *y),
                        dpi,
                    ));
                }
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            }
            | Event::CaptureChanged => {
                // Only a drag this bar started holds the capture; a host that
                // forwards every event must not lose a capture of its own.
                if self.drag.take().is_some() && matches!(event, Event::MouseUp { .. }) {
                    ui.release_capture();
                }
            }
            _ => {}
        }
    }
}

/// Paints a bar node's track and, when the content overflows, the thumb. The
/// canvas's bounds are the track.
pub(crate) fn paint(
    canvas: &mut dyn Canvas,
    scroll: Scroll,
    orientation: Orientation,
    theme: Theme,
) {
    let track = canvas.bounds();
    canvas.clear(theme.background);
    paint_state(
        canvas,
        track,
        scroll,
        orientation,
        theme,
        ThumbState::Normal,
    );
}

/// Paints a bar's track and thumb into `track`, with the thumb in `state`.
///
/// For a widget that draws its own bar; the toolkit widgets use bar nodes.
pub fn paint_state(
    canvas: &mut dyn Canvas,
    track: Rect,
    scroll: Scroll,
    orientation: Orientation,
    theme: Theme,
    state: ThumbState,
) {
    canvas.fill_rect(track, theme.scrollbar_track);
    if let Some(thumb) = thumb(track, scroll, orientation, canvas.dpi()) {
        let radius = thumb.width().min(thumb.height()) as f32 / 2.0;
        canvas.fill_rounded_rect(thumb, radius, thumb_color(theme, state));
    }
}
