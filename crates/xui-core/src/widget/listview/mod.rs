#![forbid(unsafe_code)]

//! [`ListView`]: a virtual report list with columns, a header, a sort arrow,
//! single/multi/range selection and a context hook.
//!
//! The list stores no rows of its own when one is installed with
//! [`ListView::with_model`]: it asks the [`ListModel`] for the text of the
//! visible cells only, so a model of any size costs the same to paint. The
//! simple [`ListView::new`] constructor keeps a plain `&[&str]` list working.
//!
//! Events map to the app's `Msg` through the closures given at construction:
//! [`on_select`](ListView::on_select)/[`on_selection`](ListView::on_selection),
//! [`on_activate`](ListView::on_activate),
//! [`on_context`](ListView::on_context) (which receives the pointer position),
//! [`on_sort`](ListView::on_sort) and [`on_resize`](ListView::on_resize). A
//! column boundary in the header can be dragged to resize the column.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::control::Control;
use super::placeable::Placeable;
use super::scrollbar::{self, Bar};
use crate::app::Ui;
use crate::backend::{NodeKind, NodeSpec, Result, WidgetId};
use crate::geometry::{Point, Rect, Size};
use crate::units::Dip;

mod api;
mod bar;
mod ellipsis;
mod events;
mod model;
mod paint;
mod resize;
mod state;

#[cfg(test)]
mod tests;

pub use self::model::{
    CellData, Column, ColumnWidth, Fill, ListModel, SelectionMode, SortDirection,
};
use self::state::{Rows, State};

/// The natural size of a list with nothing to size it; a `fill` entry overrides
/// the axis it fills.
const NATURAL_WIDTH: Dip = Dip(360.0);
const NATURAL_HEIGHT: Dip = Dip(200.0);

/// Maps a row index to the app's message.
type RowMapper<M> = Box<dyn Fn(usize) -> Option<M>>;
/// Maps a right click's row and pointer position (node-local pixels) to the
/// app's message.
type ContextMapper<M> = Box<dyn Fn(usize, Point) -> Option<M>>;
/// Maps the whole selection to the app's message.
type SelectionMapper<M> = Box<dyn Fn(&[usize]) -> Option<M>>;
/// Maps a finished column resize to the app's message.
type ResizeMapper<M> = Box<dyn Fn(usize, Dip) -> Option<M>>;

/// The app-level events a [`ListView`] maps to `Msg`.
pub(crate) struct Mappers<M> {
    pub(crate) select: RefCell<Option<RowMapper<M>>>,
    pub(crate) selection: RefCell<Option<SelectionMapper<M>>>,
    pub(crate) activate: RefCell<Option<RowMapper<M>>>,
    pub(crate) context: RefCell<Option<ContextMapper<M>>>,
    pub(crate) sort: RefCell<Option<RowMapper<M>>>,
    pub(crate) resize: RefCell<Option<ResizeMapper<M>>>,
}

impl<M> Mappers<M> {
    fn new() -> Mappers<M> {
        Mappers {
            select: RefCell::new(None),
            selection: RefCell::new(None),
            activate: RefCell::new(None),
            context: RefCell::new(None),
            sort: RefCell::new(None),
            resize: RefCell::new(None),
        }
    }
}

/// A list of rows with a header, columns and a selection.
///
/// The selection and activation map to the app's `Msg` through
/// [`on_select`](ListView::on_select) and
/// [`on_activate`](ListView::on_activate); a header click maps through
/// [`on_sort`](ListView::on_sort), dragging a header boundary resizes the
/// column and maps through [`on_resize`](ListView::on_resize), and a right
/// click maps through [`on_context`](ListView::on_context) with its pointer
/// position.
pub struct ListView<M: 'static> {
    control: Control<M>,
    _bar: Control<M>,
    bar: Rc<Bar>,
    state: Rc<RefCell<State>>,
    mappers: Rc<Mappers<M>>,
}

impl<M: 'static> ListView<M> {
    /// Creates a list of `items` at `bounds`, with the first row selected (or
    /// no selection when empty). It has one full-width column and no header.
    pub fn new(ui: &Ui<M>, bounds: Rect, items: &[&str]) -> Result<ListView<M>> {
        let rows = Rows::Simple(items.iter().map(|item| item.to_string()).collect());
        Self::build(ui, bounds, rows)
    }

    /// Creates a virtual list with no bounds of its own, for a layout to place
    /// (see [`crate::arrange`]); add columns with [`column`](ListView::column).
    /// The model can be replaced later with [`set_model`](ListView::set_model).
    pub fn auto(ui: &Ui<M>, model: impl ListModel + 'static) -> Result<ListView<M>> {
        Self::with_model(ui, Rect::default(), model)
    }

    /// Creates a virtual list backed by `model` at `bounds`. Add columns with
    /// [`column`](ListView::column)/[`add_column`](ListView::add_column); a
    /// list with at least one column draws a header.
    pub fn with_model(
        ui: &Ui<M>,
        bounds: Rect,
        model: impl ListModel + 'static,
    ) -> Result<ListView<M>> {
        Self::build(ui, bounds, Rows::Model(Rc::new(model)))
    }

    fn build(ui: &Ui<M>, bounds: Rect, rows: Rows) -> Result<ListView<M>> {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::ListView, bounds).tab_stop())?;
        let bar_control = Control::new(
            &ui.with_parent(control.id()),
            &NodeSpec::new(NodeKind::Container, Rect::default()),
        )?;
        let mut selected = BTreeSet::new();
        let focused = if rows.len() == 0 {
            None
        } else {
            selected.insert(0);
            Some(0)
        };
        let state = Rc::new(RefCell::new(State {
            rows,
            columns: Vec::new(),
            mode: SelectionMode::Single,
            selected,
            focused,
            anchor: focused,
            hover: None,
            offset: 0,
            sort: None,
            resize: None,
            enabled: true,
        }));
        let mappers = Rc::new(Mappers::new());
        let bar = Rc::new(Bar::new(bar_control.id()));

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
                let metrics = bar::metrics(&scoped, id, &state.borrow());
                scrollbar::paint(canvas, metrics, theme.get());
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
            let mapper = bar::mapper(ui.clone(), control.id(), Rc::clone(&bar), Rc::clone(&state));
            bar_control.on_events(mapper);
        }
        bar::layout(ui, control.id(), &bar, &state.borrow());
        ui.raise(bar.id());

        Ok(ListView {
            control,
            _bar: bar_control,
            bar,
            state,
            mappers,
        })
    }

    /// Adds a left-aligned column of `width`.
    pub fn column(self, title: impl Into<String>, width: impl Into<ColumnWidth>) -> ListView<M> {
        self.add_column(Column::new(title, width))
    }

    /// Adds a right-aligned column of `width`.
    pub fn column_right(
        self,
        title: impl Into<String>,
        width: impl Into<ColumnWidth>,
    ) -> ListView<M> {
        self.add_column(Column::right(title, width))
    }

    /// Adds a pre-built [`Column`], e.g. one that is centred.
    pub fn add_column(self, column: Column) -> ListView<M> {
        self.state.borrow_mut().columns.push(column);
        bar::layout(
            self.control.ui(),
            self.control.id(),
            &self.bar,
            &self.state.borrow(),
        );
        self.control.invalidate();
        self
    }

    /// Sets how clicks and the keyboard select rows.
    pub fn selection_mode(self, mode: SelectionMode) -> ListView<M> {
        self.state.borrow_mut().mode = mode;
        self
    }

    /// A shortcut for [`selection_mode`](ListView::selection_mode): multi
    /// select, or single select.
    pub fn multi_select(self, multi: bool) -> ListView<M> {
        self.selection_mode(if multi {
            SelectionMode::Multi
        } else {
            SelectionMode::Single
        })
    }

    /// Maps selecting the primary row to the app's message. When
    /// [`on_selection`](ListView::on_selection) is set it takes over and this
    /// one is not called.
    pub fn on_select(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> ListView<M> {
        *self.mappers.select.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a selection change to the app's message. The slice holds every
    /// selected row, ascending â€” empty when the selection was cleared.
    pub fn on_selection(self, mapper: impl Fn(&[usize]) -> Option<M> + 'static) -> ListView<M> {
        *self.mappers.selection.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps activating the focused row (Return or a double-click) to the app's
    /// message.
    pub fn on_activate(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> ListView<M> {
        *self.mappers.activate.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a right click (or the Menu key) on a row to the app's message. The
    /// pointer position is in node-local device pixels, so the app can anchor
    /// its context menu there. For the keyboard key, it is the focused row's
    /// bottom-left corner.
    pub fn on_context(self, mapper: impl Fn(usize, Point) -> Option<M> + 'static) -> ListView<M> {
        *self.mappers.context.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a finished column resize to the app's message: the column and its
    /// new design width. Dragging a `Fill` boundary converts that column to a
    /// fixed width, so an app can persist the value back through
    /// [`set_column_width`](ListView::set_column_width).
    pub fn on_resize(self, mapper: impl Fn(usize, Dip) -> Option<M> + 'static) -> ListView<M> {
        *self.mappers.resize.borrow_mut() = Some(Box::new(mapper));
        self
    }

    /// Maps a header click to the app's message. The list toggles its own sort
    /// arrow; the app sorts the model and calls
    /// [`set_sort_indicator`](ListView::set_sort_indicator) for a programmatic
    /// indicator.
    pub fn on_sort(self, mapper: impl Fn(usize) -> Option<M> + 'static) -> ListView<M> {
        *self.mappers.sort.borrow_mut() = Some(Box::new(mapper));
        self
    }
}

impl<M: 'static> Placeable<M> for ListView<M> {
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
        // A layout moves only the list's own node; the scrollbar is a child
        // node, so re-lay it out against the list's new bounds.
        bar::layout(ui, self.control.id(), &self.bar, &self.state.borrow());
    }
}
