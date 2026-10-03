#![forbid(unsafe_code)]

//! Password masking for [`Edit`](super::Edit): what a masked field shows in
//! place of its text.
//!
//! The model counts caret positions in chars, so the mask is one glyph per
//! char: a char index into the text is the same index into the mask, and the
//! caret, the selection and hit-testing all measure the masked string without
//! any mapping.

use std::borrow::Cow;

/// The glyph a masked field shows for each character (U+2022 BULLET).
pub(super) const MASK: char = '\u{2022}';

/// The string a field displays and measures: `text` itself, or one [`MASK`]
/// per char when `masked`.
pub(super) fn display(text: &str, masked: bool) -> Cow<'_, str> {
    if masked {
        Cow::Owned(std::iter::repeat_n(MASK, text.chars().count()).collect())
    } else {
        Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn an_unmasked_field_shows_its_text_unchanged() {
        assert!(matches!(display("secret", false), Cow::Borrowed("secret")));
    }

    #[test]
    fn a_masked_field_shows_one_glyph_per_char() {
        assert_eq!(display("", true), "");
        assert_eq!(display("abc", true), "\u{2022}\u{2022}\u{2022}");
        // Multi-byte chars are one caret step each, so one glyph each.
        assert_eq!(display("é€😀", true).chars().count(), 3);
    }

    proptest! {
        #[test]
        fn the_mask_has_as_many_caret_positions_as_the_text(text in ".*") {
            let masked = display(&text, true);
            prop_assert_eq!(masked.chars().count(), text.chars().count());
            prop_assert!(masked.chars().all(|c| c == MASK));
        }
    }
}
