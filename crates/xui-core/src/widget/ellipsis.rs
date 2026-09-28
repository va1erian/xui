#![forbid(unsafe_code)]

//! Pure single-line end-ellipsis truncation: given a width budget and a way
//! to measure a candidate string, decides how much of the text fits.
//!
//! Kept separate from any one widget's painter and tested with a fake
//! measurer, in the style of [`super::flow_text::layout`]'s pure layout
//! tests, so the truncation decision needs no backend. Shared by
//! [`super::listview`] and [`super::statusbar`], which both draw single-line
//! cell text that must not run into a neighboring cell.

use std::borrow::Cow;

/// The end-of-text ellipsis appended to a truncated cell.
pub(crate) const ELLIPSIS: char = '\u{2026}';

/// Returns `text` unchanged when `measure` reports it already fits inside
/// `max_width`; otherwise the longest prefix that, with a trailing ellipsis,
/// still fits — or an empty string when not even the ellipsis fits.
///
/// `measure` returns the pixel width of the string it is given, and is
/// assumed monotonic: a longer prefix never measures narrower. Truncation is
/// by `char`, not by grapheme cluster, so a truncation that lands inside a
/// combining sequence is possible but rare for the plain labels a list cell
/// draws.
pub(crate) fn truncate<'a>(
    text: &'a str,
    max_width: i32,
    measure: &mut dyn FnMut(&str) -> i32,
) -> Cow<'a, str> {
    if text.is_empty() || measure(text) <= max_width {
        return Cow::Borrowed(text);
    }
    if max_width <= 0 {
        return Cow::Borrowed("");
    }
    let mut buf = [0u8; 4];
    let ellipsis = ELLIPSIS.encode_utf8(&mut buf);
    if measure(ellipsis) > max_width {
        return Cow::Borrowed("");
    }

    // `boundaries[k]` is the byte offset after the first `k` characters, for
    // `k` in `0..=chars.len()`.
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect();

    // The largest kept-character count whose prefix, plus the ellipsis,
    // still fits: `fits(0)` holds (checked above), so this always finds one.
    let mut lo = 0i64;
    let mut hi = boundaries.len() as i64 - 1;
    let mut kept = 0usize;
    while lo <= hi {
        let mid = ((lo + hi) / 2) as usize;
        let candidate = format!("{}{}", &text[..boundaries[mid]], ellipsis);
        if measure(&candidate) <= max_width {
            kept = mid;
            lo = mid as i64 + 1;
        } else {
            hi = mid as i64 - 1;
        }
    }
    Cow::Owned(format!("{}{}", &text[..boundaries[kept]], ellipsis))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A measurer with a fixed 10px advance per character.
    fn fixed(text: &str) -> i32 {
        text.chars().count() as i32 * 10
    }

    #[test]
    fn text_that_fits_is_returned_unchanged() {
        let out = truncate("hi", 100, &mut fixed);
        assert_eq!(out, "hi");
        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn overflowing_text_is_cut_and_ellipsized() {
        // "Senorita" is 8 chars (80px); a 45px budget keeps 3 plus the
        // ellipsis (40px), since a 4th char would reach 50px.
        let out = truncate("Senorita", 45, &mut fixed);
        assert_eq!(out, "Sen\u{2026}");
    }

    #[test]
    fn a_budget_too_small_for_even_the_ellipsis_yields_empty() {
        let out = truncate("Senorita", 5, &mut fixed);
        assert_eq!(out, "");
    }

    #[test]
    fn a_non_positive_budget_yields_empty() {
        let out = truncate("Senorita", 0, &mut fixed);
        assert_eq!(out, "");
    }

    #[test]
    fn empty_text_is_returned_unchanged() {
        let out = truncate("", 100, &mut fixed);
        assert_eq!(out, "");
    }

    #[test]
    fn truncation_respects_character_boundaries() {
        // Multi-byte characters must not be sliced mid-codepoint.
        let out = truncate("Señorita", 45, &mut fixed);
        assert_eq!(out, "Señ\u{2026}");
    }

    #[test]
    fn exact_fit_needs_no_ellipsis() {
        let out = truncate("abcde", 50, &mut fixed);
        assert_eq!(out, "abcde");
    }
}
