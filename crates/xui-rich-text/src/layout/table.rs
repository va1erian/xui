#![forbid(unsafe_code)]

//! Tables in the flow. A table is laid out as one block: each cell is a small
//! flow of its own paragraphs at the column's width, a row is as tall as its
//! tallest cell, and the rows stack. Floats from above end at the table's top.
//!
//! On pages a row is the unit that moves: one that would cross a page's
//! bottom starts the next page instead, below a copy of the header row when
//! the table has one. A row taller than a page is split between lines, each
//! cell's lines moving past the page gap on their own.

use std::ops::Range;

use xui_core::Dip;
use xui_core::backend::TextShaper;

use super::floats::FloatCtx;
use super::flow::{Env, Layout, lay_out};
use super::resolve::scale_for;
use crate::model::{Document, Table, TableId, TableSpan};

/// The space between a cell's edge and its text.
pub const CELL_PADDING: Dip = Dip(4.0);
/// A header row is repeated on later pages only while it takes at most this
/// share of a page.
const MAX_HEADER_SHARE: f32 = 0.5;

/// One laid-out row.
#[derive(Clone, Debug, PartialEq)]
pub struct RowLayout {
    /// The row's top, in area pixels.
    pub top: f32,
    /// Its height.
    pub height: f32,
    /// Each cell's paragraphs, left to right.
    pub cells: Vec<Range<usize>>,
}

impl RowLayout {
    /// The row's bottom edge.
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

/// A laid-out table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableLayout {
    /// The table.
    pub id: TableId,
    /// Its paragraphs.
    pub paras: Range<usize>,
    /// The column edges in area pixels, one more than there are columns.
    pub edges: Vec<f32>,
    /// The rows, top to bottom.
    pub rows: Vec<RowLayout>,
    /// Where copies of the header row are drawn at the top of the later pages
    /// the table runs onto (their tops, in area pixels).
    pub repeats: Vec<f32>,
    /// Whether the grid is drawn.
    pub border: bool,
    /// The top it was laid out at.
    pub(crate) laid_y: f32,
    /// The top of the flow where it was placed (above any floats it cleared).
    pub(crate) flow_y: f32,
}

impl TableLayout {
    /// The table's top edge.
    pub fn top(&self) -> f32 {
        self.rows.first().map_or(self.laid_y, |r| r.top)
    }

    /// The table's bottom edge, where the flow continues.
    pub fn bottom(&self) -> f32 {
        self.rows.last().map_or(self.laid_y, RowLayout::bottom)
    }

    /// The row holding flow position `y` (the nearest one above or below it
    /// when `y` is outside the table or in a gap between pages).
    pub fn row_at(&self, y: f32) -> usize {
        self.rows
            .partition_point(|r| r.bottom() <= y)
            .min(self.rows.len().saturating_sub(1))
    }

    /// The column holding `x` (clamped to the first and last).
    pub fn col_at(&self, x: f32) -> usize {
        let inner = &self.edges[1..self.edges.len() - 1];
        inner.partition_point(|&edge| edge <= x)
    }

    /// The row and column of the cell holding paragraph `para`.
    pub fn cell_of(&self, para: usize) -> Option<(usize, usize)> {
        let row = self
            .rows
            .partition_point(|r| r.cells.last().is_some_and(|c| c.end <= para));
        let col = self
            .rows
            .get(row)?
            .cells
            .iter()
            .position(|c| c.contains(&para))?;
        Some((row, col))
    }

    /// Moves the table down by `dy`.
    fn shift(&mut self, dy: f32) {
        self.laid_y += dy;
        self.flow_y += dy;
        for row in &mut self.rows {
            row.top += dy;
        }
        for top in &mut self.repeats {
            *top += dy;
        }
    }

    /// Whether the table was laid out from the same grid.
    pub(super) fn same_grid(&self, span: &TableSpan) -> bool {
        self.id == span.id
            && self.paras == span.paras
            && self.rows.len() == span.rows.len()
            && self
                .rows
                .iter()
                .zip(&span.rows)
                .all(|(r, cells)| r.cells == *cells)
    }
}

/// The column edges of `table` across `width` pixels.
fn edges(table: &Table, width: f32) -> Vec<f32> {
    let mut at = 0.0;
    let mut out = vec![0.0];
    for w in &table.columns {
        at += w * width;
        out.push(at.min(width));
    }
    if let Some(last) = out.last_mut() {
        *last = width;
    }
    out
}

impl Layout {
    /// The laid-out tables, top to bottom.
    pub fn tables(&self) -> &[TableLayout] {
        &self.tables
    }

    /// The laid-out table paragraph `para` is in.
    pub fn table_of(&self, para: usize) -> Option<&TableLayout> {
        let at = self.tables.partition_point(|t| t.paras.end <= para);
        self.tables.get(at).filter(|t| t.paras.contains(&para))
    }

    /// The inner column edge within `slop` pixels of `point` (area pixels),
    /// as the index of its table in [`tables`](Layout::tables) and of the
    /// edge in [`TableLayout::edges`].
    pub fn column_edge_at(
        &self,
        point: xui_core::geometry::Point,
        slop: f32,
    ) -> Option<(usize, usize)> {
        let (x, y) = (point.x as f32, point.y as f32);
        self.tables.iter().enumerate().find_map(|(index, t)| {
            if y < t.top() || y >= t.bottom() {
                return None;
            }
            let inner = 1..t.edges.len() - 1;
            let edge = inner
                .into_iter()
                .find(|&k| (t.edges[k] - x).abs() <= slop)?;
            Some((index, edge))
        })
    }

    /// Forgets the tables at or after paragraph `first` (their paragraphs are
    /// being replaced or renumbered).
    pub(super) fn drop_tables_from(&mut self, first: usize) {
        self.tables.retain(|t| t.paras.end <= first);
    }

    /// Where the flow continues after paragraph `index`: its bottom and the
    /// floats still active, or a table's bottom with none.
    pub(super) fn after(&self, index: usize) -> (f32, FloatCtx) {
        let pages = self.pages;
        match self.table_of(index) {
            Some(t) => (t.bottom(), FloatCtx::default().with_pages(pages)),
            None => {
                let p = &self.paras[index];
                (
                    p.bottom(),
                    FloatCtx::from_relative(&p.exit, p.bottom()).with_pages(pages),
                )
            }
        }
    }

    /// Whether the table laid out for `span` can stay, moved to `y` if need
    /// be: the same grid, every paragraph clean and verified, the same list
    /// numbers, and on pages not moved at all.
    pub(super) fn table_reusable(
        &self,
        span: &TableSpan,
        y: f32,
        numbers: &[Option<usize>],
    ) -> bool {
        let Some(t) = self.table_of(span.paras.start) else {
            return false;
        };
        t.same_grid(span)
            && (self.pages.is_none() || (t.flow_y - y).abs() < 0.01)
            && span.paras.clone().all(|i| {
                let p = &self.paras[i];
                !p.dirty && !p.speculative && p.number == numbers[i]
            })
    }

    /// Moves the table laid out for `span` so the flow reaches it at `y`.
    pub(super) fn move_table(&mut self, span: &TableSpan, y: f32) {
        let Some(at) = self.tables.iter().position(|t| t.paras == span.paras) else {
            return;
        };
        let dy = y - self.tables[at].flow_y;
        if dy == 0.0 {
            return;
        }
        self.tables[at].shift(dy);
        for i in span.paras.clone() {
            let p = &mut self.paras[i];
            p.y += dy;
            p.laid_y += dy;
        }
    }

    /// Lays out the table `span` with the flow reaching it at `y` and the
    /// floats in `ctx`. Returns its bottom.
    pub(super) fn lay_table(
        &mut self,
        doc: &Document,
        shaper: &dyn TextShaper,
        span: &TableSpan,
        y: f32,
        ctx: &FloatCtx,
        numbers: &[Option<usize>],
    ) -> f32 {
        let fallback;
        let table = match doc.tables().get(span.id) {
            Some(table) => table,
            None => {
                fallback = Table::new(span.columns());
                &fallback
            }
        };
        let pad = CELL_PADDING.0 * scale_for(self.dpi);
        let edges = edges(table, self.width);
        let pages = self.pages;
        let mut top = ctx.clear_below(y);
        let mut rows = Vec::with_capacity(span.rows.len());
        let mut repeats = Vec::new();
        let mut header = None;
        for (r, cells) in span.rows.iter().enumerate() {
            let mut height = self.lay_row(doc, shaper, cells, &edges, top, pad, numbers);
            if let Some(pages) = pages
                && !pages.at_top(top)
                && pages.gap(top, height).is_some()
            {
                top = pages.next_top(top);
                if let Some(h) = header.filter(|&h| r > 0 && h <= pages.content * MAX_HEADER_SHARE)
                {
                    repeats.push(top);
                    top += h;
                }
                height = self.lay_row(doc, shaper, cells, &edges, top, pad, numbers);
            }
            if r == 0 && table.header {
                header = Some(height);
            }
            rows.push(RowLayout {
                top,
                height,
                cells: cells.clone(),
            });
            top += height;
        }
        let laid = TableLayout {
            id: span.id,
            paras: span.paras.clone(),
            edges,
            rows,
            repeats,
            border: table.border,
            laid_y: ctx.clear_below(y),
            flow_y: y,
        };
        let bottom = laid.bottom();
        let at = self
            .tables
            .partition_point(|t| t.paras.end <= span.paras.start);
        match self.tables.get(at) {
            Some(t) if t.paras.start < span.paras.end => self.tables[at] = laid,
            _ => self.tables.insert(at, laid),
        }
        bottom
    }

    /// Lays out one row's cells with its top at `top`; returns its height.
    #[allow(clippy::too_many_arguments)]
    fn lay_row(
        &mut self,
        doc: &Document,
        shaper: &dyn TextShaper,
        cells: &[Range<usize>],
        edges: &[f32],
        top: f32,
        pad: f32,
        numbers: &[Option<usize>],
    ) -> f32 {
        let mut bottom = top + 2.0 * pad;
        for (col, paras) in cells.iter().enumerate() {
            let (left, right) = (
                edges[col],
                edges.get(col + 1).copied().unwrap_or(edges[col]),
            );
            let x = left + pad;
            let width = (right - left - 2.0 * pad).max(1.0);
            let mut y = top + pad;
            for index in paras.clone() {
                let mut ctx = FloatCtx::default().with_pages(self.pages);
                let env = Env {
                    shaper,
                    cache: &mut self.cache,
                    dpi: self.dpi,
                    width,
                    x,
                    cell: true,
                };
                let mut laid = lay_out(doc, index, env, y, &mut ctx, numbers[index]);
                laid.exit.clear();
                y = laid.bottom();
                self.paras[index] = laid;
            }
            bottom = bottom.max(y + pad);
        }
        bottom - top
    }

    /// Places the table `span` at `y` by the paragraphs' current heights,
    /// estimates included (no pagination): what `reposition` uses for a table
    /// that is not laid out yet. Returns its bottom.
    pub(super) fn place_estimated(&mut self, span: &TableSpan, y: f32) -> f32 {
        let pad = CELL_PADDING.0 * scale_for(self.dpi);
        let mut top = y;
        for cells in &span.rows {
            let mut bottom = top + 2.0 * pad;
            for paras in cells {
                let mut at = top + pad;
                for i in paras.clone() {
                    self.paras[i].y = at;
                    at += self.paras[i].height;
                }
                bottom = bottom.max(at + pad);
            }
            top = bottom;
        }
        top
    }
}
