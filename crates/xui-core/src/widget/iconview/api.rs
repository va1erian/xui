#![forbid(unsafe_code)]

//! The [`IconView`](super::IconView) runtime API: models, icon size, selection
//! and scrolling.

use std::collections::BTreeSet;
use std::rc::Rc;

use super::IconView;
use super::bar;
use super::model::{IconModel, IconSize};
use crate::backend::WidgetId;
use crate::geometry::Point;
use crate::property::{Properties, Property, Value};
use crate::widget::CellData;
use crate::widget::SelectionMode;

impl<M: 'static> IconView<M> {
    /// Sets the icon size, reflows the tiles, keeps the focused item in view
    /// and repaints. See [`IconSize`].
    pub fn set_icon_size(&self, size: IconSize) {
        let bounds = self.control.bounds();
        let dpi = self.control.dpi();
        {
            let mut state = self.state.borrow_mut();
            if state.size == size {
                return;
            }
            state.size = size;
            state.cache_reset();
            match state.focused {
                Some(item) => state.ensure_visible(item, bounds, dpi),
                None => state.clamp_offset(bounds, dpi),
            }
        }
        self.reflow();
    }

    /// The current icon size.
    pub fn icon_size(&self) -> IconSize {
        self.state.borrow().size
    }

    /// Replaces the model and refreshes the view. The scroll resets and the
    /// selection is kept where it still exists.
    pub fn set_model(&self, model: impl IconModel + 'static) {
        self.state.borrow_mut().replace_model(Rc::new(model));
        self.reflow();
    }

    /// Replaces the model with plain names.
    pub fn set_items(&self, items: &[&str]) {
        let model: Vec<String> = items.iter().map(|item| item.to_string()).collect();
        self.set_model(model);
    }

    /// Sets how clicks and the keyboard select tiles.
    pub fn selection_mode(self, mode: SelectionMode) -> IconView<M> {
        self.state.borrow_mut().mode = mode;
        self
    }

    /// A shortcut for [`selection_mode`](IconView::selection_mode): multi
    /// select, or single select.
    pub fn multi_select(self, multi: bool) -> IconView<M> {
        self.selection_mode(if multi {
            SelectionMode::Multi
        } else {
            SelectionMode::Single
        })
    }

    /// Maps selecting the primary item to the app's message. When
    /// [`on_selection`](IconView::on_selection) is set it takes over and this
    /// one is not called.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> IconView<M> {
        *self.mappers.select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a selection change to the app's message. The slice holds every
    /// selected item, ascending — empty when the selection was cleared.
    pub fn on_selection(self, mapper: impl Fn(&[usize]) -> Option<M> + 'static) -> IconView<M> {
        *self.mappers.selection.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps activating the focused item (Return or a double-click) to the app's
    /// message.
    pub fn on_activate(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> IconView<M> {
        *self.mappers.activate.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a right click (or the Menu key) to the app's message. The item is
    /// `None` on empty space; the pointer position is in node-local device
    /// pixels, so the app can anchor its context menu there.
    pub fn on_context(
        self,
        mapper: impl Fn(Option<usize>, Point) -> Option<M> + 'static,
    ) -> IconView<M> {
        *self.mappers.context.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// The first selected item, if any.
    pub fn selected(&self) -> Option<usize> {
        self.state.borrow().selected.iter().next().copied()
    }

    /// Every selected item, ascending.
    pub fn selection(&self) -> Vec<usize> {
        self.state.borrow().selection()
    }

    /// The item the keyboard acts on.
    pub fn focused(&self) -> Option<usize> {
        self.state.borrow().focused
    }

    /// Selects `index` alone without raising an event; an out-of-range index
    /// clears the selection.
    pub fn select(&self, index: Option<usize>) {
        let len = self.state.borrow().len();
        let mut state = self.state.borrow_mut();
        state.selected.clear();
        match index.filter(|index| *index < len) {
            Some(index) => {
                state.selected.insert(index);
                state.focused = Some(index);
                state.anchor = Some(index);
            }
            None => {
                state.focused = None;
                state.anchor = None;
            }
        }
        drop(state);
        self.control.invalidate();
    }

    /// Makes `items` the selection without raising an event. Out-of-range and
    /// duplicate items are dropped; the first becomes the focus.
    pub fn set_selection(&self, items: &[usize]) {
        let len = self.state.borrow().len();
        let mut next = BTreeSet::new();
        for item in items.iter().copied().filter(|item| *item < len) {
            next.insert(item);
        }
        let focused = next.iter().next().copied();
        let mut state = self.state.borrow_mut();
        state.selected = next;
        state.focused = focused;
        state.anchor = focused;
        drop(state);
        self.control.invalidate();
    }

    /// Scrolls `index` into view.
    pub fn ensure_visible(&self, index: usize) {
        let bounds = self.control.bounds();
        self.state
            .borrow_mut()
            .ensure_visible(index, bounds, self.control.dpi());
        self.control.invalidate();
        self.control.ui().invalidate(self.bar.id());
    }

    /// Reads back an item's opaque payload, if the model attached one.
    pub fn item_data(&self, item: usize) -> Option<CellData> {
        self.state.borrow().model.data(item)
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.state.borrow().len()
    }

    /// Whether the view has no items.
    pub fn is_empty(&self) -> bool {
        self.state.borrow().len() == 0
    }

    /// The view's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the view. A disabled view is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.enabled = enabled;
            if !enabled {
                state.hover = None;
            }
        }
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Raises the view above its siblings together with its scrollbar. Raising
    /// only the view would draw it over its own scrollbar, so use this when a
    /// surface stacks the view above other nodes.
    pub fn raise(&self) {
        let ui = self.control.ui();
        ui.raise(self.control.id());
        ui.raise(self.bar.id());
    }

    /// Marks the view selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }

    /// Schedules a repaint of the view. Call this after the model's *appearance*
    /// changed without its items changing, such as a live icon state the model
    /// reads in [`IconModel::paint_icon`](super::IconModel::paint_icon).
    pub fn invalidate(&self) {
        self.control.invalidate();
    }

    /// Re-lays the scrollbar out and repaints against the current state.
    fn reflow(&self) {
        let ui = self.control.ui();
        bar::layout(ui, self.control.id(), &self.bar, &self.state.borrow());
        self.control.invalidate();
        ui.invalidate(self.bar.id());
    }
}

impl<M: 'static> Properties for IconView<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.selected().map_or(-1, |index| index as i64)),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("selected", Value::Integer(index)) => {
                self.select((index >= 0).then_some(index as usize));
                true
            }
            _ => false,
        }
    }
}
