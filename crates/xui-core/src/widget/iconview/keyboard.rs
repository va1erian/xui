#![forbid(unsafe_code)]

//! The icon view's keyboard mapper: arrow/Home/End/Page navigation, Ctrl+Space
//! toggling, Return activation and the Menu-key context request.

use std::cell::RefCell;
use std::rc::Rc;

use super::Mappers;
use super::events::emit_selection;
use super::layout::{self, Direction};
use super::state::State;
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::geometry::Point;
use crate::message::{Key, Modifiers};
use crate::widget::SelectionMode;
use crate::widget::scrollbar::ScrollBar;

/// Handles one key press. `columns` and `page` are the viewport's column count
/// and rows-per-page.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_key<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &Rc<RefCell<State>>,
    mappers: &Mappers<M>,
    bar: &ScrollBar,
    key: Key,
    modifiers: Modifiers,
    dpi: u32,
) -> Option<M> {
    let len = state.borrow().len();
    if len == 0 {
        return None;
    }
    let bounds = ui.bounds(id);
    let (columns, page) = {
        let state = state.borrow();
        let viewport = state.viewport(bounds, dpi);
        let metrics = state.metrics(dpi);
        let page = (viewport.height / metrics.stride_y().max(1)).max(1) as usize;
        (viewport.columns, page)
    };
    let page_step = page.saturating_mul(columns);

    if key == Key::MENU || (key == Key::F10 && modifiers.shift) {
        let anchor = state.borrow().primary().map(|item| {
            let state = state.borrow();
            let metrics = state.metrics(dpi);
            let tile = layout::tile_rect(item, columns, metrics);
            (item, Point::new(0, tile.bottom - state.offset))
        });
        let (item, at) = anchor?;
        return mappers
            .context
            .borrow()
            .as_ref()
            .and_then(|context| context(Some(item), at));
    }

    let before = state.borrow().selection();
    let mut activate = None;
    {
        let mut state = state.borrow_mut();
        let current = state.focused.unwrap_or(0).min(len - 1);
        let shift = modifiers.shift;
        match key {
            Key::LEFT => move_focus(
                &mut state,
                layout::navigate(len, columns, current, Direction::Left),
                shift,
            ),
            Key::RIGHT => move_focus(
                &mut state,
                layout::navigate(len, columns, current, Direction::Right),
                shift,
            ),
            Key::UP => move_focus(
                &mut state,
                layout::navigate(len, columns, current, Direction::Up),
                shift,
            ),
            Key::DOWN => move_focus(
                &mut state,
                layout::navigate(len, columns, current, Direction::Down),
                shift,
            ),
            Key::HOME => move_focus(&mut state, 0, shift),
            Key::END => move_focus(&mut state, len - 1, shift),
            Key::PAGE_UP => move_focus(&mut state, current.saturating_sub(page_step), shift),
            Key::PAGE_DOWN => move_focus(
                &mut state,
                current.saturating_add(page_step).min(len - 1),
                shift,
            ),
            Key::SPACE if state.mode == SelectionMode::Multi => {
                if let Some(item) = state.focused {
                    state.toggle(item);
                }
            }
            Key::RETURN => activate = state.focused,
            _ => return None,
        }
        if let Some(item) = state.focused {
            state.ensure_visible(item, bounds, dpi);
        }
    }
    ui.invalidate(id);
    ui.invalidate(bar.id());

    if let Some(item) = activate {
        return mappers
            .activate
            .borrow()
            .as_ref()
            .and_then(|activate| activate(item));
    }
    if state.borrow().selection() == before {
        None
    } else {
        emit_selection(mappers, state)
    }
}

/// Applies a click's modifiers to the selection according to the mode.
pub(super) fn apply_click(state: &mut State, item: usize, modifiers: Modifiers) {
    match state.mode {
        SelectionMode::Single => state.set_single(item),
        SelectionMode::Multi if modifiers.ctrl => state.toggle(item),
        SelectionMode::Multi if modifiers.shift => state.extend(item),
        SelectionMode::Multi => state.set_single(item),
        SelectionMode::Range if modifiers.shift => state.extend(item),
        SelectionMode::Range => state.set_single(item),
    }
}

/// Moves the focus; Shift extends the selection instead of replacing it.
fn move_focus(state: &mut State, item: usize, shift: bool) {
    if shift && state.mode != SelectionMode::Single {
        state.extend(item);
    } else {
        state.set_single(item);
    }
}
