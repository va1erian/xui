#![forbid(unsafe_code)]

//! The [`ListView`](super::ListView) runtime API: models, selection, scrolling
//! and the sort indicator.

use std::collections::BTreeSet;

use super::state::{ROW, Rows, header_px};
use super::{CellData, ListModel, ListView, SortDirection, bar};
use crate::backend::WidgetId;
use crate::property::{Properties, Property, Value};

impl<M: 'static> ListView<M> {
    /// Raises the list above its siblings together with its scrollbar. Raising
    /// only the list would draw it over its own scrollbar, so use this when a
    /// surface stacks the list above other nodes.
    pub fn raise(&self) {
        let ui = self.control.ui();
        ui.raise(self.control.id());
        ui.raise(self.bar.id());
    }

    /// Replaces the rows with a virtual `model` and refreshes the view. The
    /// selection is kept where it still exists.
    pub fn set_model(&self, model: impl ListModel + 'static) {
        let rows = Rows::Model(std::rc::Rc::new(model));
        self.replace_rows(rows);
    }

    /// Replaces the rows with a fresh read of the same data (a live table):
    /// unlike [`ListView::set_model`], the scroll position, the selection and
    /// the focus are kept where the new rows still have them, and nothing is
    /// selected that was not, so a periodic refresh does not move the view.
    pub fn refresh_model(&self, model: impl ListModel + 'static) {
        let (offset, selected, focused, anchor) = {
            let state = self.state.borrow();
            (
                state.offset,
                state.selected.clone(),
                state.focused,
                state.anchor,
            )
        };
        self.set_model(model);
        {
            let mut state = self.state.borrow_mut();
            let len = state.rows.len();
            state.selected = selected.into_iter().filter(|row| *row < len).collect();
            state.focused = focused.filter(|row| *row < len);
            state.anchor = anchor.filter(|row| *row < len);
            state.offset = offset.min(len.saturating_sub(1));
        }
        bar::layout(
            self.control.ui(),
            self.control.id(),
            &self.bar,
            &self.state.borrow(),
        );
        self.control.invalidate();
    }

    /// Replaces the rows with plain text. A selection past the new end moves
    /// to the last row.
    pub fn set_items(&self, items: &[&str]) {
        let rows = Rows::Simple(items.iter().map(|item| item.to_string()).collect());
        self.replace_rows(rows);
    }

    fn replace_rows(&self, rows: Rows) {
        let len = rows.len();
        let mut state = self.state.borrow_mut();
        state.rows = rows;
        state.selected.retain(|row| *row < len);
        if state.focused.is_none_or(|row| row >= len) {
            state.focused = if len == 0 { None } else { Some(len - 1) };
        }
        if state.selected.is_empty()
            && let Some(focused) = state.focused
        {
            state.selected.insert(focused);
        }
        state.anchor = state.focused;
        state.hover = None;
        state.offset = 0;
        drop(state);
        bar::layout(
            self.control.ui(),
            self.control.id(),
            &self.bar,
            &self.state.borrow(),
        );
        self.control.invalidate();
    }

    /// The first selected row, if any.
    pub fn selected(&self) -> Option<usize> {
        self.state.borrow().selected.iter().next().copied()
    }

    /// Every selected row, ascending.
    pub fn selection(&self) -> Vec<usize> {
        self.state.borrow().selection()
    }

    /// The row the keyboard acts on.
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

    /// Makes `rows` the selection without raising an event. Out-of-range and
    /// duplicate rows are dropped; the first row becomes the focus.
    pub fn set_selection(&self, rows: &[usize]) {
        let len = self.state.borrow().len();
        let mut next = BTreeSet::new();
        for row in rows.iter().copied().filter(|row| *row < len) {
            next.insert(row);
        }
        let focused = next.iter().next().copied();
        let mut state = self.state.borrow_mut();
        state.selected = next;
        state.focused = focused;
        state.anchor = focused;
        drop(state);
        self.control.invalidate();
    }

    /// Scrolls `row` into view.
    pub fn ensure_visible(&self, row: usize) {
        let dpi = self.control.dpi();
        let header = self.state.borrow().has_header();
        let bounds = self.control.bounds();
        let body = (bounds.height() - header_px(header, dpi)).max(0);
        let visible = ((body / ROW.to_px(dpi).value().max(1)) as usize).max(1);
        self.state.borrow_mut().ensure_visible(row, visible);
        self.control.invalidate();
        self.control.ui().invalidate(self.bar.id());
    }

    /// Shows a sort arrow on `column` without raising an event.
    pub fn set_sort_indicator(&self, column: usize, direction: SortDirection) {
        self.state.borrow_mut().sort = Some((column, direction));
        self.control.invalidate();
    }

    /// Removes the sort arrow from `column`.
    pub fn clear_sort_indicator(&self, column: usize) {
        let mut state = self.state.borrow_mut();
        if state.sort.is_some_and(|(sorted, _)| sorted == column) {
            state.sort = None;
            drop(state);
            self.control.invalidate();
        }
    }

    /// The column the sort arrow is on, with its direction.
    pub fn sort_indicator(&self) -> Option<(usize, SortDirection)> {
        self.state.borrow().sort
    }

    /// The number of columns.
    pub fn column_count(&self) -> usize {
        self.state.borrow().columns.len()
    }

    /// Sets a column's width without raising an event; out-of-range columns are
    /// ignored. Pass [`Fill`](super::Fill) to make the column share the
    /// leftover space again, or a design value to fix it.
    pub fn set_column_width(&self, column: usize, width: impl Into<super::ColumnWidth>) {
        let width = width.into();
        let mut state = self.state.borrow_mut();
        let Some(column) = state.columns.get_mut(column) else {
            return;
        };
        column.width = width;
        drop(state);
        self.control.invalidate();
    }

    /// A column's width spec, `None` when `column` is out of range.
    pub fn column_width(&self, column: usize) -> Option<super::ColumnWidth> {
        self.state.borrow().columns.get(column).map(|cl| cl.width)
    }

    /// Reads back a cell's text. Allocates; the paint path borrows instead.
    pub fn cell_text(&self, row: usize, column: usize) -> Option<String> {
        self.state
            .borrow()
            .rows
            .cell(row, column)
            .map(str::to_string)
    }

    /// Reads back a cell's opaque payload, if the model attached one.
    pub fn cell_data(&self, row: usize, column: usize) -> Option<CellData> {
        self.state.borrow().rows.data(row, column)
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.state.borrow().len()
    }

    /// Whether the list has no rows.
    pub fn is_empty(&self) -> bool {
        self.state.borrow().len() == 0
    }

    /// The list's node identity.
    pub fn id(&self) -> WidgetId {
        self.control.id()
    }

    /// Enables or disables the list. A disabled list is dimmed and ignores
    /// input.
    pub fn set_enabled(&self, enabled: bool) {
        self.state.borrow_mut().enabled = enabled;
        self.control.set_enabled(enabled);
        self.control.invalidate();
    }

    /// Marks the list selected, so its painter draws an outline (a form
    /// editor's selection).
    pub fn set_selected(&self, selected: bool) {
        self.control.set_selected(selected);
    }
}

impl<M: 'static> Properties for ListView<M> {
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
