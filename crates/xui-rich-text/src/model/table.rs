#![forbid(unsafe_code)]

//! Tables: a grid of cells, each holding one or more paragraphs.
//!
//! The document stays a flat list of paragraphs. A paragraph inside a table
//! carries a [`CellMark`] naming its table and whether it starts a row, starts
//! a cell or continues the cell before it; rows and columns are derived from
//! those marks, so inserting a row is inserting paragraphs, with nothing to
//! renumber. A table's own settings (column widths, header row, border) live
//! in the [`TableTable`], like images in the object table.

use std::collections::HashMap;
use std::ops::Range;

/// The most columns a table may have.
pub const MAX_COLUMNS: usize = 64;
/// The most rows a table may have.
pub const MAX_ROWS: usize = 1024;

/// The id of a table in a [`TableTable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableId(pub(crate) u32);

/// Where a paragraph sits in its table's grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CellStart {
    /// It starts a row (and so the row's first cell).
    Row,
    /// It starts a cell other than a row's first.
    Cell,
    /// It continues the cell of the paragraph before it.
    Continue,
}

/// The mark of a paragraph inside a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellMark {
    /// The table.
    pub table: TableId,
    /// Where the paragraph sits.
    pub start: CellStart,
}

impl CellMark {
    /// The mark of a paragraph continuing this one's cell.
    pub fn continued(self) -> CellMark {
        CellMark {
            start: CellStart::Continue,
            ..self
        }
    }
}

/// A table's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// The column widths, as fractions of the text width; they sum to 1.
    pub columns: Vec<f32>,
    /// Whether the first row is a header, repeated at the top of each page
    /// the table continues on.
    pub header: bool,
    /// Whether a thin grid is drawn around and between the cells.
    pub border: bool,
}

impl Table {
    /// A table of `columns` equal columns with a border and no header row.
    pub fn new(columns: usize) -> Table {
        let columns = columns.clamp(1, MAX_COLUMNS);
        Table {
            columns: vec![1.0 / columns as f32; columns],
            header: false,
            border: true,
        }
    }

    /// The same table with widths proportional to `widths`.
    pub fn with_widths(mut self, widths: &[f32]) -> Table {
        let total: f32 = widths.iter().sum();
        self.columns = widths.iter().map(|w| w / total).collect();
        self
    }

    /// Checks the settings, describing the first problem.
    pub fn check(&self) -> Result<(), String> {
        if self.columns.is_empty() || self.columns.len() > MAX_COLUMNS {
            return Err(format!("{} columns", self.columns.len()));
        }
        if self.columns.iter().any(|w| !w.is_finite() || *w <= 0.0) {
            return Err("a column width is not a positive number".into());
        }
        let total: f32 = self.columns.iter().sum();
        if (total - 1.0).abs() > 1e-3 {
            return Err(format!("column widths sum to {total}"));
        }
        Ok(())
    }
}

/// The tables a document's paragraphs belong to.
#[derive(Clone, Debug, Default)]
pub struct TableTable {
    tables: HashMap<TableId, Table>,
    next: u32,
}

impl TableTable {
    /// An empty table store.
    pub fn new() -> TableTable {
        TableTable::default()
    }

    /// Adds `table` and returns its new id.
    pub fn insert(&mut self, table: Table) -> TableId {
        let id = TableId(self.next);
        self.next += 1;
        self.tables.insert(id, table);
        id
    }

    /// Restores `table` under a known `id` (undo, loading a file).
    pub fn insert_with_id(&mut self, id: TableId, table: Table) {
        self.next = self.next.max(id.0 + 1);
        self.tables.insert(id, table);
    }

    /// The id the next [`insert`](TableTable::insert) will give.
    pub fn next_id(&self) -> TableId {
        TableId(self.next)
    }

    /// The table `id` names.
    pub fn get(&self, id: TableId) -> Option<&Table> {
        self.tables.get(&id)
    }

    /// The table `id` names, mutably.
    pub(crate) fn get_mut(&mut self, id: TableId) -> Option<&mut Table> {
        self.tables.get_mut(&id)
    }

    /// Removes and returns the table `id` names.
    pub fn remove(&mut self, id: TableId) -> Option<Table> {
        self.tables.remove(&id)
    }

    /// The number of tables.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Whether there are no tables.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
}

/// A table as it lies in the paragraph list: its paragraphs and, row by row,
/// the paragraphs of each cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSpan {
    /// The table.
    pub id: TableId,
    /// All its paragraphs.
    pub paras: Range<usize>,
    /// Each row's cells, left to right, as paragraph ranges.
    pub rows: Vec<Vec<Range<usize>>>,
}

impl TableSpan {
    /// The number of columns of the first row (every row has as many).
    pub fn columns(&self) -> usize {
        self.rows.first().map_or(0, Vec::len)
    }

    /// The row and column of the cell holding paragraph `para`.
    pub fn cell_of(&self, para: usize) -> Option<(usize, usize)> {
        if !self.paras.contains(&para) {
            return None;
        }
        let row = self
            .rows
            .partition_point(|cells| cells.last().is_some_and(|c| c.end <= para));
        let col = self.rows.get(row)?.iter().position(|c| c.contains(&para))?;
        Some((row, col))
    }

    /// The paragraphs of the cell at `row` and `col`.
    pub fn cell(&self, row: usize, col: usize) -> Option<Range<usize>> {
        self.rows.get(row)?.get(col).cloned()
    }

    /// The paragraphs of row `row`.
    pub fn row(&self, row: usize) -> Option<Range<usize>> {
        let cells = self.rows.get(row)?;
        Some(cells.first()?.start..cells.last()?.end)
    }
}

/// The tables in `marks` (each paragraph's mark, in order), with their grids;
/// marks that do not form a table (a run not starting a row) are skipped.
pub(crate) fn spans<'a>(marks: impl Iterator<Item = Option<&'a CellMark>>) -> Vec<TableSpan> {
    let mut out: Vec<TableSpan> = Vec::new();
    let mut open = false;
    for (index, mark) in marks.enumerate() {
        let Some(mark) = mark else {
            open = false;
            continue;
        };
        let continues = open && out.last().is_some_and(|t| t.id == mark.table);
        match (continues, mark.start) {
            (false, CellStart::Row) => {
                out.push(TableSpan {
                    id: mark.table,
                    paras: index..index + 1,
                    rows: vec![vec![index..index + 1]],
                });
                open = true;
            }
            (false, _) => open = false,
            (true, start) => {
                let span = out.last_mut().expect("an open table");
                span.paras.end = index + 1;
                match start {
                    CellStart::Row => span.rows.push(std::iter::once(index..index + 1).collect()),
                    CellStart::Cell => span.rows.last_mut().expect("a row").push(index..index + 1),
                    CellStart::Continue => {
                        span.rows
                            .last_mut()
                            .and_then(|r| r.last_mut())
                            .expect("a cell")
                            .end = index + 1;
                    }
                }
            }
        }
    }
    out
}
