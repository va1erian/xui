#![forbid(unsafe_code)]

//! GFM tables: column widths from the cells' natural widths, and one row at a
//! time laid out in them.
//!
//! A column is as wide as its widest cell on one line. When the columns do
//! not fit, they shrink in proportion and their cells wrap. The widths are
//! worked out once per table and width, from every row, so rows laid out at
//! different times line up.

use std::ops::Range;

use super::flow::{Flow, flow};
use super::geom::{BlockLayout, Ctx, Decor, FRect};
use crate::model::{BlockKind, Inline, Span};
use crate::style::Role;

/// Each table's rows and its column widths for the current width.
#[derive(Default)]
pub(crate) struct Tables {
    /// Block indices of each table's rows.
    rows: Vec<Range<usize>>,
    /// Column widths, and the width they were worked out for.
    widths: Vec<Option<(f32, Vec<f32>)>>,
}

impl Tables {
    /// Finds every table's rows in `blocks`.
    pub fn new(doc: &crate::model::Document) -> Tables {
        let mut rows: Vec<Range<usize>> = Vec::new();
        for (i, block) in doc.blocks().iter().enumerate() {
            if let BlockKind::TableRow { table, .. } = block.kind {
                let table = table as usize;
                if rows.len() <= table {
                    rows.resize(table + 1, i..i);
                }
                rows[table].end = i + 1;
            }
        }
        Tables {
            widths: vec![None; rows.len()],
            rows,
        }
    }

    /// Forgets the column widths, for a new DPI or style.
    pub fn clear(&mut self) {
        self.widths.iter_mut().for_each(|w| *w = None);
    }

    fn widths(&mut self, ctx: &mut Ctx, table: usize, width: f32) -> Vec<f32> {
        if let Some((at, widths)) = &self.widths[table]
            && *at == width
        {
            return widths.clone();
        }
        let widths = columns(ctx, self.rows[table].clone(), width);
        self.widths[table] = Some((width, widths.clone()));
        widths
    }
}

/// The spans of a row that fall in `cell`; no span crosses a cell.
fn cell_spans<'a>(spans: &'a [Span], cell: &Range<u32>) -> &'a [Span] {
    let from = spans.partition_point(|s| s.range.end <= cell.start);
    let to = spans.partition_point(|s| s.range.start < cell.end);
    &spans[from..to.max(from)]
}

fn cell_flow<'s>(ctx: &Ctx<'s>, row: usize, column: usize, cell: &Range<u32>) -> Flow<'s> {
    let block = &ctx.doc.blocks[row];
    let (table, header) = match block.kind {
        BlockKind::TableRow { table, header, .. } => (table, header),
        _ => (0, false),
    };
    Flow {
        spans: cell_spans(ctx.doc.spans(block), cell),
        heading: None,
        extra: if header { Inline::BOLD } else { Inline::NONE },
        quoted: block.quote > 0,
        align: ctx
            .doc
            .table(table)
            .columns
            .get(column)
            .copied()
            .unwrap_or_default(),
    }
}

/// Column widths (cell padding included) for the rows `rows` in `width`.
fn columns(ctx: &mut Ctx, rows: Range<usize>, width: f32) -> Vec<f32> {
    let inset = ctx.px(ctx.style.inset);
    let mut natural: Vec<f32> = Vec::new();
    let mut scratch = BlockLayout::default();
    for row in rows {
        let cells = ctx.doc.cells(&ctx.doc.blocks[row]);
        if natural.len() < cells.len() {
            natural.resize(cells.len(), 0.0);
        }
        for (column, cell) in cells.iter().enumerate() {
            let f = cell_flow(ctx, row, column, cell);
            let w = flow(ctx, &f, (0.0, 0.0), f32::INFINITY, &mut scratch).width;
            natural[column] = natural[column].max(w.ceil() + 2.0 * inset);
            scratch.pieces.clear();
            scratch.decor.clear();
            scratch.links.clear();
        }
    }
    let total: f32 = natural.iter().sum();
    if total > width && total > 0.0 {
        let least = 3.0 * inset;
        natural
            .iter_mut()
            .for_each(|w| *w = (*w * width / total).floor().max(least));
    }
    natural
}

/// Lays out table row `index`, returning its height.
pub(crate) fn row(
    ctx: &mut Ctx,
    index: usize,
    left: f32,
    width: f32,
    tables: &mut Tables,
    out: &mut BlockLayout,
) -> f32 {
    let block = &ctx.doc.blocks[index];
    let BlockKind::TableRow { table, header, .. } = block.kind else {
        return 0.0;
    };
    let widths = tables.widths(ctx, table as usize, width);
    let inset = ctx.px(ctx.style.inset);
    let cells = ctx.doc.cells(block);
    let mut height: f32 = 0.0;
    let mut x = left;
    for (column, (cell, w)) in cells.iter().zip(&widths).enumerate() {
        let f = cell_flow(ctx, index, column, cell);
        let flowed = flow(ctx, &f, (x + inset, inset), (w - 2.0 * inset).max(1.0), out);
        height = height.max(flowed.height);
        x += w;
    }
    let height = height + 2.0 * inset;
    let mut x = left;
    for w in &widths {
        out.decor
            .push(Decor::Frame(FRect::new(x, 0.0, *w, height), Role::Border));
        x += w;
    }
    if header {
        out.decor.insert(
            0,
            Decor::Fill(FRect::new(left, 0.0, x - left, height), Role::Surface),
        );
    }
    height
}
