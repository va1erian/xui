#![forbid(unsafe_code)]

//! [`GridView`]: a virtualised grid of tiles over a [`GridModel`].
//!
//! The grid stores no tiles of its own: it asks the [`GridModel`] for the
//! visible ones only, so a model of any size costs the same to paint. Each
//! tile is drawn by a custom painter set with
//! [`on_paint_tile`](GridView::on_paint_tile) — art, text, whatever the app
//! wants — or by the default painter, which draws the tile's image and label.
//!
//! The selection and activation map to the app's `Msg` through the closures
//! given at construction: [`on_select`](GridView::on_select) and
//! [`on_activate`](GridView::on_activate). The view handles hit-testing,
//! selection, hover, arrow-key navigation and wheel scrolling itself.

use std::cell::RefCell;
use std::rc::Rc;

use super::control::Control;
use crate::app::Ui;
use crate::backend::{Canvas, NodeKind, NodeSpec, Result};
use crate::geometry::Rect;

mod api;
mod events;
mod layout;
mod model;
mod paint;
mod place;
mod state;

#[cfg(test)]
mod tests;

pub use self::model::{GridModel, Tile, TilePaint, TileSize};
use self::state::State;

/// Maps a tile index to the app's message.
type IndexMapper<M> = Box<dyn Fn(usize) -> Option<M>>;

/// Draws one tile; see [`GridView::on_paint_tile`].
pub(crate) type TilePainter = Box<dyn for<'a> Fn(&mut dyn Canvas, &TilePaint<'a>)>;

/// The app-level events a [`GridView`] maps to `Msg`, plus its tile painter.
pub(crate) struct Mappers<M> {
    pub(crate) select: RefCell<Option<IndexMapper<M>>>,
    pub(crate) activate: RefCell<Option<IndexMapper<M>>>,
    pub(crate) painter: RefCell<Option<TilePainter>>,
}

/// A virtualised grid of tiles.
///
/// Create one with [`with_model`](GridView::with_model) (or
/// [`new`](GridView::new) for plain labels), then set its size with
/// [`tile_size`](GridView::tile_size), map its events and optionally replace
/// the tile painter with [`on_paint_tile`](GridView::on_paint_tile).
pub struct GridView<M: 'static> {
    control: Control<M>,
    state: Rc<RefCell<State>>,
    mappers: Rc<Mappers<M>>,
}

impl<M: 'static> GridView<M> {
    /// Creates a virtual grid backed by `model` at `bounds`, with the first
    /// tile selected (or no selection when empty).
    pub fn with_model(
        ui: &Ui<M>,
        bounds: Rect,
        model: impl GridModel + 'static,
    ) -> Result<GridView<M>> {
        Self::build(ui, bounds, Rc::new(model))
    }

    /// Creates a grid of plain labels backed by `items`, with the first selected.
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<GridView<M>> {
        let model: Vec<String> = items.iter().map(|item| item.to_string()).collect();
        Self::build(ui, bounds, Rc::new(model))
    }

    fn build(ui: &Ui<M>, bounds: Rect, model: Rc<dyn GridModel>) -> Result<GridView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, bounds).tab_stop())?;
        let state = Rc::new(RefCell::new(State::new(model)));
        let mappers = Rc::new(Mappers {
            select: RefCell::new(None),
            activate: RefCell::new(None),
            painter: RefCell::new(None),
        });

        {
            let state = Rc::clone(&state);
            let mappers = Rc::clone(&mappers);
            let theme = ui.theme_handle();
            let outline = control.selected_handle();
            control.set_painter(Rc::new(move |canvas| {
                let theme = theme.get();
                let state = state.borrow();
                let painter = mappers.painter.borrow();
                paint::paint(canvas, &state, &theme, &painter, outline.get());
            }));
        }
        {
            let mapper = events::mapper(
                ui.clone(),
                control.id(),
                Rc::clone(&state),
                Rc::clone(&mappers),
            );
            control.on_events(mapper);
        }

        Ok(GridView {
            control,
            state,
            mappers,
        })
    }

    /// Sets the tile size (and gap); the columns reflow to the new width.
    pub fn tile_size(self, size: impl Into<TileSize>) -> GridView<M> {
        self.set_tile_size(size);
        self
    }

    /// Maps selecting a tile (a click or an arrow key) to the app's message.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> GridView<M> {
        *self.mappers.select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps activating a tile (Return, Space or a double-click) to the app's
    /// message.
    pub fn on_activate(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> GridView<M> {
        *self.mappers.activate.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Replaces the default painter with `painter`, called once per visible
    /// tile. The widget has already filled the tile's selection/hover
    /// background when it is called.
    pub fn on_paint_tile(
        self,
        painter: impl Fn(&mut dyn Canvas, &TilePaint<'_>) + 'static,
    ) -> GridView<M> {
        *self.mappers.painter.borrow_mut() = Some(Box::new(painter));
        self.control.invalidate();
        self
    }
}
