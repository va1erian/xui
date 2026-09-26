#![forbid(unsafe_code)]

//! The [`GridView`](super::GridView) runtime API: models, tile size, selection
//! and scrolling.

use std::rc::Rc;

use super::GridView;
use super::model::{GridModel, TileSize};
use crate::backend::WidgetId;
use crate::property::{Properties, Property, Value};

impl<M: 'static> GridView<M> {
    /// Replaces the model and refreshes the view. The scroll resets and a
    /// selection past the new end moves to the first tile.
    pub fn set_model(&self, model: impl GridModel + 'static) {
        let mut state = self.state.borrow_mut();
        state.model = Rc::new(model);
        let len = state.model.len();
        if state.selected.is_none_or(|index| index >= len) {
            state.selected = (!state.model.is_empty()).then_some(0);
        }
        state.hover = None;
        state.offset = 0;
        drop(state);
        self.control.invalidate();
    }

    /// Sets the tile size (and gap), reflowing the columns.
    pub fn set_tile_size(&self, size: impl Into<TileSize>) {
        self.state.borrow_mut().size = size.into();
        self.control.invalidate();
    }

    /// The current tile size.
    pub fn current_tile_size(&self) -> TileSize {
        self.state.borrow().size
    }

    /// The selected tile, if any.
    pub fn selected(&self) -> Option<usize> {
        self.state.borrow().selected
    }

    /// Selects `index` without raising an event; an out-of-range index clears
    /// the selection.
    pub fn select(&self, index: Option<usize>) {
        let len = self.state.borrow().len();
        self.state.borrow_mut().selected = index.filter(|index| *index < len);
        self.control.invalidate();
    }

    /// The number of tiles.
    pub fn len(&self) -> usize {
        self.state.borrow().len()
    }

    /// Whether the grid has no tiles.
    pub fn is_empty(&self) -> bool {
        self.state.borrow().len() == 0
    }

    /// Scrolls `index` into view.
    pub fn ensure_visible(&self, index: usize) {
        let dpi = self.control.dpi();
        let bounds = self.control.bounds();
        self.state
            .borrow_mut()
            .ensure_visible(index, bounds.width(), bounds.height(), dpi);
        self.control.invalidate();
    }

    /// The grid's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the grid. A disabled grid is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.state.borrow_mut().enabled = enabled;
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the grid selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for GridView<M> {
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
