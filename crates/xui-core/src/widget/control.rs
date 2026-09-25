#![forbid(unsafe_code)]

//! [`Control`]: the portable node handle every widget holds, plus the
//! [`HasText`] capability.
//!
//! A `Control` owns one node created through [`Ui`]: it registers the widget's
//! painter and event mapper, moves and shows it, and destroys it on drop.

use std::cell::Cell;
use std::rc::Rc;

use crate::app::Ui;
use crate::backend::{Event, NodeSpec, Painter, Result, WidgetId};
use crate::geometry::Rect;

/// The node handle every widget holds.
pub struct Control<M: 'static> {
    ui: Ui<M>,
    id: WidgetId,
    /// Whether a form editor has selected the widget; its painter draws an
    /// outline while set.
    selected: Rc<Cell<bool>>,
}

impl<M: 'static> Control<M> {
    /// Creates a node from `spec`, parented to the window.
    pub fn new(ui: &Ui<M>, spec: &NodeSpec) -> Result<Control<M>> {
        let id = ui.create_node(spec)?;
        Ok(Control {
            ui: ui.clone(),
            id,
            selected: Rc::new(Cell::new(false)),
        })
    }

    /// The node's identity.
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// The window handle this node belongs to.
    pub fn ui(&self) -> &Ui<M> {
        &self.ui
    }

    /// Registers the widget's draw routine.
    pub fn set_painter(&self, painter: Painter) {
        self.ui.set_painter(self.id, painter);
    }

    /// Registers the widget's event mapper.
    pub fn on_events(&self, mapper: impl Fn(&Event) -> Option<M> + 'static) {
        self.ui.register_events(self.id, mapper);
    }

    /// Moves/resizes the node.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.id, rect)]);
    }

    /// Shows or hides the node.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.id, visible);
    }

    /// Enables or disables the node.
    pub fn set_enabled(&self, enabled: bool) {
        self.ui.set_enabled(self.id, enabled);
    }

    /// Gives the node the keyboard focus.
    pub fn focus(&self) {
        self.ui.focus(self.id);
    }

    /// Schedules a repaint of the node.
    pub fn invalidate(&self) {
        self.ui.invalidate(self.id);
    }

    /// The node's current text.
    pub fn text(&self) -> String {
        self.ui.text(self.id)
    }

    /// Replaces the node's text.
    pub fn set_text(&self, text: &str) {
        self.ui.set_text(self.id, text);
    }

    /// The node's current bounds, in device pixels.
    pub fn bounds(&self) -> Rect {
        self.ui.bounds(self.id)
    }

    /// Marks the widget as selected or not. A painted widget draws an outline
    /// while selected (a form editor's selection); a natively hosted one draws
    /// nothing.
    pub fn set_selected(&self, selected: bool) {
        self.selected.set(selected);
        self.invalidate();
    }

    /// Whether the widget is selected.
    pub fn is_selected(&self) -> bool {
        self.selected.get()
    }

    /// A shared handle to the selection flag, so a painter reads it live.
    pub(crate) fn selected_handle(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.selected)
    }

    /// The window's dots-per-inch.
    pub fn dpi(&self) -> u32 {
        self.ui.dpi()
    }
}

impl<M: 'static> Drop for Control<M> {
    fn drop(&mut self) {
        // The widget owns its node: destroying it drops the painter the
        // backend held and unregisters the event mapper (which captured the
        // window), so nothing outlives the widget.
        self.ui.destroy(self.id);
    }
}

/// A widget that carries a single text.
pub trait HasText {
    /// The widget's current text.
    fn text(&self) -> String;

    /// Replaces the widget's text.
    fn set_text(&self, text: &str);
}
