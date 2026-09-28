#![forbid(unsafe_code)]

//! The tree's event mapper: selection, expansion, checkbox toggling and the
//! keyboard navigation, all skipping hidden rows.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::Mappers;
use super::flatten::{self, FlatNode, State};
use super::model::NodeId;
use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::geometry::Point;
use crate::message::{Key, MouseButton};
use crate::widget::scrollbar::Bar;

/// The shared handles an event mapper reads and writes.
pub(crate) struct Input<M: 'static> {
    pub(crate) ui: Ui<M>,
    pub(crate) id: WidgetId,
    pub(crate) state: Rc<RefCell<State>>,
    pub(crate) selected: Rc<Cell<Option<NodeId>>>,
    pub(crate) hover: Rc<Cell<Option<NodeId>>>,
    pub(crate) enabled: Rc<Cell<bool>>,
    pub(crate) checkboxes: Rc<Cell<bool>>,
    pub(crate) tri_state: Rc<Cell<bool>>,
    pub(crate) mappers: Rc<Mappers<M>>,
    pub(crate) bar: Rc<Bar>,
}

impl<M: 'static> Input<M> {
    /// Maps one backend event to the app's message.
    pub(crate) fn handle(&self, event: &Event) -> Option<M> {
        if let Event::Resize { .. } = event {
            self.sync_bar();
            return None;
        }
        // In design mode the editor handles input, not the widget.
        if (self.ui.is_design_mode() && event.is_input()) || !self.enabled.get() {
            return None;
        }
        match event {
            Event::MouseDown {
                button: MouseButton::Left,
                ..
            } => self.mouse_down(event),
            Event::MouseDown {
                button: MouseButton::Right,
                ..
            } => self.context(event),
            Event::MouseDoubleClick {
                button: MouseButton::Left,
                ..
            } => self.double_click(event),
            Event::MouseMove { y, .. } => {
                let id = self.id_at(*y);
                if self.hover.get() != id {
                    self.hover.set(id);
                    self.ui.invalidate(self.id);
                }
                None
            }
            Event::MouseLeave => {
                self.hover.set(None);
                self.ui.invalidate(self.id);
                None
            }
            Event::MouseWheel {
                delta,
                horizontal: false,
                ..
            } => self.wheel(*delta),
            Event::KeyDown { repeat, system, .. } if *repeat <= 1 && !*system => {
                self.key_down(event)
            }
            _ => None,
        }
    }

    fn mouse_down(&self, event: &Event) -> Option<M> {
        let (x, y) = event.position()?;
        let dpi = self.ui.dpi();
        let (slot, index, offset) = {
            let state = self.state.borrow();
            let slot = flatten::row_at(dpi, y, state.rows.len(), state.offset)?;
            (
                slot,
                flatten::slot_to_index(&state.rows, slot)?,
                state.offset,
            )
        };
        let (id, depth, expandable, expanded) = {
            let state = self.state.borrow();
            let node = &state.rows[index];
            (node.id, node.depth, node.expandable, node.expanded)
        };
        if flatten::chevron_hit(dpi, 0, depth, expandable, x) {
            return self.expand(index, !expanded);
        }
        if self.checkboxes.get() {
            let row_height = flatten::ROW.to_px(dpi).value().max(1);
            let top = (slot - offset) as i32 * row_height;
            if flatten::checkbox_rect(dpi, 0, top, depth).contains(Point::new(x, y)) {
                return self.toggle_check(index);
            }
        }
        self.selected.set(Some(id));
        self.ui.invalidate(self.id);
        self.mappers
            .select
            .borrow()
            .as_ref()
            .and_then(|map| map(id))
    }

    /// Activates the row under a double click (not its chevron or checkbox).
    fn double_click(&self, event: &Event) -> Option<M> {
        let (x, y) = event.position()?;
        let dpi = self.ui.dpi();
        let (id, on_control) = {
            let state = self.state.borrow();
            let slot = flatten::row_at(dpi, y, state.rows.len(), state.offset)?;
            let index = flatten::slot_to_index(&state.rows, slot)?;
            let node = &state.rows[index];
            let row_height = flatten::ROW.to_px(dpi).value().max(1);
            let top = (slot - state.offset) as i32 * row_height;
            let on_chevron = flatten::chevron_hit(dpi, 0, node.depth, node.expandable, x);
            let on_check = self.checkboxes.get()
                && flatten::checkbox_rect(dpi, 0, top, node.depth).contains(Point::new(x, y));
            (node.id, on_chevron || on_check)
        };
        if on_control {
            return None;
        }
        self.selected.set(Some(id));
        self.ui.invalidate(self.id);
        self.activate(id)
    }

    /// Maps activating `id` to the app's message.
    fn activate(&self, id: NodeId) -> Option<M> {
        self.mappers
            .activate
            .borrow()
            .as_ref()
            .and_then(|map| map(id))
    }

    /// Maps a right click on a row to the app's context message with its
    /// node-local pointer position.
    fn context(&self, event: &Event) -> Option<M> {
        let (x, y) = event.position()?;
        let dpi = self.ui.dpi();
        let (id, at) = {
            let state = self.state.borrow();
            let slot = flatten::row_at(dpi, y, state.rows.len(), state.offset)?;
            let index = flatten::slot_to_index(&state.rows, slot)?;
            (state.rows[index].id, Point::new(x, y))
        };
        self.emit_context(id, at)
    }

    /// Maps the Menu key to the selected row's context message, anchored at
    /// the row's bottom-left corner.
    fn context_keyboard(&self) -> Option<M> {
        let dpi = self.ui.dpi();
        let (id, at) = {
            let state = self.state.borrow();
            let id = self.selected.get()?;
            let index = state.find(id)?;
            let slot = flatten::index_to_slot(&state.rows, index)?;
            let row = slot.saturating_sub(state.offset);
            let y = (row as i32 + 1) * flatten::ROW.to_px(dpi).value().max(1);
            (id, Point::new(0, y))
        };
        self.emit_context(id, at)
    }

    /// Selects `id`, repaints and maps its context message.
    fn emit_context(&self, id: NodeId, at: Point) -> Option<M> {
        self.selected.set(Some(id));
        self.ui.invalidate(self.id);
        self.mappers
            .context
            .borrow()
            .as_ref()
            .and_then(|map| map(id, at))
    }

    fn key_down(&self, event: &Event) -> Option<M> {
        let Event::KeyDown { key, modifiers, .. } = *event else {
            return None;
        };
        let selected = self.selected.get();
        match key {
            Key::MENU => self.context_keyboard(),
            Key::F10 if modifiers.shift => self.context_keyboard(),
            Key::UP | Key::DOWN | Key::HOME | Key::END => {
                let target = {
                    let state = self.state.borrow();
                    if state.rows.is_empty() {
                        return None;
                    }
                    let from = selected.and_then(|id| state.find(id));
                    match key {
                        Key::UP => step(&state.rows, from, -1),
                        Key::DOWN => step(&state.rows, from, 1),
                        Key::HOME => {
                            (0..state.rows.len()).find(|i| flatten::is_visible(&state.rows, *i))
                        }
                        _ => (0..state.rows.len())
                            .rev()
                            .find(|i| flatten::is_visible(&state.rows, *i)),
                    }
                };
                if let Some(index) = target {
                    let id = self.state.borrow().rows[index].id;
                    if selected != Some(id) {
                        self.selected.set(Some(id));
                        self.ui.invalidate(self.id);
                    }
                    self.scroll_selection_into_view();
                }
                None
            }
            Key::LEFT | Key::RIGHT => self.expand(self.index_of(selected)?, key == Key::RIGHT),
            Key::RETURN => {
                let id = selected?;
                if self.mappers.activate.borrow().is_some() {
                    return self.activate(id);
                }
                self.mappers
                    .select
                    .borrow()
                    .as_ref()
                    .and_then(|map| map(id))
            }
            Key::SPACE => {
                if !self.checkboxes.get() {
                    return None;
                }
                self.toggle_check(self.index_of(selected)?)
            }
            _ => None,
        }
    }

    /// Expands or collapses the row at `index`, raising `on_toggle`.
    fn expand(&self, index: usize, expanded: bool) -> Option<M> {
        let id = {
            let mut state = self.state.borrow_mut();
            if !state.set_expanded(index, expanded) {
                return None;
            }
            let visible = self.visible_slots();
            state.clamp_offset(visible);
            state.rows[index].id
        };
        self.ui.invalidate(self.id);
        self.sync_bar();
        self.mappers
            .toggle
            .borrow()
            .as_ref()
            .and_then(|map| map(id, expanded))
    }

    /// Advances the checkbox of the row at `index`, raising `on_check`.
    fn toggle_check(&self, index: usize) -> Option<M> {
        let (id, next) = {
            let mut state = self.state.borrow_mut();
            let next = state.rows[index].checked.cycled(self.tri_state.get());
            state.rows[index].checked = next;
            (state.rows[index].id, next)
        };
        self.ui.invalidate(self.id);
        self.mappers
            .check
            .borrow()
            .as_ref()
            .and_then(|map| map(id, next))
    }

    fn index_of(&self, id: Option<NodeId>) -> Option<usize> {
        self.state.borrow().find(id?)
    }

    fn id_at(&self, y: i32) -> Option<NodeId> {
        let dpi = self.ui.dpi();
        let state = self.state.borrow();
        let slot = flatten::row_at(dpi, y, state.rows.len(), state.offset)?;
        let index = flatten::slot_to_index(&state.rows, slot)?;
        Some(state.rows[index].id)
    }

    /// Re-lays the bar out after the visible row count changed.
    fn sync_bar(&self) {
        super::bar::layout(&self.ui, self.id, &self.bar, &self.state.borrow());
        self.ui.invalidate(self.bar.id());
    }

    /// How many whole rows fit in the body.
    fn visible_slots(&self) -> usize {
        let row_px = flatten::ROW.to_px(self.ui.dpi()).value().max(1);
        ((self.ui.bounds(self.id).height().max(0) / row_px) as usize).max(1)
    }

    /// Scrolls the wheel by `delta` notches.
    fn wheel(&self, delta: i16) -> Option<M> {
        let visible = self.visible_slots();
        let mut state = self.state.borrow_mut();
        let max = state.visible_len().saturating_sub(visible);
        let step = i64::from(delta) * flatten::WHEEL_ROWS as i64;
        let next = (state.offset as i64 - step).clamp(0, max as i64) as usize;
        if next != state.offset {
            state.offset = next;
            drop(state);
            self.ui.invalidate(self.id);
            self.ui.invalidate(self.bar.id());
        }
        None
    }

    /// Scrolls just enough that the selected row is fully visible.
    fn scroll_selection_into_view(&self) {
        let Some(id) = self.selected.get() else {
            return;
        };
        let slot = {
            let state = self.state.borrow();
            state
                .find(id)
                .and_then(|index| flatten::index_to_slot(&state.rows, index))
        };
        let visible = self.visible_slots();
        let changed = {
            let mut state = self.state.borrow_mut();
            let before = state.offset;
            match slot {
                Some(slot) => state.scroll_to_slot(slot, visible),
                None => state.clamp_offset(visible),
            }
            state.offset != before
        };
        if changed {
            self.ui.invalidate(self.id);
            self.ui.invalidate(self.bar.id());
        }
    }
}

/// The next visible row from `from` in `dir`, or the first/last when nothing is
/// selected.
fn step(rows: &[FlatNode], from: Option<usize>, dir: isize) -> Option<usize> {
    let len = rows.len() as isize;
    let mut at = match from {
        Some(index) => index as isize + dir,
        None if dir > 0 => 0,
        None => len - 1,
    };
    while at >= 0 && at < len && !flatten::is_visible(rows, at as usize) {
        at += dir;
    }
    (at >= 0 && at < len).then_some(at as usize)
}
