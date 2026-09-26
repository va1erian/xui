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
}

impl<M: 'static> Input<M> {
    /// Maps one backend event to the app's message.
    pub(crate) fn handle(&self, event: &Event) -> Option<M> {
        // In design mode the editor handles input, not the widget.
        if (self.ui.is_design_mode() && event.is_input()) || !self.enabled.get() {
            return None;
        }
        match event {
            Event::MouseDown {
                button: MouseButton::Left,
                ..
            } => self.mouse_down(event),
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
            Event::KeyDown { repeat, system, .. } if *repeat <= 1 && !*system => {
                self.key_down(event)
            }
            _ => None,
        }
    }

    fn mouse_down(&self, event: &Event) -> Option<M> {
        let (x, y) = event.position()?;
        let dpi = self.ui.dpi();
        let (slot, index) = {
            let state = self.state.borrow();
            let slot = flatten::row_at(dpi, y, state.rows.len())?;
            (slot, flatten::slot_to_index(&state.rows, slot)?)
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
            let top = slot as i32 * row_height;
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

    fn key_down(&self, event: &Event) -> Option<M> {
        let Event::KeyDown { key, .. } = *event else {
            return None;
        };
        let selected = self.selected.get();
        match key {
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
                }
                None
            }
            Key::LEFT | Key::RIGHT => self.expand(self.index_of(selected)?, key == Key::RIGHT),
            Key::RETURN => self
                .mappers
                .select
                .borrow()
                .as_ref()
                .and_then(|map| map(selected?)),
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
            state.rows[index].id
        };
        self.ui.invalidate(self.id);
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
        let slot = flatten::row_at(dpi, y, state.rows.len())?;
        let index = flatten::slot_to_index(&state.rows, slot)?;
        Some(state.rows[index].id)
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
