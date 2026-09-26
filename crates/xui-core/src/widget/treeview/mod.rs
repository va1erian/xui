#![forbid(unsafe_code)]

//! [`TreeView`]: a flattened, fixed-row-height collapsible tree.
//!
//! The simple [`TreeView::new`] keeps a flat `&[TreeRow]` working.
//! [`TreeView::with_model`] grows the tree on demand: only the roots are read
//! up front, and a branch's children are fetched the first time it expands, so
//! an unopened branch costs nothing. A per-row checkbox is switched on with
//! [`TreeView::checkboxes`] and its third state with [`TreeView::tri_state`].
//! Indent guides run down each ancestor column, unless
//! [`TreeView::indent_guides`] turns them off. A row may carry a leading
//! [`RowIcon`], a vector [`Glyph`](crate::widget::Glyph) or an
//! [`Image`](crate::image::Image), drawn before its label.
//!
//! Events map to the app's `Msg` through the closures given at construction:
//! [`on_select`](TreeView::on_select),
//! [`on_toggle`](TreeView::on_toggle), [`on_check`](TreeView::on_check) and
//! [`on_context`](TreeView::on_context), which receives the pointer position.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::{Point, Rect};
use crate::property::{Properties, Property, Value};

mod events;
mod flatten;
mod icon;
mod model;
mod paint;

#[cfg(test)]
mod tests;

use self::flatten::State;
pub use self::icon::RowIcon;
pub use self::model::{CheckState, NodeId, TreeModel, TreeNode, TreeRow};

type SelectMapper<M> = Rc<RefCell<Option<Box<dyn Fn(NodeId) -> Option<M>>>>>;
type ToggleMapper<M> = Rc<RefCell<Option<Box<dyn Fn(NodeId, bool) -> Option<M>>>>>;
type CheckMapper<M> = Rc<RefCell<Option<Box<dyn Fn(NodeId, CheckState) -> Option<M>>>>>;
/// Maps a right-clicked row and its node-local pointer position to the app's
/// message.
type ContextMapper<M> = Rc<RefCell<Option<Box<dyn Fn(NodeId, Point) -> Option<M>>>>>;

/// The app-level events a [`TreeView`] maps to `Msg`.
pub(crate) struct Mappers<M> {
    pub(crate) select: SelectMapper<M>,
    pub(crate) toggle: ToggleMapper<M>,
    pub(crate) check: CheckMapper<M>,
    pub(crate) context: ContextMapper<M>,
}

impl<M> Mappers<M> {
    fn new() -> Mappers<M> {
        Mappers {
            select: Rc::new(RefCell::new(None)),
            toggle: Rc::new(RefCell::new(None)),
            check: Rc::new(RefCell::new(None)),
            context: Rc::new(RefCell::new(None)),
        }
    }
}

/// A flattened, fixed-row-height tree over flat rows or a virtual model.
pub struct TreeView<M: 'static> {
    control: Control<M>,
    state: Rc<RefCell<State>>,
    selected: Rc<Cell<Option<NodeId>>>,
    hover: Rc<Cell<Option<NodeId>>>,
    enabled: Rc<Cell<bool>>,
    checkboxes: Rc<Cell<bool>>,
    tri_state: Rc<Cell<bool>>,
    guides: Rc<Cell<bool>>,
    mappers: Rc<Mappers<M>>,
}

impl<M: 'static> TreeView<M> {
    /// Creates a tree of `rows` at `bounds`.
    pub fn new(ui: &Ui<M>, bounds: Rect, rows: &[TreeRow]) -> Result<TreeView<M>> {
        Self::build(ui, bounds, State::flat(rows))
    }

    /// Creates a virtual tree at `bounds`, loading `model`'s roots now and a
    /// branch's children the first time it expands.
    pub fn with_model(
        ui: &Ui<M>,
        bounds: Rect,
        model: impl TreeModel + 'static,
    ) -> Result<TreeView<M>> {
        Self::build(ui, bounds, State::model(Rc::new(model)))
    }

    fn build(ui: &Ui<M>, bounds: Rect, state: State) -> Result<TreeView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::TreeView, bounds).tab_stop())?;
        let state = Rc::new(RefCell::new(state));
        let selected = Rc::new(Cell::new(None));
        let hover = Rc::new(Cell::new(None));
        let enabled = Rc::new(Cell::new(true));
        let checkboxes = Rc::new(Cell::new(false));
        let tri_state = Rc::new(Cell::new(false));
        let guides = Rc::new(Cell::new(true));
        let mappers = Rc::new(Mappers::new());

        {
            let state = Rc::clone(&state);
            let selected = Rc::clone(&selected);
            let hover = Rc::clone(&hover);
            let enabled = Rc::clone(&enabled);
            let checkboxes = Rc::clone(&checkboxes);
            let guides = Rc::clone(&guides);
            let theme = ui.theme_handle();
            let outline = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                paint::paint(
                    canvas,
                    &state.borrow(),
                    &theme.get(),
                    paint::Options {
                        current: selected.get(),
                        hover: hover.get(),
                        enabled: enabled.get(),
                        checkboxes: checkboxes.get(),
                        outline: outline.get(),
                        guides: guides.get(),
                    },
                );
            }));
        }
        {
            let input = events::Input {
                ui: ui.clone(),
                id: control.id(),
                state: Rc::clone(&state),
                selected: Rc::clone(&selected),
                hover: Rc::clone(&hover),
                enabled: Rc::clone(&enabled),
                checkboxes: Rc::clone(&checkboxes),
                tri_state: Rc::clone(&tri_state),
                mappers: Rc::clone(&mappers),
            };
            control.on_events(move |event| input.handle(event));
        }

        Ok(TreeView {
            control,
            state,
            selected,
            hover,
            enabled,
            checkboxes,
            tri_state,
            guides,
            mappers,
        })
    }

    /// Maps selecting a row (by node id; the row index for flat rows) to the
    /// app's message.
    pub fn on_select(self, mapper: impl Fn(NodeId) -> Option<M> + 'static) -> TreeView<M> {
        *self.mappers.select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps toggling a row and its new expanded state to the app's message.
    pub fn on_toggle(self, mapper: impl Fn(NodeId, bool) -> Option<M> + 'static) -> TreeView<M> {
        *self.mappers.toggle.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a row's checkbox changing to the app's message.
    pub fn on_check(
        self,
        mapper: impl Fn(NodeId, CheckState) -> Option<M> + 'static,
    ) -> TreeView<M> {
        *self.mappers.check.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a right click (or the Menu key) on a row to the app's message. The
    /// pointer position is in node-local device pixels, so the app can anchor
    /// its context menu there. For the keyboard key, it is the row's
    /// bottom-left corner.
    pub fn on_context(self, mapper: impl Fn(NodeId, Point) -> Option<M> + 'static) -> TreeView<M> {
        *self.mappers.context.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Shows or hides a checkbox on every row.
    pub fn checkboxes(self, checkboxes: bool) -> TreeView<M> {
        self.checkboxes.set(checkboxes);
        self.control.invalidate();
        self
    }

    /// Whether a checkbox click also visits [`CheckState::Indeterminate`].
    pub fn tri_state(self, tri_state: bool) -> TreeView<M> {
        self.tri_state.set(tri_state);
        self
    }

    /// Shows or hides the indent guides (the vertical lines under each
    /// ancestor column). A shallow tree, like a two-level navigator, may read
    /// better without them. On by default.
    pub fn indent_guides(self, guides: bool) -> TreeView<M> {
        self.guides.set(guides);
        self.control.invalidate();
        self
    }

    /// The selected row's id, if any.
    pub fn selected(&self) -> Option<NodeId> {
        self.selected.get()
    }

    /// Selects the row `id` without raising an event; an unknown id clears it.
    pub fn select(&self, id: Option<NodeId>) {
        let id = id.filter(|id| self.state.borrow().find(*id).is_some());
        self.selected.set(id);
        self.control.invalidate();
    }

    /// The checkbox state of the row `id`, if the row exists.
    pub fn checked(&self, id: NodeId) -> Option<CheckState> {
        let index = self.state.borrow().find(id)?;
        Some(self.state.borrow().rows[index].checked)
    }

    /// Sets the checkbox state of the row `id` without raising an event.
    pub fn set_checked(&self, id: NodeId, state: CheckState) {
        let index = self.state.borrow().find(id);
        if let Some(index) = index {
            self.state.borrow_mut().rows[index].checked = state;
            self.control.invalidate();
        }
    }

    /// Replaces the tree with flat `rows`, dropping a model and any selection
    /// whose id no longer exists.
    pub fn set_rows(&self, rows: &[TreeRow]) {
        *self.state.borrow_mut() = State::flat(rows);
        self.hover.set(None);
        let keep = self
            .selected
            .get()
            .filter(|id| self.state.borrow().find(*id).is_some());
        self.selected.set(keep);
        self.control.invalidate();
    }

    /// Replaces the tree with a model, reading its roots now and clearing the
    /// selection.
    pub fn set_model(&self, model: impl TreeModel + 'static) {
        *self.state.borrow_mut() = State::model(Rc::new(model));
        self.selected.set(None);
        self.hover.set(None);
        self.control.invalidate();
    }

    /// The number of materialized rows, hidden descendants included.
    pub fn len(&self) -> usize {
        self.state.borrow().rows.len()
    }

    /// Whether the tree has no materialized rows.
    pub fn is_empty(&self) -> bool {
        self.state.borrow().rows.is_empty()
    }

    /// The tree's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the tree; a disabled tree is dimmed and ignores input.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the tree selected, so its painter draws an outline.
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for TreeView<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "selected",
            value: Value::Integer(self.selected().map_or(-1, |id| id as i64)),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        if name != "selected" {
            return false;
        }
        if let Value::Integer(id) = value {
            self.select((id >= 0).then_some(id as usize));
            return true;
        }
        false
    }
}
