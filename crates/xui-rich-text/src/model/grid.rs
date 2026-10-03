#![forbid(unsafe_code)]

//! A document's tables: finding them in the paragraph list and checking that
//! the cell marks form whole grids.

use std::collections::HashSet;

use super::paragraph::Paragraph;
use super::table::{CellStart, Table, TableId, TableSpan, TableTable, spans};
use super::{DocPos, Document};

impl Document {
    /// The table settings.
    pub fn tables(&self) -> &TableTable {
        &self.tables
    }

    /// Every table, top to bottom, with its grid.
    pub fn table_spans(&self) -> Vec<TableSpan> {
        spans(self.paragraphs.iter().map(|p| p.cell.as_ref()))
    }

    /// The table paragraph `para` is in, with its grid.
    pub fn table_at(&self, para: usize) -> Option<TableSpan> {
        let id = self.paragraphs.get(para)?.cell?.table;
        let same = |p: &Paragraph| p.cell.is_some_and(|c| c.table == id);
        let start = self.paragraphs[..para]
            .iter()
            .rposition(|p| !same(p))
            .map_or(0, |i| i + 1);
        let end = self.paragraphs[para..]
            .iter()
            .position(|p| !same(p))
            .map_or(self.paragraphs.len(), |i| para + i);
        let mut span = spans(self.paragraphs[start..end].iter().map(|p| p.cell.as_ref()))
            .into_iter()
            .next()?;
        let shift = |r: &mut std::ops::Range<usize>| {
            r.start += start;
            r.end += start;
        };
        shift(&mut span.paras);
        span.rows.iter_mut().flatten().for_each(shift);
        Some(span)
    }

    /// The first paragraph of the cell `para` is in, or `None` outside any
    /// table: two positions with the same region can be joined by an edit.
    pub(crate) fn region(&self, para: usize) -> Option<usize> {
        self.paragraphs.get(para)?.cell?;
        let first = self.paragraphs[..=para]
            .iter()
            .rposition(|p| p.cell.is_none_or(|c| c.start != CellStart::Continue))?;
        Some(first)
    }

    /// Whether an edit may join `a` and `b` into one paragraph: both are in
    /// one cell, or both are outside tables.
    pub(crate) fn same_region(&self, a: DocPos, b: DocPos) -> bool {
        self.region(a.para) == self.region(b.para)
    }

    /// Removes the tables `paras` belong to that no paragraph of the document
    /// is in any more, returning them.
    pub(crate) fn take_unused_tables(&mut self, paras: &[Paragraph]) -> Vec<(TableId, Table)> {
        let mut ids: Vec<TableId> = paras
            .iter()
            .filter_map(|p| p.cell)
            .map(|c| c.table)
            .collect();
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Vec::new();
        }
        let used: HashSet<TableId> = self
            .paragraphs
            .iter()
            .filter_map(|p| p.cell)
            .map(|c| c.table)
            .collect();
        ids.into_iter()
            .filter(|id| !used.contains(id))
            .filter_map(|id| self.tables.remove(id).map(|t| (id, t)))
            .collect()
    }

    /// Checks that the cell marks form whole tables: each table's paragraphs
    /// are contiguous, start a row, give every row as many cells as the table
    /// has columns, and are followed by a paragraph outside any table.
    pub(crate) fn check_tables(&self) -> Result<(), String> {
        let found = self.table_spans();
        let mut seen = HashSet::new();
        let mut covered = 0;
        for span in &found {
            let table = self
                .tables
                .get(span.id)
                .ok_or_else(|| format!("paragraph {}: unknown table", span.paras.start))?;
            table
                .check()
                .map_err(|e| format!("table at paragraph {}: {e}", span.paras.start))?;
            if !seen.insert(span.id) {
                return Err(format!(
                    "paragraph {}: a table continues after other paragraphs",
                    span.paras.start
                ));
            }
            if span.rows.len() > super::table::MAX_ROWS {
                return Err(format!(
                    "table at paragraph {}: too many rows",
                    span.paras.start
                ));
            }
            if let Some(row) = span
                .rows
                .iter()
                .position(|r| r.len() != table.columns.len())
            {
                return Err(format!(
                    "table at paragraph {}: row {row} has {} cells, not {}",
                    span.paras.start,
                    span.rows[row].len(),
                    table.columns.len()
                ));
            }
            if self
                .paragraphs
                .get(span.paras.end)
                .is_none_or(|p| p.cell.is_some())
            {
                return Err(format!(
                    "table at paragraph {}: no paragraph after it",
                    span.paras.start
                ));
            }
            covered += span.paras.len();
        }
        let marked = self.paragraphs.iter().filter(|p| p.cell.is_some()).count();
        if marked != covered {
            return Err("a cell mark is outside any table".into());
        }
        Ok(())
    }
}
