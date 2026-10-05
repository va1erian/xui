#![forbid(unsafe_code)]

//! Grids: items flowed into columns, row by row.
//!
//! Column widths come from the [`Track`]s; row heights from the items in the
//! row (the tallest natural height, measured at the item's cell width, or a
//! `Fill`/`Fixed`/`Height` sizing on any of them). Both axes are split by a
//! [`Stack`], so leftover pixels are shared the same way rows and columns share
//! them.

use super::{
    Align, Constraints, Group, Item, LeafFn, Out, Sizing, Track, align_both, clamp_to_caps,
};
use crate::geometry::{Rect, Size};
use crate::layout::{Stack, StackDirection, StackSlot};
use crate::units::{Dip, Px};

/// Where one visible item sits: its row, first column and column count.
struct Cell<'a, K> {
    item: &'a Item<K>,
    row: usize,
    column: usize,
    span: usize,
}

/// Flows `items` into `columns` columns, row by row.
fn cells<'a, K: Copy>(items: &[&'a Item<K>], columns: usize) -> Vec<Cell<'a, K>> {
    let mut cells = Vec::with_capacity(items.len());
    let (mut row, mut column) = (0, 0);
    for item in items {
        let span = item.span.min(columns);
        if column + span > columns {
            row += 1;
            column = 0;
        }
        cells.push(Cell {
            item,
            row,
            column,
            span,
        });
        column += span;
        if column == columns {
            row += 1;
            column = 0;
        }
    }
    cells
}

fn row_count<K>(cells: &[Cell<'_, K>]) -> usize {
    cells.last().map_or(0, |cell| cell.row + 1)
}

/// Each column's natural width: its fixed size, or the widest single-column
/// item in an `Auto` column. A `Fill` column has none.
fn column_naturals<K: Copy>(
    cells: &[Cell<'_, K>],
    columns: &[Track],
    dpi: u32,
    leaf: LeafFn<'_, K>,
) -> Vec<i32> {
    let mut widths: Vec<i32> = columns
        .iter()
        .map(|track| match track {
            Track::Fixed(size) => size.to_px(dpi).value().max(0),
            Track::Auto | Track::Fill(_) => 0,
        })
        .collect();
    for cell in cells.iter().filter(|cell| cell.span == 1) {
        if columns[cell.column] == Track::Auto {
            let natural = cell.item.natural(
                StackDirection::Horizontal,
                Constraints::unbounded(dpi),
                leaf,
            );
            widths[cell.column] = widths[cell.column].max(natural.width);
        }
    }
    widths
}

/// The row slots: a `Fill` item makes its row fill (by the largest weight), a
/// `Fixed` or `Height` item fixes it, otherwise the row takes its tallest
/// item's natural height at the item's cell width.
fn row_slots<K: Copy>(
    cells: &[Cell<'_, K>],
    cell_width: &dyn Fn(&Cell<'_, K>) -> Option<i32>,
    dpi: u32,
    leaf: LeafFn<'_, K>,
) -> Vec<StackSlot> {
    let mut slots = vec![StackSlot::FixedPx(Px(0)); row_count(cells)];
    for cell in cells {
        let slot = &mut slots[cell.row];
        match cell.item.sizing {
            Sizing::Fill(weight) => {
                *slot = match *slot {
                    StackSlot::Fill(current) => StackSlot::Fill(current.max(weight)),
                    _ => StackSlot::Fill(weight),
                };
            }
            _ if matches!(slot, StackSlot::Fill(_)) => {}
            Sizing::Fixed(size) | Sizing::Height(size) => {
                let px = size.to_px(dpi).value().max(0);
                *slot = StackSlot::FixedPx(Px(px.max(fixed_px(*slot))));
            }
            _ => {
                let mut constraints = Constraints::unbounded(dpi);
                if let Some(width) = cell_width(cell) {
                    constraints = constraints.with_width(width);
                }
                let natural = cell
                    .item
                    .natural(StackDirection::Vertical, constraints, leaf);
                *slot = StackSlot::FixedPx(Px(natural.height.max(fixed_px(*slot))));
            }
        }
    }
    slots
}

fn fixed_px(slot: StackSlot) -> i32 {
    match slot {
        StackSlot::FixedPx(px) => px.value(),
        _ => 0,
    }
}

/// The column slots for `columns`, given their natural widths.
fn column_slots(columns: &[Track], naturals: &[i32]) -> Vec<StackSlot> {
    columns
        .iter()
        .zip(naturals)
        .map(|(track, &natural)| match track {
            Track::Fill(weight) => StackSlot::Fill(*weight),
            Track::Auto | Track::Fixed(_) => StackSlot::FixedPx(Px(natural)),
        })
        .collect()
}

fn stack(direction: StackDirection, spacing: Dip, slots: &[StackSlot]) -> Stack {
    let stack = match direction {
        StackDirection::Horizontal => Stack::horizontal(),
        StackDirection::Vertical => Stack::vertical(),
    };
    slots
        .iter()
        .fold(stack.spacing(spacing), |stack, slot| stack.push(*slot))
}

/// Places `group`'s items in a grid of `columns` inside `rect`.
pub(super) fn place<K: Copy>(
    group: &Group<K>,
    columns: &[Track],
    rect: Rect,
    dpi: u32,
    leaf: LeafFn<'_, K>,
    out: &mut Out<K>,
) {
    let visible = group.visible_items(leaf, dpi);
    let cells = cells(&visible, columns.len());
    if cells.is_empty() {
        return;
    }
    let inner = group.margins.apply(rect, dpi);
    let naturals = column_naturals(&cells, columns, dpi, leaf);
    let xs = stack(
        StackDirection::Horizontal,
        group.spacing,
        &column_slots(columns, &naturals),
    )
    .split(inner, dpi);
    let span_rect = |cell: &Cell<'_, K>| {
        let first = xs[cell.column];
        let last = xs[cell.column + cell.span - 1];
        (first.left, last.right)
    };
    let width = |cell: &Cell<'_, K>| {
        let (left, right) = span_rect(cell);
        Some(right - left)
    };
    let ys = stack(
        StackDirection::Vertical,
        group.spacing,
        &row_slots(&cells, &width, dpi, leaf),
    )
    .split(inner, dpi);
    for cell in &cells {
        let (left, right) = span_rect(cell);
        let row = ys[cell.row];
        let area = Rect::new(left, row.top, right, row.bottom);
        let align = cell.item.align.unwrap_or(group.align);
        let area = align_in_cell(cell.item, area, align, dpi, leaf);
        let area = clamp_to_caps(cell.item, area, align, dpi);
        cell.item.place(area, dpi, leaf, out);
    }
}

/// Narrows `area` on both axes to the item's natural size, placed by `align`;
/// a stretched item keeps the whole cell.
fn align_in_cell<K: Copy>(
    item: &Item<K>,
    area: Rect,
    align: Align,
    dpi: u32,
    leaf: LeafFn<'_, K>,
) -> Rect {
    if align == Align::Stretch {
        return area;
    }
    let constraints = Constraints::unbounded(dpi).with_width(area.width());
    let natural = item.natural(StackDirection::Vertical, constraints, leaf);
    align_both(area, natural, align)
}

/// The grid's natural size within `constraints`: the margins plus the
/// columns' natural widths and the rows' natural heights, with the gaps.
///
/// Each row is measured at the widths its cells will get: with a width bound,
/// the columns are split as [`place`] splits them; without one, a cell over a
/// `Fill` column has no width bound (its width is not known yet).
pub(super) fn measure<K: Copy>(
    group: &Group<K>,
    columns: &[Track],
    constraints: Constraints,
    leaf: LeafFn<'_, K>,
) -> Size {
    let dpi = constraints.dpi;
    let visible = group.visible_items(leaf, dpi);
    let cells = cells(&visible, columns.len());
    let naturals = column_naturals(&cells, columns, dpi, leaf);
    let px = |value: Dip| value.to_px(dpi).value().max(0);
    let gap = px(group.spacing);
    let margins = group.margins;
    let xs = constraints.max_width.map(|max| {
        let inner = (max - px(margins.left) - px(margins.right)).max(0);
        stack(
            StackDirection::Horizontal,
            group.spacing,
            &column_slots(columns, &naturals),
        )
        .split(Rect::new(0, 0, inner, 1), dpi)
    });
    let width = |cell: &Cell<'_, K>| {
        let last = cell.column + cell.span - 1;
        match &xs {
            Some(xs) => Some(xs[last].right - xs[cell.column].left),
            None if columns[cell.column..=last]
                .iter()
                .any(|track| matches!(track, Track::Fill(_))) =>
            {
                None
            }
            None => Some(
                naturals[cell.column..=last].iter().sum::<i32>() + gap * (cell.span as i32 - 1),
            ),
        }
    };
    let rows: Vec<i32> = row_slots(&cells, &width, dpi, leaf)
        .into_iter()
        .map(fixed_px)
        .collect();
    let along = |sizes: &[i32]| -> i32 {
        sizes.iter().sum::<i32>() + gap * (sizes.len().saturating_sub(1) as i32)
    };
    Size::new(
        along(&naturals) + px(margins.left) + px(margins.right),
        along(&rows) + px(margins.top) + px(margins.bottom),
    )
}
