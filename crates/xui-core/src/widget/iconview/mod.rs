#![forbid(unsafe_code)]

//! [`IconView`]: a virtual, cross-platform Windows XP-style icon view.
//!
//! Each item is a tile with an icon on the left and up to three lines of text
//! on the right. Tiles flow left to right and wrap, and the view scrolls
//! vertically behind its own scrollbar. Like [`ListView`](super::ListView) it
//! stores no items of its own when built with [`with_model`](IconView::with_model):
//! it asks the [`IconModel`] for the visible tiles only, so a model of 100 000
//! items costs the same to paint as ten.
//!
//! Events map to the app's `Msg` through the closures given at construction:
//! [`on_select`](IconView::on_select)/[`on_selection`](IconView::on_selection),
//! [`on_activate`](IconView::on_activate) and
//! [`on_context`](IconView::on_context). A left click selects, a double click or
//! Return activates, and a right click selects an unselected tile and reports
//! the pointer position.

use std::cell::RefCell;
use std::rc::Rc;

use super::Orientation;
use super::control::Control;
use super::placeable::Placeable;
use super::scrollbar::{self, ScrollBar};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::{Point, Rect, Size};
use crate::theme::{Theme, Themed};
use crate::units::Dip;

mod api;
mod bar;
mod events;
mod keyboard;
mod layout;
mod metrics;
mod model;
mod paint;
mod state;

#[cfg(test)]
mod tests;

use self::bar::{layout as bar_layout, mapper as bar_mapper, metrics as bar_metrics};
pub use self::model::{IconModel, IconSize};
use self::state::State;

/// The natural size of a view with nothing to size it.
const NATURAL_WIDTH: Dip = Dip(420.0);
const NATURAL_HEIGHT: Dip = Dip(240.0);

/// Maps an item index to the app's message.
type IndexMapper<M> = Box<dyn Fn(usize) -> Option<M>>;
/// Maps a right click's item (or `None`) and pointer position (node-local
/// pixels) to the app's message.
type ContextMapper<M> = Box<dyn Fn(Option<usize>, Point) -> Option<M>>;
/// Maps the whole selection to the app's message.
type SelectionMapper<M> = Box<dyn Fn(&[usize]) -> Option<M>>;

/// The app-level events an [`IconView`] maps to `Msg`.
pub(crate) struct Mappers<M> {
    pub(crate) select: RefCell<Option<IndexMapper<M>>>,
    pub(crate) selection: RefCell<Option<SelectionMapper<M>>>,
    pub(crate) activate: RefCell<Option<IndexMapper<M>>>,
    pub(crate) context: RefCell<Option<ContextMapper<M>>>,
}

impl<M> Mappers<M> {
    fn new() -> Mappers<M> {
        Mappers {
            select: RefCell::new(None),
            selection: RefCell::new(None),
            activate: RefCell::new(None),
            context: RefCell::new(None),
        }
    }
}

/// A virtual grid of icon tiles.
///
/// The selection and activation map to the app's `Msg` through
/// [`on_select`](IconView::on_select)/[`on_selection`](IconView::on_selection)
/// and [`on_activate`](IconView::on_activate); a right click maps through
/// [`on_context`](IconView::on_context) with its item and pointer position.
pub struct IconView<M: 'static> {
    control: Control<M>,
    _bar: Control<M>,
    bar: Rc<ScrollBar>,
    state: Rc<RefCell<State>>,
    mappers: Rc<Mappers<M>>,
}

impl<M: 'static> IconView<M> {
    /// Creates a view of plain names at `bounds`, with the first item selected
    /// (or no selection when empty). Each name is a one-line tile.
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<IconView<M>> {
        let model: Vec<String> = items.iter().map(|item| item.to_string()).collect();
        Self::build(ui, bounds, Rc::new(model))
    }

    /// Creates a view backed by `model` at `bounds`. The model can be replaced
    /// later with [`set_model`](IconView::set_model).
    pub fn with_model(
        ui: &Ui<M>,
        bounds: Rect,
        model: impl IconModel + 'static,
    ) -> Result<IconView<M>> {
        Self::build(ui, bounds, Rc::new(model))
    }

    fn build(ui: &Ui<M>, bounds: Rect, model: Rc<dyn IconModel>) -> Result<IconView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::ListView, bounds).tab_stop())?;
        let bar_control = Control::new(
            &ui.with_parent(control.id()),
            &NodeSpec::new(NodeKind::Container, Rect::default()),
        )?;
        let state = Rc::new(RefCell::new(State::new(model)));
        let mappers = Rc::new(Mappers::new());
        let bar = Rc::new(ScrollBar::new(bar_control.id()));

        {
            let state = Rc::clone(&state);
            let theme = ui.theme_handle();
            let outline = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let state = state.borrow();
                paint::paint(canvas, &state, &theme, outline.get());
            }));
        }
        {
            let state = Rc::clone(&state);
            let id = control.id();
            let scoped = ui.clone();
            let theme = ui.theme_handle();
            bar_control.set_painter(Rc::new(move |canvas| {
                let metrics = bar_metrics(&scoped, id, &state.borrow());
                scrollbar::paint(canvas, metrics, Orientation::Vertical, theme.get());
            }));
        }
        {
            let mapper = events::mapper(
                ui.clone(),
                control.id(),
                Rc::clone(&state),
                Rc::clone(&mappers),
                Rc::clone(&bar),
            );
            control.on_events(mapper);
        }
        {
            let mapper = bar_mapper(ui.clone(), control.id(), Rc::clone(&bar), Rc::clone(&state));
            bar_control.on_events(mapper);
        }
        bar_layout(ui, control.id(), &bar, &state.borrow());
        ui.raise(bar.id());

        Ok(IconView {
            control,
            _bar: bar_control,
            bar,
            state,
            mappers,
        })
    }
}

impl<M: 'static> Placeable<M> for IconView<M> {
    fn id(&self) -> WidgetId {
        self.control.id()
    }

    fn natural_size(&self, _ui: &Ui<M>, dpi: u32) -> Size {
        Size::new(
            NATURAL_WIDTH.to_px(dpi).value(),
            NATURAL_HEIGHT.to_px(dpi).value(),
        )
    }

    fn placed(&self, ui: &Ui<M>, _rect: Rect) {
        // A layout moves only the view's own node; the scrollbar is a child
        // node, so re-lay it out against the view's new bounds and drop any
        // scroll the smaller viewport can no longer hold.
        let bounds = ui.bounds(self.control.id());
        self.state
            .borrow_mut()
            .clamp_offset(bounds, self.control.dpi());
        bar_layout(ui, self.control.id(), &self.bar, &self.state.borrow());
    }
}

impl<M: 'static> Themed for IconView<M> {
    fn apply_theme(&self, _theme: &Theme) {
        // The painter reads the live theme tokens, so a repaint is enough.
        self.control.invalidate();
    }
}
