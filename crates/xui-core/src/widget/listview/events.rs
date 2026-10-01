#![forbid(unsafe_code)]

//! The list view's event mapper: pointer and keyboard input to selection,
//! scrolling and the app's `Msg`.

use std::cell::RefCell;
use std::rc::Rc;

use super::Mappers;
use super::bar;
use super::model::SelectionMode;
use super::resize;
use super::state::{
    ROW, State, WHEEL_ROWS, column_at, column_widths, header_px, row_at, toggled_sort,
};
use crate::app::Ui;
use crate::backend::{Cursor, Event, WidgetId};
use crate::geometry::Point;
use crate::message::{Key, Modifiers, MouseButton};
use crate::widget::scrollbar::ScrollBar;

/// Builds the event closure a [`ListView`](super::ListView) registers.
pub(crate) fn mapper<M: 'static>(
    ui: Ui<M>,
    id: WidgetId,
    state: Rc<RefCell<State>>,
    mappers: Rc<Mappers<M>>,
    bar: Rc<ScrollBar>,
) -> impl Fn(&Event) -> Option<M> + 'static {
    move |event| {
        if let Event::Resize { .. } = event {
            bar::layout(&ui, id, &bar, &state.borrow());
            return None;
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
            x,
            y,
            button,
            modifiers,
        } => {
            let (x, y, button, modifiers) = (*x, *y, *button, *modifiers);
            let (header, header_h) = {
                let state = state.borrow();
                let header = state.has_header();
                (header, header_px(header, dpi))
            };
            if header && y < header_h {
                if button != MouseButton::Left {
                    return None;
                }
                // A press on a column boundary starts a resize instead of a
                // sort, so dragging never toggles the arrow.
                let grabbed = {
                    let mut state = state.borrow_mut();
                    let widths = column_widths(dpi, ui.bounds(id).width(), &state.columns);
                    resize::begin(&mut state, x, &widths, dpi)
                };
                if grabbed.is_some() {
                    ui.set_capture(id);
                    ui.set_cursor(id, Cursor::SizeHorizontal);
                    return None;
                }
                let column = {
                    let state = state.borrow();
                    let widths = column_widths(dpi, ui.bounds(id).width(), &state.columns);
                    column_at(&widths, x)
                };
                let column = column?;
                {
                    let mut state = state.borrow_mut();
                    state.sort = Some(toggled_sort(state.sort, column));
                }
                ui.invalidate(id);
                return mappers.sort.borrow().as_ref().and_then(|sort| sort(column));
            }
            let row = {
                let state = state.borrow();
                row_at(
                    ROW.to_px(dpi).value().max(1),
                    state.offset,
                    state.len(),
                    header_h,
                    y,
                )
            };
            let row = row?;
            match button {
                MouseButton::Right => {
                    state.borrow_mut().focused = Some(row);
                    ui.invalidate(id);
                    mappers
                        .context
                        .borrow()
                        .as_ref()
                        .and_then(|context| context(row, Point::new(x, y)))
                }
                MouseButton::Left => {
                    {
                        let mut state = state.borrow_mut();
                        apply_click(&mut state, row, modifiers);
                        let visible = visible_rows(ui, id, header, dpi);
                        state.ensure_visible(row, visible);
                    }
                    ui.invalidate(id);
                    ui.invalidate(bar.id());
                    selection_message(mappers, state)
                }
                _ => None,
            }
        }
        Event::MouseUp {
            button: MouseButton::Left,
            ..
        }
        | Event::CaptureChanged => {
            let finished = {
                let mut state = state.borrow_mut();
                resize::finish(&mut state)
            };
            ui.release_capture();
            ui.set_cursor(id, Cursor::Default);
            finished.and_then(|(column, width)| {
                mappers
                    .resize
                    .borrow()
                    .as_ref()
                    .and_then(|resize| resize(column, width))
            })
        }
        Event::MouseDoubleClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => {
            let header_h = header_px(state.borrow().has_header(), dpi);
            if *y >= header_h {
                let row = {
                    let state = state.borrow();
                    row_at(
                        ROW.to_px(dpi).value().max(1),
                        state.offset,
                        state.len(),
                        header_h,
                        *y,
                    )
                }?;
                return mappers
                    .activate
                    .borrow()
                    .as_ref()
                    .and_then(|activate| activate(row));
            }
            let (column, width) = {
                let mut state = state.borrow_mut();
                let widths = column_widths(dpi, ui.bounds(id).width(), &state.columns);
                let column = resize::boundary_at(&widths, *x, resize::GRAB.to_px(dpi).value())?;
                // Drop any drag the preceding press began, then auto-size.
                state.resize = None;
                let width = resize::autosize(ui, id, &state, column, dpi);
                resize::set_width(&mut state, column, width);
                (column, width)
            };
            ui.set_cursor(id, Cursor::SizeHorizontal);
            ui.invalidate(id);
            mappers
                .resize
                .borrow()
                .as_ref()
                .and_then(|resize| resize(column, width))
        }
        Event::MouseMove { x, y, .. } => {
            let dragging = {
                let mut state = state.borrow_mut();
                resize::drag(&mut state, *x, dpi).is_some()
            };
            if dragging {
                ui.set_cursor(id, Cursor::SizeHorizontal);
                ui.invalidate(id);
                return None;
            }
            let (header_h, boundary) = {
                let state = state.borrow();
                let header_h = header_px(state.has_header(), dpi);
                let boundary = if *y < header_h {
                    let widths = column_widths(dpi, ui.bounds(id).width(), &state.columns);
                    resize::boundary_at(&widths, *x, resize::GRAB.to_px(dpi).value())
                } else {
                    None
                };
                (header_h, boundary)
            };
            ui.set_cursor(
                id,
                if boundary.is_some() {
                    Cursor::SizeHorizontal
                } else {
                    Cursor::Default
                },
            );
            let row = {
                let state = state.borrow();
                row_at(
                    ROW.to_px(dpi).value().max(1),
                    state.offset,
                    state.len(),
                    header_h,
                    *y,
                )
            };
            let changed = {
                let mut state = state.borrow_mut();
                let changed = state.hover != row;
                state.hover = row;
                changed
            };
            if changed {
                ui.invalidate(id);
            }
            None
        }
        Event::MouseLeave => {
            ui.set_cursor(id, Cursor::Default);
            let mut state = state.borrow_mut();
            if state.hover.take().is_some() {
                drop(state);
                ui.invalidate(id);
            }
            None
        }
        Event::MouseWheel {
            delta, horizontal, ..
        } if !*horizontal => {
            let header = state.borrow().has_header();
            let visible = visible_rows(ui, id, header, dpi);
            let mut state = state.borrow_mut();
            let max = state.len().saturating_sub(visible);
            // Backends report a notch as 1 or as 120 (`WHEEL_DELTA`), so scroll
            // by the direction, not the magnitude, as `ScrollView` does.
            let step = i64::from(delta.signum()) * WHEEL_ROWS as i64;
            let next = (state.offset as i64 - step).clamp(0, max as i64) as usize;
            if next != state.offset {
                state.offset = next;
                drop(state);
                ui.invalidate(id);
                ui.invalidate(bar.id());
            }
            None
        }
        Event::KeyDown {
            key,
            modifiers,
            repeat,
            system,
        } if *repeat <= 1 && !*system => handle_key(ui, id, state, mappers, bar, *key, *modifiers),
        _ => None,
    }
}

fn handle_key<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    bar: &ScrollBar,
    key: Key,
    modifiers: Modifiers,
) -> Option<M> {
    let header = state.borrow().has_header();
    let visible = visible_rows(ui, id, header, ui.dpi());
    let len = state.borrow().len();
    if len == 0 {
        return None;
    }
    if key == Key::MENU || (key == Key::F10 && modifiers.shift) {
        let row = state.borrow().primary()?;
        // Anchor the keyboard menu at the focused row's bottom-left corner.
        let (offset, header_h) = {
            let state = state.borrow();
            (state.offset, header_px(state.has_header(), ui.dpi()))
        };
        let row_px = ROW.to_px(ui.dpi()).value().max(1);
        let slot = row.saturating_sub(offset) as i32;
        let at = Point::new(0, header_h + (slot + 1) * row_px);
        return mappers
            .context
            .borrow()
            .as_ref()
            .and_then(|context| context(row, at));
    }

    if key == Key::RETURN {
        // The row is copied out first: `activate` is app code and may call
        // back into this list (`set_items`, `select`), which must not find
        // the state still borrowed.
        let row = state.borrow().focused?;
        return mappers
            .activate
            .borrow()
            .as_ref()
            .and_then(|activate| activate(row));
    }

    let before = state.borrow().selected.clone();
    {
        let mut state = state.borrow_mut();
        let current = state.focused.unwrap_or(0);
        match key {
            Key::UP => move_focus(&mut state, current.saturating_sub(1), modifiers.shift),
            Key::DOWN => move_focus(&mut state, (current + 1).min(len - 1), modifiers.shift),
            Key::HOME => move_focus(&mut state, 0, modifiers.shift),
            Key::END => move_focus(&mut state, len - 1, modifiers.shift),
            Key::PAGE_UP => {
                move_focus(&mut state, current.saturating_sub(visible), modifiers.shift)
            }
            Key::PAGE_DOWN => move_focus(
                &mut state,
                (current + visible).min(len - 1),
                modifiers.shift,
            ),
            Key::SPACE if state.mode == SelectionMode::Multi => {
                let row = state.focused?;
                state.toggle(row);
            }
            _ => return None,
        }
        if let Some(row) = state.focused {
            state.ensure_visible(row, visible);
        }
    }
    ui.invalidate(id);
    ui.invalidate(bar.id());
    if state.borrow().selected == before {
        None
    } else {
        selection_message(mappers, state)
    }
}

/// Applies a click's modifiers to the selection according to the mode.
fn apply_click(state: &mut State, row: usize, modifiers: Modifiers) {
    match state.mode {
        SelectionMode::Single => state.set_single(row),
        SelectionMode::Multi if modifiers.ctrl => state.toggle(row),
        SelectionMode::Multi if modifiers.shift => state.extend(row),
        SelectionMode::Multi => state.set_single(row),
        SelectionMode::Range if modifiers.shift => state.extend(row),
        SelectionMode::Range => state.set_single(row),
    }
}

/// Moves the focus; Shift extends the selection instead of replacing it.
fn move_focus(state: &mut State, row: usize, shift: bool) {
    if shift && state.mode != SelectionMode::Single {
        state.extend(row);
    } else {
        state.set_single(row);
    }
}

/// Maps the current selection to the app's message. `on_selection` sees the
/// whole set; otherwise `on_select` sees the focused row.
///
/// The selection is copied out and the borrow released before the mapper runs:
/// the mapper is app code and may call back into this list (a file dialog
/// refreshes its rows from `on_select`), which must not find the state borrowed.
fn selection_message<M: 'static>(mappers: &Mappers<M>, state: &RefCell<State>) -> Option<M> {
    let (rows, primary) = {
        let state = state.borrow();
        (state.selection(), state.primary())
    };
    if let Some(selection) = mappers.selection.borrow().as_ref() {
        return selection(&rows);
    }
    match primary {
        Some(primary) => mappers
            .select
            .borrow()
            .as_ref()
            .and_then(|select| select(primary)),
        None => None,
    }
}

/// How many whole rows fit in the body below the header.
fn visible_rows<M: 'static>(ui: &Ui<M>, id: WidgetId, header: bool, dpi: u32) -> usize {
    let bounds = ui.bounds(id);
    let body = (bounds.height() - header_px(header, dpi)).max(0);
    ((body / ROW.to_px(dpi).value().max(1)) as usize).max(1)
}
