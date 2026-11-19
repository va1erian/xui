#![forbid(unsafe_code)]

//! How a [`StatusBar`](super::StatusBar) divides its width between its parts.

use crate::units::Dip;

/// How wide a status bar part is.
///
/// A bar's parts default to [`PartWidth::Fill`]`(1)`, which shares the
/// width equally.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PartWidth {
    /// A share of the width left once the fixed and auto parts are placed,
    /// proportional to its number: 1 and 3 give a quarter and three
    /// quarters. `Fill(0)` takes no room.
    Fill(u32),
    /// A fixed width in design units (a clock, a cursor position).
    Fixed(Dip),
    /// As wide as the part's text plus its padding.
    Auto,
}

impl Default for PartWidth {
    fn default() -> PartWidth {
        PartWidth::Fill(1)
    }
}

/// The `(left, right)` span of each part along a bar `total` pixels wide, in
/// pixels from the bar's left edge.
///
/// `widths[i]` sizes part `i` (parts past its end are `Fill(1)`); `fit` is
/// the padded text width of every part, used by [`PartWidth::Auto`]; `count`
/// is the number of parts. Spans are contiguous and stay inside the bar: a bar
/// too narrow for its fixed and fit parts clips the last ones.
pub(super) fn spans(
    widths: &[PartWidth],
    count: usize,
    total: i32,
    dpi: u32,
    fit: &mut dyn FnMut(usize) -> i32,
) -> Vec<(i32, i32)> {
    let width_of = |index: usize| widths.get(index).copied().unwrap_or_default();
    let mut rigid = vec![0i32; count];
    let mut weights = vec![0i64; count];
    for index in 0..count {
        match width_of(index) {
            PartWidth::Fill(share) => weights[index] = i64::from(share),
            PartWidth::Fixed(width) => rigid[index] = width.to_px(dpi).value().max(0),
            PartWidth::Auto => rigid[index] = fit(index).max(0),
        }
    }
    let room = i64::from((total - rigid.iter().sum::<i32>()).max(0));
    let weight_total: i64 = weights.iter().sum();

    let mut spans = Vec::with_capacity(count);
    let (mut rigid_before, mut weight_before) = (0i64, 0i64);
    let mut left = 0;
    for index in 0..count {
        rigid_before += i64::from(rigid[index]);
        weight_before += weights[index];
        // Cumulative, so the shares sum to the room exactly (no rounding drift)
        // and equal weights reproduce an equal split.
        let share = if weight_total == 0 {
            0
        } else {
            room * weight_before / weight_total
        };
        let right = ((rigid_before + share) as i32).min(total);
        spans.push((left, right.max(left)));
        left = right.max(left);
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans_of(widths: &[PartWidth], count: usize, total: i32) -> Vec<(i32, i32)> {
        spans(widths, count, total, 96, &mut |index| {
            10 * (index as i32 + 1)
        })
    }

    #[test]
    fn the_default_splits_the_width_equally() {
        assert_eq!(
            spans_of(&[], 3, 300),
            vec![(0, 100), (100, 200), (200, 300)]
        );
        // The same integer rounding as `width * index / count`.
        assert_eq!(spans_of(&[], 3, 100), vec![(0, 33), (33, 66), (66, 100)]);
    }

    #[test]
    fn a_fixed_part_keeps_its_width_and_the_rest_share_the_remainder() {
        let widths = [PartWidth::Fill(1), PartWidth::Fixed(Dip(60.0))];
        assert_eq!(spans_of(&widths, 2, 300), vec![(0, 240), (240, 300)]);
    }

    #[test]
    fn weights_divide_the_room_proportionally() {
        let widths = [PartWidth::Fill(1), PartWidth::Fill(3)];
        assert_eq!(spans_of(&widths, 2, 400), vec![(0, 100), (100, 400)]);
    }

    #[test]
    fn a_fit_part_takes_its_measured_width() {
        let widths = [PartWidth::Auto, PartWidth::Fill(1), PartWidth::Auto];
        assert_eq!(
            spans_of(&widths, 3, 200),
            vec![(0, 10), (10, 170), (170, 200)]
        );
    }

    #[test]
    fn a_bar_too_narrow_for_its_rigid_parts_clips_the_last_ones() {
        let widths = [PartWidth::Fixed(Dip(80.0)), PartWidth::Fixed(Dip(80.0))];
        assert_eq!(spans_of(&widths, 2, 100), vec![(0, 80), (80, 100)]);
        assert_eq!(spans_of(&widths, 2, 50), vec![(0, 50), (50, 50)]);
    }

    #[test]
    fn spans_scale_with_the_dpi() {
        let widths = [PartWidth::Fixed(Dip(50.0)), PartWidth::Fill(1)];
        let at_144 = spans(&widths, 2, 300, 144, &mut |_| 0);
        assert_eq!(at_144, vec![(0, 75), (75, 300)]);
    }

    #[test]
    fn all_zero_weights_leave_the_room_empty() {
        let widths = [PartWidth::Fill(0), PartWidth::Fill(0)];
        assert_eq!(spans_of(&widths, 2, 100), vec![(0, 0), (0, 0)]);
    }
}
