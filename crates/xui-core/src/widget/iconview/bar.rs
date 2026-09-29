#![forbid(unsafe_code)]

//! The icon view's vertical scrollbar: its metrics, layout and input mapping.

use std::cell::RefCell;
use std::rc::Rc;

use super::state::State;
use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::geometry::Rect;
use crate::widget::scrollbar::{Scroll, ScrollBar};

/// The scroll metrics of the view's viewport.
pub(crate) fn metrics<M: 'static>(ui: &Ui<M>, id: WidgetId, state: &State) -> Scroll {
    let bounds = ui.bounds(id);
    let viewport = state.viewport(bounds, ui.dpi());
    Scroll {
        viewport: bounds.height().max(0),
        content: viewport.content_height,
        offset: state.offset,
    }
}

/// Lays the bar along the trailing edge and shows it only on overflow.
pub(crate) fn layout<M: 'static>(ui: &Ui<M>, id: WidgetId, bar: &ScrollBar, state: &State) {
    let bounds = ui.bounds(id);
    let viewport = state.viewport(bounds, ui.dpi());
    bar.set_track(viewport.bar_width, bounds.height().max(0));
    ui.set_visible(bar.id(), viewport.bar_width > 0);
    ui.apply_moves(&[(
        bar.id(),
        Rect::new(
            bounds.right - viewport.bar_width,
            bounds.top,
            bounds.right,
            bounds.bottom,
        ),
    )]);
}

/// Builds the event closure the bar node registers.
pub(crate) fn mapper<M: 'static>(
    ui: Ui<M>,
    id: WidgetId,
    bar: Rc<ScrollBar>,
    state: Rc<RefCell<State>>,
) -> impl Fn(&Event) -> Option<M> + 'static {
    move |event| {
        if (ui.is_design_mode() && event.is_input()) || !state.borrow().enabled {
            return None;
        }
        let dpi = ui.dpi();
        let metrics = metrics(&ui, id, &state.borrow());
        bar.handle(
            &ui,
            metrics,
            |target| set_scroll(&ui, id, &bar, &state, target, dpi),
            event,
        );
        None
    }
}

/// Applies a pixel `target` offset, clamped to the content.
fn set_scroll<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    bar: &ScrollBar,
    state: &Rc<RefCell<State>>,
    target: i32,
    dpi: u32,
) {
    let bounds = ui.bounds(id);
    let mut state = state.borrow_mut();
    let max = state.max_offset(bounds, dpi);
    let next = target.clamp(0, max);
    if next != state.offset {
        state.offset = next;
        drop(state);
        ui.invalidate(id);
        ui.invalidate(bar.id());
    }
}
