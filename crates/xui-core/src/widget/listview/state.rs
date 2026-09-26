#![forbid(unsafe_code)]

//! The list view's private state, plus the row/column arithmetic its painter
//! and its event mapper share.

use std::collections::BTreeSet;
use std::rc::Rc;

use super::model::{CellData, Column, ColumnWidth, ListModel, SelectionMode, SortDirection};
use crate::units::Dip;

/// The design height of one row.
pub(crate) const ROW: Dip = Dip(22.0);
/// The design height of the header row.
pub(crate) const HEADER: Dip = Dip(24.0);
/// The design size of row and header text.
pub(crate) const TEXT_SIZE: Dip = Dip(12.0);
/// The horizontal text inset inside a cell.
pub(crate) const PADDING: Dip = Dip(6.0);
/// A `Fill` column never collapses below this design width.
const MIN_FILL: Dip = Dip(48.0);
/// Rows scrolled per wheel notch.
pub(crate) const WHEEL_ROWS: usize = 3;

/// The list's backing rows: literal text or a virtual model.
pub(crate) enum Rows {
    /// One text per row, drawn in a single full-width column.
    Simple(Vec<String>),
    /// A virtual model, read one visible cell at a time.
    Model(Rc<dyn ListModel>),
}

impl Rows {
    pub(crate) fn len(&self) -> usize {
        match self {
            Rows::Simple(items) => items.len(),
            Rows::Model(model) => model.rows(),
        }
    }

    pub(crate) fn cell(&self, row: usize, column: usize) -> Option<&str> {
        match self {
            Rows::Simple(items) => (column == 0)
                .then(|| items.get(row).map(String::as_str))
                .flatten(),
            Rows::Model(model) => model.cell(row, column),
        }
    }

    pub(crate) fn data(&self, row: usize, column: usize) -> Option<CellData> {
        match self {
            Rows::Simple(_) => None,
            Rows::Model(model) => model.data(row, column),
        }
    }
}

/// Everything the list view reads on paint and writes on input.
pub(crate) struct State {
    pub(crate) rows: Rows,
    pub(crate) columns: Vec<Column>,
    pub(crate) mode: SelectionMode,
    /// Selected rows, ascending and unique.
    pub(crate) selected: BTreeSet<usize>,
    /// The row the keyboard acts on.
    pub(crate) focused: Option<usize>,
    /// The row a Shift range is measured from.
    pub(crate) anchor: Option<usize>,
    pub(crate) hover: Option<usize>,
    /// The first row drawn, i.e. the scroll position.
    pub(crate) offset: usize,
    /// `(column, direction)` of the header sort arrow.
    pub(crate) sort: Option<(usize, SortDirection)>,
    pub(crate) enabled: bool,
}

impl State {
    /// The number of rows.
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether a header row is drawn.
    pub(crate) fn has_header(&self) -> bool {
        !self.columns.is_empty()
    }

    /// Every selected row, ascending.
    pub(crate) fn selection(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// The row messages and keyboard act on: the focused row, else the first
    /// selected one.
    pub(crate) fn primary(&self) -> Option<usize> {
        self.focused
            .or_else(|| self.selected.iter().next().copied())
    }

    /// Replaces the selection with `row`.
    pub(crate) fn set_single(&mut self, row: usize) {
        self.selected.clear();
        self.selected.insert(row);
        self.focused = Some(row);
        self.anchor = Some(row);
    }

    /// Toggles `row` in or out of the selection (Ctrl+click).
    pub(crate) fn toggle(&mut self, row: usize) {
        if !self.selected.remove(&row) {
            self.selected.insert(row);
        }
        self.focused = Some(row);
        self.anchor = Some(row);
    }

    /// Replaces the selection with the range from the anchor to `row`
    /// (Shift+click/arrow).
    pub(crate) fn extend(&mut self, row: usize) {
        let anchor = self.anchor.unwrap_or(row);
        let (low, high) = if anchor <= row {
            (anchor, row)
        } else {
            (row, anchor)
        };
        self.selected = (low..=high).collect();
        self.focused = Some(row);
        if self.anchor.is_none() {
            self.anchor = Some(row);
        }
    }

    /// Scrolls just enough that `row` is fully visible in a body `visible`
    /// rows tall.
    pub(crate) fn ensure_visible(&mut self, row: usize, visible: usize) {
        let visible = visible.max(1);
        if row < self.offset {
            self.offset = row;
        } else if row >= self.offset + visible {
            self.offset = row + 1 - visible;
        }
    }
}

/// The header height in device pixels (`0` when there is no header).
pub(crate) fn header_px(has_header: bool, dpi: u32) -> i32 {
    if has_header {
        HEADER.to_px(dpi).value()
    } else {
        0
    }
}

/// The raw row index under node-local `y`, accounting for the header and the
/// scroll offset. `None` above the body or past the last row.
pub(crate) fn row_at(
    row_px: i32,
    offset: usize,
    count: usize,
    header: i32,
    y: i32,
) -> Option<usize> {
    if y < header {
        return None;
    }
    let slot = ((y - header) / row_px.max(1)) as usize;
    let row = offset + slot;
    (row < count).then_some(row)
}

/// The device-pixel width of each column inside `total` px: a fixed column
/// keeps its design width and the `Fill` columns share the leftover equally.
pub(crate) fn column_widths(dpi: u32, total: i32, columns: &[Column]) -> Vec<i32> {
    let fixed = |width: ColumnWidth| match width {
        ColumnWidth::Fixed(design) => design.to_px(dpi).value(),
        ColumnWidth::Fill => 0,
    };
    let fills = columns
        .iter()
        .filter(|column| column.width == ColumnWidth::Fill)
        .count();
    if fills == 0 {
        return columns.iter().map(|column| fixed(column.width)).collect();
    }
    let used: i32 = columns.iter().map(|column| fixed(column.width)).sum();
    let min = MIN_FILL.to_px(dpi).value();
    let each = ((total - used).max(0) / fills as i32).max(min);
    columns
        .iter()
        .map(|column| match column.width {
            ColumnWidth::Fill => each,
            width => fixed(width),
        })
        .collect()
}

/// The left edge of each column, given its width.
pub(crate) fn column_spans(widths: &[i32]) -> Vec<(i32, i32)> {
    let mut left = 0;
    widths
        .iter()
        .map(|width| {
            let span = (left, left + width);
            left += width;
            span
        })
        .collect()
}

/// The column containing node-local `x`, if any.
pub(crate) fn column_at(widths: &[i32], x: i32) -> Option<usize> {
    let mut left = 0;
    for (index, width) in widths.iter().enumerate() {
        if x >= left && x < left + width {
            return Some(index);
        }
        left += width;
    }
    None
}

/// The sort `(column, direction)` a header click on `column` produces: the
/// same column flips direction, a new column starts ascending.
pub(crate) fn toggled_sort(
    current: Option<(usize, SortDirection)>,
    column: usize,
) -> (usize, SortDirection) {
    match current {
        Some((sorted, SortDirection::Ascending)) if sorted == column => {
            (column, SortDirection::Descending)
        }
        _ => (column, SortDirection::Ascending),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::listview::model::{Column, Fill};

    #[test]
    fn fill_columns_share_the_leftover_equally() {
        let columns = [
            Column::new("A", Dip::new(100.0)),
            Column::new("B", Fill),
            Column::new("C", Fill),
        ];
        assert_eq!(column_widths(96, 300, &columns), vec![100, 100, 100]);
        assert_eq!(column_widths(96, 400, &columns), vec![100, 150, 150]);
    }

    #[test]
    fn fixed_widths_convert_at_the_dpi() {
        let columns = [Column::new("A", Dip::new(100.0))];
        assert_eq!(column_widths(96, 300, &columns), vec![100]);
        assert_eq!(column_widths(192, 300, &columns), vec![200]);
    }

    #[test]
    fn spans_and_hits_line_up_with_the_widths() {
        let widths = [100, 150, 50];
        assert_eq!(
            column_spans(&widths),
            vec![(0, 100), (100, 250), (250, 300)]
        );
        assert_eq!(column_at(&widths, 0), Some(0));
        assert_eq!(column_at(&widths, 249), Some(1));
        assert_eq!(column_at(&widths, 250), Some(2));
        assert_eq!(column_at(&widths, 300), None);
    }

    #[test]
    fn rows_honour_the_header_and_scroll() {
        assert_eq!(row_at(22, 0, 10, 24, 23), None);
        assert_eq!(row_at(22, 0, 10, 24, 24), Some(0));
        assert_eq!(row_at(22, 0, 10, 24, 24 + 22), Some(1));
        assert_eq!(row_at(22, 3, 10, 24, 24), Some(3));
        assert_eq!(row_at(22, 0, 2, 24, 24 + 2 * 22), None);
    }

    #[test]
    fn a_second_header_click_flips_the_sort() {
        assert_eq!(toggled_sort(None, 2), (2, SortDirection::Ascending));
        assert_eq!(
            toggled_sort(Some((2, SortDirection::Ascending)), 2),
            (2, SortDirection::Descending)
        );
        assert_eq!(
            toggled_sort(Some((2, SortDirection::Descending)), 2),
            (2, SortDirection::Ascending)
        );
        assert_eq!(
            toggled_sort(Some((2, SortDirection::Ascending)), 5),
            (5, SortDirection::Ascending)
        );
    }
}
