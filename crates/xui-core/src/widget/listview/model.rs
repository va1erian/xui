#![forbid(unsafe_code)]

//! The list view's data model and its column, width and selection vocabulary.
//!
//! A [`ListModel`] supplies cell text lazily, so a large model paints only the
//! rows that are visible. [`Column`] specs give the header its titles and
//! widths, [`ColumnWidth`] fixes a width or shares the leftover space, and
//! [`SortDirection`] is the header arrow's direction.

use std::any::Any;
use std::rc::Rc;

use crate::icon::IconRef;
use crate::units::Dip;

/// An opaque per-cell payload a [`ListModel`] can attach to a cell. It is
/// cloned cheaply (an `Rc`) and read back with
/// [`ListView::cell_data`](super::ListView::cell_data); the widget never
/// interprets it.
pub type CellData = Rc<dyn Any>;

/// Supplies a [`ListView`](super::ListView) with rows without storing them in
/// the view itself.
///
/// The model is borrowed for the paint and [`cell`](ListModel::cell) returns a
/// borrowed `&str`, so a virtual list allocates nothing per visible cell.
/// Implement it for the struct that already owns the rows.
pub trait ListModel {
    /// The number of rows.
    fn rows(&self) -> usize;

    /// The text of the cell at `row`/`column`, or `None` for an empty cell.
    fn cell(&self, row: usize, column: usize) -> Option<&str>;

    /// Whether the model holds no rows.
    fn is_empty(&self) -> bool {
        self.rows() == 0
    }

    /// An optional opaque payload attached to a cell, e.g. an id the app's
    /// context handler keys off. `None` by default.
    fn data(&self, _row: usize, _column: usize) -> Option<CellData> {
        None
    }

    /// An optional leading icon for `row`, drawn before the first column's
    /// text in the row's text colour (so it follows the theme, the selection
    /// and the disabled state). `None` by default, and a row without one keeps
    /// its text at the ordinary inset.
    fn icon(&self, _row: usize) -> Option<IconRef> {
        None
    }
}

impl ListModel for Vec<String> {
    fn rows(&self) -> usize {
        Vec::len(self)
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        (column == 0)
            .then(|| self.get(row).map(String::as_str))
            .flatten()
    }
}

impl ListModel for Vec<Vec<String>> {
    fn rows(&self) -> usize {
        Vec::len(self)
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        self.get(row)
            .and_then(|cells| cells.get(column))
            .map(String::as_str)
    }
}

/// A column's width: a fixed design value, or [`Fill`] to share the leftover
/// client width with the other `Fill` columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColumnWidth {
    /// A fixed design value, converted once at the list's DPI.
    Fixed(Dip),
    /// Shares whatever width the fixed columns leave behind.
    Fill,
}

impl From<Dip> for ColumnWidth {
    fn from(width: Dip) -> ColumnWidth {
        ColumnWidth::Fixed(width)
    }
}

/// A [`ColumnWidth`] that shares the leftover client width with the other
/// `Fill` columns; the widths are recomputed on every paint, so a `Fill`
/// column never leaves a gap on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill;

impl From<Fill> for ColumnWidth {
    fn from(_: Fill) -> ColumnWidth {
        ColumnWidth::Fill
    }
}

/// A report-mode column: its header title, width and cell alignment.
///
/// The cell text comes from the [`ListModel`], not from the column, so a
/// column is a plain value (no boxed accessor). Build it with [`Column::new`]
/// or [`Column::right`], then [`ListView::add_column`](super::ListView::add_column).
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// The header title.
    pub title: String,
    /// Fixed width or [`Fill`].
    pub width: ColumnWidth,
    /// Whether the column's cells are right-aligned.
    pub align_right: bool,
    /// Whether the column's cells are centred (ignored when
    /// [`align_right`](Self::align_right) is set).
    pub centered: bool,
}

impl Column {
    /// A left-aligned column of `width`.
    pub fn new(title: impl Into<String>, width: impl Into<ColumnWidth>) -> Column {
        Column {
            title: title.into(),
            width: width.into(),
            align_right: false,
            centered: false,
        }
    }

    /// A right-aligned column (numbers, durations) of `width`.
    pub fn right(title: impl Into<String>, width: impl Into<ColumnWidth>) -> Column {
        Column {
            align_right: true,
            ..Column::new(title, width)
        }
    }

    /// Horizontally centres this column's cells, e.g. a narrow glyph column.
    pub fn centered(mut self) -> Column {
        self.centered = true;
        self
    }
}

/// Which way a sorted column is ordered, for the header arrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending.
    Ascending,
    /// Descending.
    Descending,
}

/// How clicks and keyboard select rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// At most one row; a click replaces the selection.
    #[default]
    Single,
    /// Any number of rows: Ctrl+click toggles a row, Shift+click extends a
    /// range from the anchor.
    Multi,
    /// A single contiguous range: Shift+click (or Shift+arrow) extends from
    /// the anchor; Ctrl is ignored.
    Range,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_strings_are_a_single_column_model() {
        let model: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(model.rows(), 2);
        assert_eq!(model.cell(1, 0), Some("b"));
        assert_eq!(model.cell(1, 1), None);
        assert_eq!(model.cell(2, 0), None);
        assert!(!model.is_empty());
        assert!(Vec::<String>::new().is_empty());
    }

    #[test]
    fn vec_rows_are_a_grid_model() {
        let model: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()], vec!["c".into()]];
        assert_eq!(model.cell(0, 1), Some("b"));
        assert_eq!(model.cell(1, 1), None);
    }

    #[test]
    fn column_widths_convert_from_dip_and_fill() {
        assert_eq!(
            ColumnWidth::from(Dip::new(80.0)),
            ColumnWidth::Fixed(Dip::new(80.0))
        );
        assert_eq!(ColumnWidth::from(Fill), ColumnWidth::Fill);
    }

    #[test]
    fn column_builders_set_alignment() {
        let column = Column::new("Name", Dip::new(80.0));
        assert!(!column.align_right && !column.centered);
        assert!(Column::right("Year", Dip::new(40.0)).align_right);
        assert!(Column::new("Star", Dip::new(20.0)).centered().centered);
    }
}
