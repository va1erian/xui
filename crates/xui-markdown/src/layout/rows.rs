#![forbid(unsafe_code)]

//! Where a code line too wide for its box is cut into rows.
//!
//! The line is shaped once, unwrapped; each row ends at the last character
//! that fits, or after the last space in the row's second half so a word is
//! not split when a space is near. The spaces at a cut stay at the end of the
//! row they follow, so no row is only blank. A row holds at least one
//! character, so a box narrower than a character still makes progress.
//!
//! Each row asks the shaped line a few questions, and a backend may answer
//! each by walking the whole line, so a line longer than [`WRAP_LIMIT`] (minified
//! data, say) is clipped rather than wrapped: wrapping it would cost the
//! square of its length.

use std::ops::Range;

use xui_core::backend::TextLayout;

use super::code::{expanded_offset, original_offset};

/// The longest line, in bytes, that wraps.
pub(super) const WRAP_LIMIT: usize = 4096;

/// The byte ranges of `line` shown on each row of a box `avail` pixels wide;
/// `whole` is `line` shaped unwrapped (tabs widened). One row when it fits.
pub(super) fn rows(whole: &dyn TextLayout, line: &str, avail: f32) -> Vec<Range<usize>> {
    let unwrapped = avail.is_nan() || avail <= 0.0 || line.len() > WRAP_LIMIT;
    if unwrapped || whole.width() <= avail {
        return std::iter::once(0..line.len()).collect();
    }
    let mid = whole.height() / 2.0;
    let mut rows = Vec::new();
    let (mut start, mut left) = (0, 0.0);
    while start < line.len() {
        let hit = whole.hit_test_point(left + avail, mid);
        let mut end = if hit.inside {
            original_offset(line, hit.byte_index).max(start)
        } else {
            line.len()
        };
        // The hit rounds to the nearer caret: a character it rounded up past
        // may not fit, and a backend that misses the point reports none.
        if x_at(whole, line, end) - left > avail + 0.5 {
            end = last_fit(whole, line, start..end, left + avail + 0.5);
        }
        if end == start {
            end = next(line, start);
        }
        end = absorb_spaces(line, after_space(line, start, end));
        rows.push(start..end);
        left = x_at(whole, line, end);
        start = end;
    }
    rows
}

/// The last character boundary in `span` (after its start) whose pen
/// position is at most `limit`, or the span's start when none is: a binary
/// search, as pen positions grow along left-to-right text.
fn last_fit(whole: &dyn TextLayout, line: &str, span: Range<usize>, limit: f32) -> usize {
    let ends: Vec<usize> = line[span.clone()]
        .char_indices()
        .map(|(at, c)| span.start + at + c.len_utf8())
        .collect();
    let fits = ends.partition_point(|&end| x_at(whole, line, end) <= limit);
    fits.checked_sub(1).map_or(span.start, |i| ends[i])
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

/// `end` moved past the spaces that follow it: they end this row instead of
/// starting the next one.
fn absorb_spaces(line: &str, end: usize) -> usize {
    end + line[end..].len() - line[end..].trim_start_matches(' ').len()
}

/// The pen position before byte `at` of `line`: the right edge of the
/// character before it (the whole prefix when that edge is not reported, as
/// some backends leave a trailing space out of a selection).
fn x_at(whole: &dyn TextLayout, line: &str, at: usize) -> f32 {
    if at == 0 {
        return 0.0;
    }
    let right = |from: usize| {
        whole
            .selection_rects(expanded_offset(line, from), expanded_offset(line, at))
            .iter()
            .map(|r| r.right as f32)
            .reduce(f32::max)
    };
    right(previous(line, at))
        .filter(|&x| x > 0.0)
        .or_else(|| right(0))
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
