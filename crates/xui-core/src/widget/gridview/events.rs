#![forbid(unsafe_code)]

//! The grid view's event mapper: pointer and keyboard input to selection,
//! hover, scrolling and the app's `Msg`.

use std::cell::RefCell;
use std::rc::Rc;

use super::Mappers;
use super::layout::{self, Direction};
use super::state::{Metrics, State};
use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::message::{Key, MouseButton};

/// Rows scrolled per wheel notch.
const WHEEL_ROWS: i32 = 3;

/// Builds the event closure a [`GridView`](super::GridView) registers.
pub(crate) fn mapper<M: 'static>(
    ui: Ui<M>,
    id: WidgetId,
    state: Rc<RefCell<State>>,
    mappers: Rc<Mappers<M>>,
) -> impl Fn(&Event) -> Option<M> + 'static {
    move |event| {
        // In design mode the editor handles input, not the widget.
        if (ui.is_design_mode() && event.is_input()) || !state.borrow().enabled {
            return None;
        }
        handle(&ui, id, &state, &mappers, event)
    }
}

fn handle<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    event: &Event,
) -> Option<M> {
    let dpi = ui.dpi();
    match event {
        Event::MouseMove { x, y, .. } => {
            let next = hit(ui, id, state, *x, *y, dpi);
            let changed = {
                let mut state = state.borrow_mut();
                let changed = state.hover != next;
                state.hover = next;
                changed
            };
            if changed {
                ui.invalidate(id);
            }
            None
        }
        Event::MouseLeave => {
            let mut state = state.borrow_mut();
            if state.hover.take().is_some() {
                drop(state);
                ui.invalidate(id);
            }
            None
        }
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => {
            let index = hit(ui, id, state, *x, *y, dpi)?;
            ui.focus(id);
            {
                let mut state = state.borrow_mut();
                state.selected = Some(index);
                let bounds = ui.bounds(id);
                state.ensure_visible(index, bounds.width(), bounds.height(), dpi);
            }
            ui.invalidate(id);
            mappers
                .select
                .borrow()
                .as_ref()
                .and_then(|select| select(index))
        }
        Event::MouseDoubleClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => {
            let index = hit(ui, id, state, *x, *y, dpi)?;
            mappers
                .activate
                .borrow()
                .as_ref()
                .and_then(|activate| activate(index))
        }
        Event::MouseWheel {
            delta,
            horizontal: false,
            ..
        } => {
            let bounds = ui.bounds(id);
            let mut state = state.borrow_mut();
            let metrics = Metrics::of(state.size, dpi);
            let step = i64::from(*delta) * WHEEL_ROWS as i64 * metrics.row_stride() as i64;
            let max = state.max_offset(bounds.width(), bounds.height(), dpi) as i64;
            let next = (state.offset as i64 - step).clamp(0, max) as i32;
            if next != state.offset {
                state.offset = next;
                drop(state);
                ui.invalidate(id);
            }
            None
        }
        Event::KeyDown {
            key,
            repeat,
            system,
            ..
        } if *repeat <= 1 && !*system => handle_key(ui, id, state, mappers, *key),
        _ => None,
    }
}

fn handle_key<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    key: Key,
) -> Option<M> {
    let len = state.borrow().len();
    if len == 0 {
        return None;
    }
    if key == Key::RETURN || key == Key::SPACE {
        let index = state.borrow().selected?;
        return mappers
            .activate
            .borrow()
            .as_ref()
            .and_then(|activate| activate(index));
    }
    let direction = match key {
        Key::LEFT => Direction::Left,
        Key::RIGHT => Direction::Right,
        Key::UP => Direction::Up,
        Key::DOWN => Direction::Down,
        _ => return None,
    };

    let dpi = ui.dpi();
    let bounds = ui.bounds(id);
    let mut state = state.borrow_mut();
    let columns = state.columns(bounds.width(), dpi);
    let current = state.selected.unwrap_or(0);
    let index = layout::navigate(len, columns, current, direction);
    state.selected = Some(index);
    state.ensure_visible(index, bounds.width(), bounds.height(), dpi);
    drop(state);
    ui.invalidate(id);
    mappers
        .select
        .borrow()
        .as_ref()
        .and_then(|select| select(index))
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
    let state = state.borrow();
    let bounds = ui.bounds(id);
    let metrics = Metrics::of(state.size, dpi);
    let columns = state.columns(bounds.width(), dpi);
    layout::index_at_point(
        x,
        y + state.offset,
        metrics.width,
        metrics.height,
        metrics.gap,
        columns,
        state.len(),
    )
}
