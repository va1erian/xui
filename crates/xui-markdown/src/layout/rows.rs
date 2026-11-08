#![forbid(unsafe_code)]

//! Where a code line too wide for its box is cut into rows.
//!
//! The line is shaped once, unwrapped; each row ends at the last character
//! that fits, or after the last space in the row's second half so a word is
//! not split when a space is near. A row holds at least one character, so a
//! box narrower than a character still makes progress.

use std::ops::Range;

use xui_core::backend::TextLayout;

use super::code::{expanded_offset, original_offset};

/// The byte ranges of `line` shown on each row of a box `avail` pixels wide;
/// `whole` is `line` shaped unwrapped (tabs widened). One row when it fits.
pub(super) fn rows(whole: &dyn TextLayout, line: &str, avail: f32) -> Vec<Range<usize>> {
    if avail.is_nan() || avail <= 0.0 || whole.width() <= avail {
        return std::iter::once(0..line.len()).collect();
    }
    let mid = whole.height() / 2.0;
    let mut rows = Vec::new();
    let mut start = 0;
    while start < line.len() {
        let left = x_at(whole, line, start);
        let hit = whole.hit_test_point(left + avail, mid);
        if !hit.inside || hit.byte_index >= expanded_offset(line, line.len()) {
            rows.push(start..line.len());
            break;
        }
        let mut end = original_offset(line, hit.byte_index).max(start);
        // The hit rounds to the nearer caret: a character it rounded up past
        // may not fit.
        while end > start && x_at(whole, line, end) - left > avail + 0.5 {
            end = previous(line, end);
        }
        if end == start {
            end = next(line, start);
        }
        end = after_space(line, start, end);
        rows.push(start..end);
        start = end;
    }
    rows
}

/// Where the row `start..end` ends to keep a word whole: after its last
/// space, if that is in the row's second half.
fn after_space(line: &str, start: usize, end: usize) -> usize {
    if end >= line.len() {
        return end;
    }
    match line[start..end].rfind(' ') {
        Some(space) if space + 1 > (end - start) / 2 => start + space + 1,
        _ => end,
    }
}

/// The pen position before byte `at` of `line`.
fn x_at(whole: &dyn TextLayout, line: &str, at: usize) -> f32 {
    if at == 0 {
        return 0.0;
    }
    whole
        .selection_rects(0, expanded_offset(line, at))
        .iter()
        .map(|r| r.right as f32)
        .reduce(f32::max)
        .unwrap_or(0.0)
}

fn previous(line: &str, at: usize) -> usize {
    line[..at].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn next(line: &str, at: usize) -> usize {
    line[at..]
        .chars()
        .next()
        .map_or(line.len(), |c| at + c.len_utf8())
}
