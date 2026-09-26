#![forbid(unsafe_code)]

//! Pure tile-grid arithmetic: how many columns fit a viewport, which rows are
//! visible after a scroll, which tile a point hits and where the keyboard
//! moves. Free of any backend type, so it is exercised by plain unit tests.

use std::ops::Range;

/// A keyboard navigation direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// How many tiles of `tile` px plus `gap` px fit across `viewport_width`, at
/// least one.
pub(crate) fn columns_for_width(viewport_width: i32, tile: i32, gap: i32) -> usize {
    if tile <= 0 {
        return 1;
    }
    let gap = gap.max(0);
    ((viewport_width + gap) / (tile + gap)).max(1) as usize
}

/// The number of rows `len` tiles need at `columns` per row.
pub(crate) fn row_count(len: usize, columns: usize) -> usize {
    len.div_ceil(columns.max(1))
}

/// The full content height, in device pixels, for `len` tiles at `columns` per
/// row. The trailing gap is not counted.
pub(crate) fn content_height_px(len: usize, columns: usize, tile: i32, gap: i32) -> i32 {
    let rows = row_count(len, columns);
    if rows == 0 {
        return 0;
    }
    let gap = gap.max(0);
    rows as i32 * (tile + gap) - gap
}

/// The rows that intersect `[offset, offset + viewport_height)`, clamped to
/// `[0, rows)`.
pub(crate) fn visible_row_range(
    offset: i32,
    viewport_height: i32,
    tile: i32,
    gap: i32,
    rows: usize,
) -> Range<usize> {
    if tile <= 0 || rows == 0 || viewport_height <= 0 {
        return 0..0;
    }
    let stride = tile + gap.max(0);
    let offset = offset.max(0);
    let first = (offset / stride) as usize;
    let last = ((offset + viewport_height + stride - 1) / stride).max(0) as usize;
    first.min(rows)..last.min(rows)
}

/// The tile index at `(x, content_y)`, where `content_y` is the scrolled
/// content coordinate (the point's `y` plus the scroll offset). `None` inside a
/// gap, before the origin or past the model.
pub(crate) fn index_at_point(
    x: i32,
    content_y: i32,
    tile: i32,
    height: i32,
    gap: i32,
    columns: usize,
    len: usize,
) -> Option<usize> {
    if tile <= 0 || height <= 0 || x < 0 || content_y < 0 {
        return None;
    }
    let gap = gap.max(0);
    let columns = columns.max(1) as i32;
    let stride_x = tile + gap;
    let stride_y = height + gap;
    let col = x / stride_x;
    let row = content_y / stride_y;
    if x - col * stride_x >= tile || content_y - row * stride_y >= height {
        return None;
    }
    if col >= columns {
        return None;
    }
    let index = (row * columns + col) as usize;
    (index < len).then_some(index)
}

/// Moves `current` one step in `direction` over `len` tiles laid out at
/// `columns` per row, wrapping at every edge: `Left`/`Right` wrap around the
/// whole model, `Up`/`Down` wrap to the same column in the last/first row
/// (clamped onto that row when it is shorter than `columns`).
pub(crate) fn navigate(len: usize, columns: usize, current: usize, direction: Direction) -> usize {
    if len == 0 {
        return 0;
    }
    let columns = columns.max(1);
    let current = current.min(len - 1);
    match direction {
        Direction::Left => {
            if current == 0 {
                len - 1
            } else {
                current - 1
            }
        }
        Direction::Right => {
            if current + 1 >= len {
                0
            } else {
                current + 1
            }
        }
        Direction::Up => {
            if current >= columns {
                current - columns
            } else {
                wrap_vertical(
                    len,
                    columns,
                    current,
                    row_count(len, columns).saturating_sub(1),
                )
            }
        }
        Direction::Down => {
            let last_row = row_count(len, columns).saturating_sub(1);
            if current / columns < last_row {
                (current + columns).min(len - 1)
            } else {
                wrap_vertical(len, columns, current, 0)
            }
        }
    }
}

/// The tile in `target_row`, same column as `current`, clamped onto that row
/// when it runs short of `columns` tiles.
fn wrap_vertical(len: usize, columns: usize, current: usize, target_row: usize) -> usize {
    let col = current % columns;
    (target_row * columns + col).min(len - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_fit_the_viewport() {
        assert_eq!(columns_for_width(400, 100, 8), 3);
        assert_eq!(columns_for_width(50, 100, 8), 1);
        assert_eq!(columns_for_width(400, 0, 8), 1);
    }

    #[test]
    fn content_height_covers_every_row() {
        assert_eq!(content_height_px(0, 4, 100, 8), 0);
        assert_eq!(content_height_px(4, 4, 100, 8), 100);
        assert_eq!(content_height_px(5, 4, 100, 8), 208);
    }

    #[test]
    fn visible_rows_match_the_scrolled_viewport() {
        assert_eq!(visible_row_range(250, 300, 100, 8, 10), 2..6);
    }

    #[test]
    fn visible_rows_clamp_to_the_model() {
        assert_eq!(visible_row_range(0, 10_000, 100, 8, 3), 0..3);
        assert_eq!(visible_row_range(0, 0, 100, 8, 3), 0..0);
    }

    #[test]
    fn hit_test_finds_the_tile_and_skips_gaps() {
        assert_eq!(index_at_point(50, 50, 100, 100, 8, 3, 10), Some(0));
        assert_eq!(index_at_point(105, 50, 100, 100, 8, 3, 10), None);
        assert_eq!(index_at_point(120, 50, 100, 100, 8, 3, 10), Some(1));
        assert_eq!(index_at_point(50, 50, 100, 100, 8, 3, 0), None);
        assert_eq!(index_at_point(-1, 0, 100, 100, 8, 3, 10), None);
        assert_eq!(index_at_point(120, 50, 100, 100, 8, 3, 1), None);
    }

    #[test]
    fn a_taller_viewport_hits_the_next_row() {
        assert_eq!(index_at_point(50, 108, 100, 100, 8, 3, 10), Some(3));
        assert_eq!(index_at_point(50, 208, 100, 100, 8, 3, 10), None);
    }

    #[test]
    fn left_right_wrap_around_the_whole_model() {
        assert_eq!(navigate(6, 3, 0, Direction::Left), 5);
        assert_eq!(navigate(6, 3, 5, Direction::Right), 0);
        assert_eq!(navigate(6, 3, 2, Direction::Right), 3);
        assert_eq!(navigate(6, 3, 3, Direction::Left), 2);
    }

    #[test]
    fn up_down_move_a_row() {
        assert_eq!(navigate(8, 3, 4, Direction::Up), 1);
        assert_eq!(navigate(8, 3, 1, Direction::Down), 4);
    }

    #[test]
    fn vertical_wrapping_clamps_a_short_row() {
        assert_eq!(navigate(8, 3, 1, Direction::Up), 7);
        assert_eq!(navigate(8, 3, 2, Direction::Up), 7);
        assert_eq!(navigate(8, 3, 7, Direction::Down), 1);
        assert_eq!(navigate(8, 3, 6, Direction::Down), 0);
    }

    #[test]
    fn navigation_is_a_no_op_on_an_empty_model() {
        assert_eq!(navigate(0, 3, 0, Direction::Right), 0);
    }
}
