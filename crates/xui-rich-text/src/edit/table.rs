#![forbid(unsafe_code)]

//! Table commands: inserting a table, adding and removing rows and columns,
//! changing a table's settings, and moving between cells with Tab.

use std::ops::Range;

use super::controller::{EditorState, Effect};
use super::tx::Cx;
use crate::model::{
    CellMark, CellStart, DocPos, Document, EditError, EditOp, MAX_COLUMNS, MAX_ROWS, ParaStyleId,
    Paragraph, Selection, Slice, Table, TableId, TableSpan,
};

/// Where the caret is in a table, for toolbars.
#[derive(Clone, Debug, PartialEq)]
pub struct TableCursor {
    /// The table.
    pub id: TableId,
    /// The caret's row, from 0.
    pub row: usize,
    /// The caret's column, from 0.
    pub column: usize,
    /// How many rows the table has.
    pub rows: usize,
    /// The table's settings.
    pub table: Table,
}

/// An empty paragraph for a new cell, styled like `like` (a paragraph of the
/// cell it sits beside).
fn empty_cell(doc: &Document, like: usize, cell: CellMark) -> Paragraph {
    let source = &doc.paragraphs()[like];
    Paragraph::new("", source.style(), source.style_at(0)).with_cell(Some(cell))
}

/// The mark of a new cell's first paragraph in `table` at column `col`.
fn mark(table: TableId, col: usize) -> CellMark {
    CellMark {
        table,
        start: if col == 0 {
            CellStart::Row
        } else {
            CellStart::Cell
        },
    }
}

/// Inserts `paras` before paragraph `at`.
fn insert(cx: &mut Cx<'_, '_>, at: usize, paras: Vec<Paragraph>) -> Result<(), EditError> {
    cx.apply(EditOp::InsertParas {
        at,
        content: Slice::paras(paras),
    })
}

/// The first paragraph of the cell at `row` and `col` of the table starting at
/// paragraph `start`, after an edit.
fn cell_start(doc: &Document, start: usize, row: usize, col: usize) -> Option<DocPos> {
    let span = doc.table_at(start)?;
    let row = row.min(span.rows.len() - 1);
    let col = col.min(span.columns() - 1);
    Some(DocPos::new(span.rows[row][col].start, 0))
}

impl EditorState {
    /// The table holding the caret, and the caret's row and column.
    fn caret_cell(&self) -> Option<(TableSpan, usize, usize)> {
        let head = self.head();
        let span = self.doc.table_at(head.para)?;
        let (row, col) = span.cell_of(head.para)?;
        Some((span, row, col))
    }

    /// The rows and columns the selection covers in the caret's table: from
    /// the anchor's cell to the head's, or the caret's alone when the anchor
    /// is outside that table.
    fn covered_cells(&self) -> Option<(TableSpan, Range<usize>, Range<usize>)> {
        let (span, row, col) = self.caret_cell()?;
        let (r0, c0) = self
            .selection
            .anchor()
            .and_then(|a| span.cell_of(a.para))
            .unwrap_or((row, col));
        let rows = row.min(r0)..row.max(r0) + 1;
        let cols = col.min(c0)..col.max(c0) + 1;
        Some((span, rows, cols))
    }

    /// Where the caret is in a table, or `None` outside one.
    pub fn table_cursor(&self) -> Option<TableCursor> {
        let (span, row, column) = self.caret_cell()?;
        Some(TableCursor {
            id: span.id,
            row,
            column,
            rows: span.rows.len(),
            table: self.doc.tables().get(span.id)?.clone(),
        })
    }

    /// Inserts a table of empty cells at the start of the selection (which
    /// it replaces) and puts the caret in its first cell. Not inside a table.
    pub(super) fn insert_table(&mut self, rows: usize, columns: usize) -> Effect {
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        if self.doc.region(range.start.para).is_some() || self.doc.region(range.end.para).is_some()
        {
            return Effect::NONE;
        }
        let (rows, columns) = (rows.clamp(1, MAX_ROWS), columns.clamp(1, MAX_COLUMNS));
        self.run(|cx| {
            if !range.is_empty() {
                cx.apply(EditOp::Delete { range })?;
            }
            let at = range.start;
            // The table goes before the caret's paragraph, split off at the
            // caret, which then follows the table.
            let index = if at.byte == 0 {
                at.para
            } else {
                cx.apply(EditOp::SplitParagraph { at })?;
                at.para + 1
            };
            let id = cx.doc().tables().next_id();
            let style = cx.doc().typing_style(DocPos::new(index, 0));
            let paras = (0..rows * columns)
                .map(|i| {
                    Paragraph::new("", ParaStyleId::DEFAULT, style)
                        .with_cell(Some(mark(id, i % columns)))
                })
                .collect();
            cx.apply(EditOp::InsertParas {
                at: index,
                content: Slice {
                    paras,
                    objects: Vec::new(),
                    tables: vec![(id, Table::new(columns))],
                },
            })?;
            Ok(Some(Selection::caret(DocPos::new(index, 0))))
        })
    }

    /// Inserts a row above or below the caret's row, its cells styled like
    /// the ones beside them, and puts the caret in its first cell.
    pub(super) fn insert_row(&mut self, below: bool) -> Effect {
        let Some((span, row, _)) = self.caret_cell() else {
            return Effect::NONE;
        };
        if span.rows.len() >= MAX_ROWS {
            return Effect::NONE;
        }
        let cells = &span.rows[row];
        let at = if below {
            cells[cells.len() - 1].end
        } else {
            cells[0].start
        };
        let paras: Vec<Paragraph> = cells
            .iter()
            .enumerate()
            .map(|(col, cell)| empty_cell(&self.doc, cell.start, mark(span.id, col)))
            .collect();
        self.run(|cx| {
            insert(cx, at, paras)?;
            Ok(Some(Selection::caret(DocPos::new(at, 0))))
        })
    }

    /// Inserts a column left or right of the caret's column, taking its width
    /// evenly from the others, and puts the caret in its cell.
    pub(super) fn insert_column(&mut self, right: bool) -> Effect {
        let Some((span, row, col)) = self.caret_cell() else {
            return Effect::NONE;
        };
        let Some(table) = self.doc.tables().get(span.id).cloned() else {
            return Effect::NONE;
        };
        if table.columns.len() >= MAX_COLUMNS {
            return Effect::NONE;
        }
        let new = if right { col + 1 } else { col };
        let count = table.columns.len() as f32;
        let mut widths: Vec<f32> = table.columns.iter().map(|w| w * count).collect();
        widths.insert(new, 1.0);
        let settings = table.with_widths(&widths);
        let start = span.paras.start;
        self.run(|cx| {
            // Bottom row first, so the rows above keep their positions.
            for cells in span.rows.iter().rev() {
                let beside = cells[col].start;
                if new == 0 {
                    cx.apply(EditOp::SetCells(vec![(
                        cells[0].start,
                        Some(mark(span.id, 1)),
                    )]))?;
                }
                let at = cells
                    .get(new)
                    .map_or(cells[cells.len() - 1].end, |c| c.start);
                let para = empty_cell(cx.doc(), beside, mark(span.id, new));
                insert(cx, at, vec![para])?;
            }
            cx.apply(EditOp::SetTable {
                id: span.id,
                table: settings,
            })?;
            Ok(cell_start(cx.doc(), start, row, new).map(Selection::caret))
        })
    }

    /// Deletes the rows the selection covers (the whole table if that is all
    /// of them).
    pub(super) fn delete_rows(&mut self) -> Effect {
        let Some((span, rows, cols)) = self.covered_cells() else {
            return Effect::NONE;
        };
        if rows.len() == span.rows.len() {
            return self.delete_table();
        }
        let paras = span.row(rows.start).unwrap_or_default().start
            ..span.row(rows.end - 1).unwrap_or_default().end;
        let start = span.paras.start;
        self.run(|cx| {
            cx.apply(EditOp::RemoveParas { paras })?;
            Ok(cell_start(cx.doc(), start, rows.start, cols.start).map(Selection::caret))
        })
    }

    /// Deletes the columns the selection covers (the whole table if that is
    /// all of them), giving their width to the others.
    pub(super) fn delete_columns(&mut self) -> Effect {
        let Some((span, rows, cols)) = self.covered_cells() else {
            return Effect::NONE;
        };
        let Some(table) = self.doc.tables().get(span.id).cloned() else {
            return Effect::NONE;
        };
        if cols.len() == table.columns.len() {
            return self.delete_table();
        }
        let widths: Vec<f32> = table
            .columns
            .iter()
            .enumerate()
            .filter(|(i, _)| !cols.contains(i))
            .map(|(_, w)| *w)
            .collect();
        let settings = table.with_widths(&widths);
        let start = span.paras.start;
        self.run(|cx| {
            for cells in span.rows.iter().rev() {
                let paras = cells[cols.start].start..cells[cols.end - 1].end;
                cx.apply(EditOp::RemoveParas {
                    paras: paras.clone(),
                })?;
                if cols.start == 0 {
                    cx.apply(EditOp::SetCells(vec![(
                        paras.start,
                        Some(mark(span.id, 0)),
                    )]))?;
                }
            }
            cx.apply(EditOp::SetTable {
                id: span.id,
                table: settings,
            })?;
            Ok(cell_start(cx.doc(), start, rows.start, cols.start).map(Selection::caret))
        })
    }

    /// Deletes the table holding the caret, which goes to the paragraph that
    /// followed it.
    pub(super) fn delete_table(&mut self) -> Effect {
        let Some((span, _, _)) = self.caret_cell() else {
            return Effect::NONE;
        };
        let at = span.paras.start;
        self.run(|cx| {
            cx.apply(EditOp::RemoveParas { paras: span.paras })?;
            Ok(Some(Selection::caret(DocPos::new(at, 0))))
        })
    }

    /// Replaces the settings of table `id`; `table` must have as many
    /// columns.
    pub(super) fn set_table(&mut self, id: TableId, table: Table) -> Effect {
        let Some(old) = self.doc.tables().get(id) else {
            return Effect::NONE;
        };
        if *old == table || table.columns.len() != old.columns.len() {
            return Effect::NONE;
        }
        self.run(|cx| {
            cx.apply(EditOp::SetTable { id, table })?;
            Ok(None)
        })
    }

    /// Tab: selects the next cell's text, adding a row after the last cell;
    /// Shift+Tab (`forward` false): the previous cell's, if any.
    pub(super) fn next_cell(&mut self, forward: bool) -> Effect {
        let Some((span, row, col)) = self.caret_cell() else {
            return Effect::NONE;
        };
        let cells: Vec<&Range<usize>> = span.rows.iter().flatten().collect();
        let at = row * span.columns() + col;
        let target = if forward {
            match cells.get(at + 1) {
                Some(cell) => (*cell).clone(),
                None => return self.insert_row(true),
            }
        } else {
            match at.checked_sub(1) {
                Some(prev) => cells[prev].clone(),
                None => return Effect::NONE,
            }
        };
        let last = target.end - 1;
        let end = DocPos::new(last, self.doc.paragraphs()[last].text().len());
        self.set_selection(Selection::text(DocPos::new(target.start, 0), end))
    }
}
