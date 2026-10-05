#![forbid(unsafe_code)]

//! Recognising rectangles that line up as a grid.

use super::Rect;
use crate::model::{Grid, Length, Node, Track};

/// Where one rectangle goes in the grid.
pub(super) struct Cell {
    /// Its place in the grid's flow order.
    pub(super) order: usize,
    /// Its height, which the grid keeps.
    pub(super) height: i64,
    /// The columns it covers.
    pub(super) span: usize,
}

/// A grid the rectangles line up as.
pub(super) struct GridLayout {
    widths: Vec<i64>,
    gap: i64,
    padding: i64,
    /// Whether every column is as wide and the padding is the same on both
    /// sides, so `Fill(1)` columns reproduce the design and also stretch.
    even: bool,
    /// One cell per rectangle, in the rectangles' order.
    pub(super) cells: Vec<Cell>,
}

impl GridLayout {
    /// The grid holding `children`, already in flow order.
    pub(super) fn into_grid(self, children: Vec<Node>) -> Grid {
        let columns = self
            .widths
            .iter()
            .map(|width| {
                if self.even {
                    Track::Fill(1)
                } else {
                    Track::Fixed(Length(*width as f32))
                }
            })
            .collect();
        Grid {
            columns,
            gap: Length(self.gap as f32),
            padding: Length(self.padding as f32),
            children,
            ..Grid::default()
        }
    }
}

/// The value every item of `values` shares, if they all agree.
fn same(mut values: impl Iterator<Item = i64>) -> Option<Option<i64>> {
    let Some(first) = values.next() else {
        return Some(None);
    };
    values.all(|value| value == first).then_some(Some(first))
}

/// `rects` as a grid inside a container of `size`, when they sit in rows
/// and columns with one gap, one padding and nothing out of place.
pub(super) fn detect(rects: &[Rect], size: (i64, i64)) -> Option<GridLayout> {
    if rects.len() < 2 {
        return None;
    }
    let mut lefts: Vec<i64> = rects.iter().map(|r| r.0).collect();
    lefts.sort_unstable();
    lefts.dedup();
    let mut tops: Vec<i64> = rects.iter().map(|r| r.1).collect();
    tops.sort_unstable();
    tops.dedup();
    let widths: Vec<i64> = lefts
        .iter()
        .map(|left| rects.iter().filter(|r| r.0 == *left).map(|r| r.2).min())
        .collect::<Option<_>>()?;
    let heights: Vec<i64> = tops
        .iter()
        .map(|top| same(rects.iter().filter(|r| r.1 == *top).map(|r| r.3)).flatten())
        .collect::<Option<_>>()?;
    let column_gaps = (1..lefts.len()).map(|k| lefts[k] - lefts[k - 1] - widths[k - 1]);
    let row_gaps = (1..tops.len()).map(|r| tops[r] - tops[r - 1] - heights[r - 1]);
    let gap = same(column_gaps.chain(row_gaps))?.unwrap_or(0);
    if gap < 0 || lefts[0] != tops[0] {
        return None;
    }

    // Each rectangle's cell and span; the flow must visit them in order.
    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by_key(|&index| (rects[index].1, rects[index].0));
    let mut cells: Vec<Option<Cell>> = (0..rects.len()).map(|_| None).collect();
    let (mut row, mut column) = (0, 0);
    for (flow, &index) in order.iter().enumerate() {
        let (left, top, width, height) = rects[index];
        if column == lefts.len() {
            (row, column) = (row + 1, 0);
        }
        if tops.get(row) != Some(&top) || lefts.get(column) != Some(&left) {
            return None;
        }
        let right = left + width;
        let last = (column..lefts.len()).find(|&k| lefts[k] + widths[k] == right)?;
        cells[index] = Some(Cell {
            order: flow,
            height,
            span: last - column + 1,
        });
        column = last + 1;
    }

    let padding = lefts[0];
    let right = lefts[lefts.len() - 1] + widths[widths.len() - 1];
    let even = same(widths.iter().copied()).flatten().is_some() && size.0 - right == padding;
    Some(GridLayout {
        widths,
        gap,
        padding,
        even,
        cells: cells.into_iter().collect::<Option<_>>()?,
    })
}
