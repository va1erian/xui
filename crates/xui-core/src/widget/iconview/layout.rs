#![forbid(unsafe_code)]

//! Pure tile-grid arithmetic for the icon view: how many columns fit a
//! viewport, where a tile sits, which tiles a point hits, which tiles a scrolled
//! viewport intersects and where the keyboard moves. Free of any widget state
//! and any backend type, so it is exercised by plain unit and property tests.

use std::ops::Range;

use super::metrics::Metrics;
use crate::geometry::Rect;

/// A keyboard navigation direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// How many tiles of `tile` px plus `gap` px fit across `width`, at least one.
/// A width that cannot hold a tile (zero, negative or smaller than one tile)
/// still yields one column, so a tiny view never divides by zero.
pub(crate) fn columns_for(width: i32, tile: i32, gap: i32) -> usize {
    if tile <= 0 {
        return 1;
    }
    let gap = gap.max(0);
    let stride = tile as i64 + gap as i64;
    ((width as i64 + gap as i64) / stride).max(1) as usize
}

/// The number of rows `len` tiles need at `columns` per row.
pub(crate) fn row_count(len: usize, columns: usize) -> usize {
    len.div_ceil(columns.max(1))
}

/// The full content height, in device pixels, for `len` tiles at `columns` per
/// row. The trailing gap is not counted, and the result saturates so a huge
/// model cannot overflow.
pub(crate) fn content_height(len: usize, columns: usize, metrics: Metrics) -> i32 {
    let rows = i64::try_from(row_count(len, columns)).unwrap_or(i64::MAX);
    if rows == 0 {
        return 0;
    }
    let stride = metrics.stride_y().max(1) as i64;
    rows.saturating_mul(stride)
        .saturating_sub(metrics.gap as i64)
        .clamp(0, i32::MAX as i64) as i32
}

/// A tile's rectangle in *content* coordinates (its top-left at the origin, so
/// the caller subtracts the scroll offset to place it). `index` past the model
/// is still a well-formed rectangle.
pub(crate) fn tile_rect(index: usize, columns: usize, metrics: Metrics) -> Rect {
    let columns = columns.max(1);
    let col = (index % columns) as i64;
    let row = i64::try_from(index / columns).unwrap_or(i64::MAX);
    let max_left = i32::MAX - metrics.width.max(1);
    let max_top = i32::MAX - metrics.height.max(1);
    let left = col.saturating_mul(metrics.stride_x().max(1) as i64);
    let top = row.saturating_mul(metrics.stride_y().max(1) as i64);
    let left = (left as i32).min(max_left);
    let top = (top as i32).min(max_top);
    Rect::new(left, top, left + metrics.width, top + metrics.height)
}

/// The tile index at `(x, y)` in content coordinates (the node-local `y` plus
/// the scroll offset). `None` inside a gap, before the origin or past the
/// model.
pub(crate) fn item_at(
    x: i32,
    y: i32,
    columns: usize,
    metrics: Metrics,
    len: usize,
) -> Option<usize> {
    if metrics.width <= 0 || metrics.height <= 0 || x < 0 || y < 0 {
        return None;
    }
    let columns = columns.max(1);
    let stride_x = metrics.stride_x().max(1) as i64;
    let stride_y = metrics.stride_y().max(1) as i64;
    let (x, y) = (x as i64, y as i64);
    let col = x / stride_x;
    let row = y / stride_y;
    if x - col * stride_x >= metrics.width as i64 || y - row * stride_y >= metrics.height as i64 {
        return None;
    }
    if col >= columns as i64 {
        return None;
    }
    let index = row.saturating_mul(columns as i64).saturating_add(col);
    if index < 0 {
        return None;
    }
    let index = usize::try_from(index).ok()?;
    (index < len).then_some(index)
}

/// The contiguous index range of tiles whose rectangles intersect the viewport
/// `[scroll, scroll + viewport_height)`. Empty when the model or the viewport is
/// empty.
pub(crate) fn visible_range(
    scroll: i32,
    viewport_height: i32,
    columns: usize,
    metrics: Metrics,
    len: usize,
) -> Range<usize> {
    if len == 0 || viewport_height <= 0 || metrics.height <= 0 {
        return 0..0;
    }
    let scroll = scroll.max(0) as i64;
    let viewport = viewport_height as i64;
    let tile = metrics.height as i64;
    let stride = metrics.stride_y().max(1) as i64;
    // The first row whose tile bottom is past the scroll position.
    let first_row = if scroll < tile {
        0
    } else {
        (scroll - tile) / stride + 1
    };
    // The last row whose tile top is before the viewport bottom; inclusive.
    let last_row = (scroll + viewport - 1).max(0) / stride;
    let rows = i64::try_from(row_count(len, columns)).unwrap_or(i64::MAX);
    let first_row = first_row.clamp(0, rows);
    let last_row = (last_row + 1).clamp(0, rows);
    let columns = columns.max(1);
    let start = first_row.saturating_mul(columns as i64);
    let end = last_row.saturating_mul(columns as i64);
    let start = usize::try_from(start).unwrap_or(usize::MAX).min(len);
    let end = usize::try_from(end).unwrap_or(usize::MAX).min(len);
    start.min(end)..end
}

/// Moves `current` one step in `direction` over `len` tiles laid out at
/// `columns` per row, without wrapping: a move off an edge stays put.
pub(crate) fn navigate(len: usize, columns: usize, current: usize, direction: Direction) -> usize {
    if len == 0 {
        return 0;
    }
    let columns = columns.max(1);
    let current = current.min(len - 1);
    match direction {
        Direction::Left => {
            if current.is_multiple_of(columns) {
                current
            } else {
                current - 1
            }
        }
        Direction::Right => {
            if current + 1 >= len || (current + 1).is_multiple_of(columns) {
                current
            } else {
                current + 1
            }
        }
        Direction::Up => {
            if current >= columns {
                current - columns
            } else {
                current
            }
        }
        Direction::Down => {
            let next = current.saturating_add(columns);
            if next < len { next } else { current }
        }
    }
}
