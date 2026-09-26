#![forbid(unsafe_code)]

//! [`TopBar`](super::TopBar)'s event handling.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::app::Ui;
use crate::backend::{Event, WidgetId};
use crate::message::MouseButton;

use super::items::{self, Item, Kind};

/// Maps a clicked icon button to an optional app message.
pub(super) type ClickMapper<M> = Rc<RefCell<Option<Box<dyn Fn(super::TopBarId) -> Option<M>>>>>;
/// Maps a toggled item's new checked state to an optional app message.
pub(super) type ToggleMapper<M> =
    Rc<RefCell<Option<Box<dyn Fn(super::TopBarId, bool) -> Option<M>>>>>;
/// Maps a slider's new value to an optional app message.
pub(super) type ChangeMapper<M> =
    Rc<RefCell<Option<Box<dyn Fn(super::TopBarId, f64) -> Option<M>>>>>;

/// The shared state a [`TopBar`](super::TopBar)'s event mapper reads.
pub(super) struct Events<M: 'static> {
    pub(super) ui: Ui<M>,
    pub(super) id: WidgetId,
    pub(super) items: Rc<RefCell<Vec<Item>>>,
    pub(super) hover: Rc<Cell<Option<usize>>>,
    pub(super) active: Rc<Cell<Option<usize>>>,
    pub(super) on_click: ClickMapper<M>,
    pub(super) on_toggle: ToggleMapper<M>,
    pub(super) on_change: ChangeMapper<M>,
}

impl<M: 'static> Events<M> {
    /// Handles one event, returning the app message it maps to, if any.
    pub(super) fn handle(&self, event: &Event) -> Option<M> {
        let ui = &self.ui;
        if ui.is_design_mode() && event.is_input() {
            return None;
        }
        let items = &self.items;
        let bounds = ui.bounds(self.id);
        let dpi = ui.dpi();
        match event {
            Event::MouseMove { x, .. } => {
                if let Some(index) = self.active.get()
                    && let Some((value, changed)) = items::set_slider(items, index, bounds, dpi, *x)
                    && changed
                {
                    ui.invalidate(self.id);
                    let item_id = items.borrow()[index].id;
                    return self
                        .on_change
                        .borrow()
                        .as_ref()
                        .and_then(|mapper| mapper(item_id, value));
                }
                let hit = items::hit_interactive(&items.borrow()[..], bounds, dpi, *x);
                if self.hover.get() != hit {
                    self.hover.set(hit);
                    ui.invalidate(self.id);
                }
                None
            }
            Event::MouseLeave | Event::CaptureChanged => {
                if self.hover.get().is_some() || self.active.get().is_some() {
                    self.hover.set(None);
                    self.active.set(None);
                    ui.invalidate(self.id);
                }
                None
            }
            Event::MouseDown {
                x,
                button: MouseButton::Left,
                ..
            } => {
                let hit = items::hit_interactive(&items.borrow()[..], bounds, dpi, *x);
                self.active.set(hit);
                self.hover.set(hit);
                ui.invalidate(self.id);
                if let Some(index) = hit
                    && let Some((value, changed)) = items::set_slider(items, index, bounds, dpi, *x)
                    && changed
                {
                    let item_id = items.borrow()[index].id;
                    return self
                        .on_change
                        .borrow()
                        .as_ref()
                        .and_then(|mapper| mapper(item_id, value));
                }
                None
            }
            Event::MouseUp {
                x,
                button: MouseButton::Left,
                ..
            } => {
                let was = self.active.replace(None);
                ui.invalidate(self.id);
                let hit = items::hit_interactive(&items.borrow()[..], bounds, dpi, *x)?;
                if Some(hit) != was {
                    return None;
                }
                let item_id = items.borrow()[hit].id;
                let kind = items.borrow()[hit].kind.clone();
                match kind {
                    Kind::Icon(_) => self
                        .on_click
                        .borrow()
                        .as_ref()
                        .and_then(|mapper| mapper(item_id)),
                    Kind::Toggle { .. } => {
                        let state = {
                            let mut borrowed = items.borrow_mut();
                            let Kind::Toggle { checked, .. } = &mut borrowed[hit].kind else {
                                return None;
                            };
                            *checked = !*checked;
                            *checked
                        };
                        self.on_toggle
                            .borrow()
                            .as_ref()
                            .and_then(|mapper| mapper(item_id, state))
                    }
                    Kind::Slider { .. } | Kind::Label(_) | Kind::Spacer(_) => None,
                }
            }
            _ => None,
        }
    }
}
