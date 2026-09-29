#![forbid(unsafe_code)]

//! The icon view's event mapper: pointer and wheel input to selection, focus,
//! scrolling and the app's `Msg`; keyboard input is handled in
//! [`keyboard`](super::keyboard).

use std::cell::RefCell;
use std::rc::Rc;

use super::Mappers;
use super::keyboard::{apply_click, handle_key};
use super::layout;
use super::state::State;
use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::geometry::Point;
use crate::message::{Modifiers, MouseButton};
use crate::widget::scrollbar::ScrollBar;

/// Rows scrolled per wheel notch.
const WHEEL_ROWS: i32 = 3;

/// Builds the event closure an [`IconView`](super::IconView) registers.
pub(crate) fn mapper<M: 'static>(
    ui: Ui<M>,
    id: WidgetId,
    state: Rc<RefCell<State>>,
    mappers: Rc<Mappers<M>>,
    bar: Rc<ScrollBar>,
) -> impl Fn(&Event) -> Option<M> + 'static {
    move |event| {
        // Focus and resize are handled even while disabled or designing.
        match event {
            Event::SetFocus => {
                state.borrow_mut().has_focus = true;
                ui.invalidate(id);
                return None;
            }
            Event::KillFocus => {
                state.borrow_mut().has_focus = false;
                ui.invalidate(id);
                return None;
            }
            Event::Resize { .. } => {
                let bounds = ui.bounds(id);
                let dpi = ui.dpi();
                super::bar::layout(&ui, id, &bar, &state.borrow());
                state.borrow_mut().clamp_offset(bounds, dpi);
                ui.invalidate(bar.id());
                return None;
            }
            _ => {}
        }
        // In design mode the editor handles input, not the widget.
        if (ui.is_design_mode() && event.is_input()) || !state.borrow().enabled {
            return None;
        }
        handle(&ui, id, &state, &mappers, &bar, event)
    }
}

fn handle<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    bar: &ScrollBar,
    event: &Event,
) -> Option<M> {
    let dpi = ui.dpi();
    match event {
        Event::MouseDown {
            button: MouseButton::Left,
            x,
            y,
            modifiers,
            ..
        } => left_click(
            ui,
            id,
            state,
            mappers,
            bar,
            Point::new(*x, *y),
            *modifiers,
            dpi,
        ),
        Event::MouseDown {
            button: MouseButton::Right,
            x,
            y,
            ..
        } => right_click(ui, id, state, mappers, bar, Point::new(*x, *y), dpi),
        Event::MouseDoubleClick {
            button: MouseButton::Left,
            x,
            y,
            ..
        } => {
            let index = hit(ui, id, state, *x, *y, dpi)?;
            mappers
                .activate
                .borrow()
                .as_ref()
                .and_then(|activate| activate(index))
        }
        Event::MouseMove { x, y, .. } => {
            update_hover(ui, id, state, *x, *y, dpi);
            None
        }
        Event::MouseLeave => {
            if state.borrow_mut().hover.take().is_some() {
                ui.invalidate(id);
            }
            None
        }
        Event::MouseWheel {
            delta,
            horizontal: false,
            ..
        } => {
            wheel(ui, id, bar, state, *delta, dpi);
            None
        }
        Event::KeyDown {
            key,
            modifiers,
            repeat,
            system,
        } if *repeat <= 1 && !*system => {
            handle_key(ui, id, state, mappers, bar, *key, *modifiers, dpi)
        }
        _ => None,
    }
}

/// A left press: select the hit tile (or clear on empty space) and raise the
/// selection message.
#[allow(clippy::too_many_arguments)]
fn left_click<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    bar: &ScrollBar,
    at: Point,
    modifiers: Modifiers,
    dpi: u32,
) -> Option<M> {
    ui.focus(id);
    match hit(ui, id, state, at.x, at.y, dpi) {
        Some(index) => {
            {
                let bounds = ui.bounds(id);
                let mut state = state.borrow_mut();
                apply_click(&mut state, index, modifiers);
                state.has_focus = true;
                state.ensure_visible(index, bounds, dpi);
            }
            ui.invalidate(id);
            ui.invalidate(bar.id());
        }
        None => {
            let changed = state.borrow_mut().clear();
            if changed {
                ui.invalidate(id);
                ui.invalidate(bar.id());
            }
        }
    }
    emit_selection(mappers, state)
}

/// A right press: select an unselected tile first (XP behaviour) but report
/// only through `on_context`, carrying the tile (`None` on empty space) and the
/// node-local pointer position.
fn right_click<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    bar: &ScrollBar,
    at: Point,
    dpi: u32,
) -> Option<M> {
    let hit = hit(ui, id, state, at.x, at.y, dpi);
    let bounds = ui.bounds(id);
    let changed = {
        let mut state = state.borrow_mut();
        match hit {
            Some(index) if !state.selected.contains(&index) => {
                state.set_single(index);
                state.ensure_visible(index, bounds, dpi);
                true
            }
            Some(index) => {
                state.focused = Some(index);
                false
            }
            None => false,
        }
    };
    if changed {
        ui.invalidate(id);
        ui.invalidate(bar.id());
    }
    mappers
        .context
        .borrow()
        .as_ref()
        .and_then(|context| context(hit, at))
}

/// The tile at node-local `(x, y)`, or `None`.
fn hit<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    x: i32,
    y: i32,
    dpi: u32,
) -> Option<usize> {
    let bounds = ui.bounds(id);
    let state = state.borrow();
    let viewport = state.viewport(bounds, dpi);
    let metrics = state.metrics(dpi);
    layout::item_at(x, y + state.offset, viewport.columns, metrics, state.len())
}

fn update_hover<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    x: i32,
    y: i32,
    dpi: u32,
) {
    let next = hit(ui, id, state, x, y, dpi);
    let changed = {
        let mut state = state.borrow_mut();
        let changed = state.hover != next;
        state.hover = next;
        changed
    };
    if changed {
        ui.invalidate(id);
    }
}

fn wheel<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    bar: &ScrollBar,
    state: &Rc<RefCell<State>>,
    delta: i16,
    dpi: u32,
) {
    let bounds = ui.bounds(id);
    let mut state = state.borrow_mut();
    let metrics = state.metrics(dpi);
    // A notch is 1 or 120 (`WHEEL_DELTA`), so scroll by its sign, not its size.
    let step = i64::from(delta.signum()) * WHEEL_ROWS as i64 * metrics.stride_y().max(1) as i64;
    let max = state.max_offset(bounds, dpi) as i64;
    let next = (state.offset as i64 - step).clamp(0, max) as i32;
    if next != state.offset {
        state.offset = next;
        drop(state);
        ui.invalidate(id);
        ui.invalidate(bar.id());
    }
}

/// Maps the current selection to the app's message. `on_selection` sees the
/// whole set; otherwise `on_select` sees the focused item. The state borrow is
/// released before the mapper runs, so a mapper may call back into the widget.
pub(super) fn emit_selection<M: 'static>(
    mappers: &Mappers<M>,
    state: &Rc<RefCell<State>>,
) -> Option<M> {
    let (rows, primary) = {
        let state = state.borrow();
        (state.selection(), state.primary())
    };
    if let Some(selection) = mappers.selection.borrow().as_ref() {
        return selection(&rows);
    }
    primary.and_then(|item| {
        mappers
            .select
            .borrow()
            .as_ref()
            .and_then(|select| select(item))
    })
}
