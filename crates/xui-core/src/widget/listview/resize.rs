#![forbid(unsafe_code)]

//! Header column resizing: the boundary hit test, the live drag and the
//! double-click auto-size.
//!
//! Dragging a column's right boundary sets that column to a
//! [`Fixed`](super::ColumnWidth::Fixed) width, converting a
//! [`Fill`](super::ColumnWidth::Fill) column to a fixed one at the dragged
//! width (the remaining `Fill` columns then share what is left). The right edge
//! of the last column is not draggable: a `Fill` last column already spans to
//! the client edge, so there is no leftover space to trade away.

use super::state::{ROW, State, TEXT_SIZE, header_px};
use crate::app::Ui;
use crate::backend::{TextStyle, WidgetId};
use crate::units::{Dip, Px};

/// How close to a boundary the pointer can be to grab it, in design values.
pub(crate) const GRAB: Dip = Dip(4.0);
/// The narrowest a dragged column may become, in design values.
pub(crate) const MIN_WIDTH: Dip = Dip(20.0);
/// The design inset a column's text leaves on each side.
const PAD: Dip = Dip(6.0);
/// How many rows auto-size samples, so a huge model stays cheap.
const AUTOSIZE_ROWS: usize = 200;

/// The interior column boundary (the right edge of column `i`, `i` not the
/// last) within `tolerance` pixels of `x`.
pub(crate) fn boundary_at(widths: &[i32], x: i32, tolerance: i32) -> Option<usize> {
    let mut left = 0;
    for (index, width) in widths
        .iter()
        .enumerate()
        .take(widths.len().saturating_sub(1))
    {
        left += width;
        if (x - left).abs() <= tolerance {
            return Some(index);
        }
    }
    None
}

/// Starts a drag on the boundary at node-local `x`, if one is there. Returns
/// the column whose width the drag will change.
pub(crate) fn begin(state: &mut State, x: i32, widths: &[i32], dpi: u32) -> Option<usize> {
    let column = boundary_at(widths, x, GRAB.to_px(dpi).value())?;
    let original = Px(widths[column]).to_dip(dpi);
    state.resize = Some(super::state::Resize {
        column,
        start_x: x,
        start_px: widths[column],
        original,
        width: original,
    });
    Some(column)
}

/// Applies a drag move to node-local `x` and returns the column's new design
/// width.
pub(crate) fn drag(state: &mut State, x: i32, dpi: u32) -> Option<Dip> {
    let (column, start_x, start_px) = {
        let resize = state.resize.as_ref()?;
        (resize.column, resize.start_x, resize.start_px)
    };
    let min = MIN_WIDTH.to_px(dpi).value();
    let width = (start_px + (x - start_x)).max(min);
    let design = Px(width).to_dip(dpi);
    if let Some(resize) = state.resize.as_mut() {
        resize.width = design;
    }
    set_width(state, column, design);
    Some(design)
}

/// Ends a drag, returning the column and its final width when it changed.
pub(crate) fn finish(state: &mut State) -> Option<(usize, Dip)> {
    let resize = state.resize.take()?;
    (resize.width != resize.original).then_some((resize.column, resize.width))
}

/// The widest of the column's header and (bounded) visible cells, converted to
/// a design value.
pub(crate) fn autosize<M: 'static>(
    ui: &Ui<M>,
    id: WidgetId,
    state: &State,
    column: usize,
    dpi: u32,
) -> Dip {
    let style = TextStyle::new(ui.theme().text, TEXT_SIZE);
    let mut widest = state
        .columns
        .get(column)
        .map_or(0, |col| ui.measure_text(&col.title, &style, dpi).width);

    let row_px = ROW.to_px(dpi).value().max(1);
    let body = (ui.bounds(id).height() - header_px(state.has_header(), dpi)).max(0);
    let visible = (body / row_px) as usize;
    let end = (state.offset + visible).min(state.len()).min(AUTOSIZE_ROWS);
    for row in state.offset..end {
        if let Some(text) = state.rows.cell(row, column) {
            widest = widest.max(ui.measure_text(text, &style, dpi).width);
        }
    }
    let width = widest + 2 * PAD.to_px(dpi).value();
    Px(width.max(MIN_WIDTH.to_px(dpi).value())).to_dip(dpi)
}

/// Sets `column`'s width to a fixed `design` value; out-of-range columns are
/// ignored.
pub(crate) fn set_width(state: &mut State, column: usize, design: Dip) {
    if let Some(column) = state.columns.get_mut(column) {
        column.width = super::ColumnWidth::Fixed(design);
    }
}
