#![forbid(unsafe_code)]

//! The page change of [`Tabs`](super::Tabs): the new page scrolls in from the
//! side its tab is on while the old one scrolls out the other way.
//!
//! Pages are child nodes, so the scroll moves them. While it runs the old
//! page stays visible and the container clips its children, so a page that is
//! partly outside is not drawn over the container's neighbours. Both pages
//! are hit-tested where they are drawn, so for the length of the scroll a
//! press lands on whatever is under the pointer at that moment. A layout of
//! the container meanwhile keeps both where the scroll is; another selection
//! or a removed page ends it at once.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{Shared, page_area};
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Rect;
use crate::motion::{Animated, Motion};

/// A page scroll in flight.
#[derive(Clone, Copy)]
struct Flight {
    /// The page scrolling out.
    from: usize,
    /// `1` when the new page comes from the right, `-1` from the left.
    direction: i32,
}

/// The scroll between two pages.
pub(super) struct Slide {
    /// 0 when the old page fills the page area, 1 when the new one does.
    progress: Rc<Animated>,
    flight: Cell<Option<Flight>>,
    /// The moves of one frame and the children hidden on the last, kept so a
    /// frame allocates nothing.
    moves: RefCell<Vec<(WidgetId, Rect)>>,
    hidden: RefCell<Vec<WidgetId>>,
}

impl Slide {
    pub(super) fn new() -> Slide {
        Slide {
            progress: Rc::new(Animated::new(1.0)),
            flight: Cell::new(None),
            moves: RefCell::default(),
            hidden: RefCell::default(),
        }
    }

    /// The page scrolling out, while a scroll is in flight.
    pub(super) fn outgoing(&self) -> Option<usize> {
        self.flight.get().map(|flight| flight.from)
    }

    /// Ends a scroll in flight where it stands: the clip goes, and the layout
    /// that follows places and hides the pages.
    pub(super) fn end<M: 'static>(&self, ui: &Ui<M>, container: WidgetId) {
        if self.flight.take().is_some() {
            self.progress.jump(1.0);
            ui.set_clip(container, None);
        }
    }
}

/// Puts both pages where a scroll in flight is now, for a layout of the
/// container that happens meanwhile (a resize, the old page shown again).
pub(super) fn keep<M: 'static>(ui: &Ui<M>, s: &Shared<M>) {
    ui.set_clip(s.id, Some(Rect::from_size(ui.bounds(s.id).size())));
    place(ui, s, s.slide.progress.value(ui.frame_time()));
}

/// Where the page `offset` pages away from the page area is, `progress` of
/// the way through a scroll in `direction`.
fn scrolled(page: Rect, progress: f32, direction: i32, offset: i32) -> Rect {
    let width = page.width() as f32;
    let shift = ((1.0 - progress) * width).round() as i32 - offset * page.width();
    page.offset(shift * direction, 0)
}

/// Scrolls from page `from` to the selected page, which the caller has just
/// laid out. Without animations the layout already is the result.
pub(super) fn start<M: 'static>(ui: &Ui<M>, s: &Rc<Shared<M>>, from: usize) {
    let to = s.selected.get();
    if !ui.animations() || from == to || page_area(ui, s).is_none() {
        return;
    }
    let direction = if to > from { 1 } else { -1 };
    s.slide.flight.set(Some(Flight { from, direction }));
    s.slide.progress.jump(0.0);
    ui.set_clip(s.id, Some(Rect::from_size(ui.bounds(s.id).size())));
    if let Some(children) = s.pages.borrow().get(from) {
        for &child in children {
            ui.set_visible(child, true);
        }
    }
    place(ui, s, 0.0);
    // Weak: the step lives in the window's driver, and the container's state
    // must not be kept alive by it.
    let (weak, step_ui) = (Rc::downgrade(s), ui.clone());
    ui.animate_with(s.id, &s.slide.progress, 1.0, Motion::TRAVEL, move |p| {
        if let Some(s) = weak.upgrade() {
            place(&step_ui, &s, p);
        }
    });
}

/// Puts both pages where the scroll is at `progress`, and ends it at 1.
fn place<M: 'static>(ui: &Ui<M>, s: &Shared<M>, progress: f32) {
    let Some(flight) = s.slide.flight.get() else {
        return;
    };
    let Some(page) = page_area(ui, s) else {
        return;
    };
    if progress >= 1.0 {
        // Cleared first: hiding the old page runs a layout, which must find
        // no scroll to end.
        s.slide.flight.set(None);
        ui.set_clip(s.id, None);
    }
    // The buffers are taken out for the frame: moving or hiding a node may
    // run a layout, which comes back here.
    let (mut moves, mut hidden) = (s.slide.moves.take(), s.slide.hidden.take());
    {
        let pages = s.pages.borrow();
        let children = |index: usize| pages.get(index).map(Vec::as_slice).unwrap_or_default();
        let (incoming, outgoing) = (children(s.selected.get()), children(flight.from));
        let to = scrolled(page, progress, flight.direction, 0);
        moves.extend(incoming.iter().map(|&child| (child, to)));
        if progress < 1.0 {
            let from = scrolled(page, progress, flight.direction, 1);
            moves.extend(outgoing.iter().map(|&child| (child, from)));
        } else {
            hidden.extend_from_slice(outgoing);
        }
    }
    ui.apply_moves(&moves);
    for &child in &hidden {
        ui.set_visible(child, false);
    }
    moves.clear();
    hidden.clear();
    s.slide.moves.replace(moves);
    s.slide.hidden.replace(hidden);
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: Rect = Rect::new(0, 32, 200, 120);

    #[test]
    fn the_new_page_starts_one_page_away_and_ends_in_place() {
        assert_eq!(scrolled(PAGE, 0.0, 1, 0), PAGE.offset(200, 0));
        assert_eq!(scrolled(PAGE, 0.0, -1, 0), PAGE.offset(-200, 0));
        assert_eq!(scrolled(PAGE, 1.0, 1, 0), PAGE);
    }

    #[test]
    fn the_old_page_stays_one_page_behind_the_new_one() {
        assert_eq!(scrolled(PAGE, 0.0, 1, 1), PAGE);
        for progress in [0.25, 0.5, 0.9] {
            let (new, old) = (
                scrolled(PAGE, progress, 1, 0),
                scrolled(PAGE, progress, 1, 1),
            );
            assert_eq!(old.right, new.left, "no gap and no overlap");
        }
        assert_eq!(scrolled(PAGE, 1.0, -1, 1), PAGE.offset(200, 0));
    }
}
