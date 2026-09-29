#![forbid(unsafe_code)]

//! Word boundaries for the edit model's Ctrl+arrow moves and word deletes.
//!
//! A word is a maximal non-whitespace run, so punctuation moves with the token
//! it touches. Moving left skips any whitespace to the left and then the run
//! before it; moving right skips whitespace to the right and then the run there
//! — the two are mirror images, which keeps Ctrl+Left and Ctrl+Right stable
//! when a caret sits in a gap.

/// The start of the word to the left of `from`: skip any whitespace, then the
/// non-whitespace run before it.
pub(super) fn word_left(chars: &[char], from: usize) -> usize {
    let mut at = from.min(chars.len());
    while at > 0 && chars[at - 1].is_whitespace() {
        at -= 1;
    }
    while at > 0 && !chars[at - 1].is_whitespace() {
        at -= 1;
    }
    at
}

/// The end of the word to the right of `from`: skip any whitespace, then the
/// non-whitespace run after it.
pub(super) fn word_right(chars: &[char], from: usize) -> usize {
    let mut at = from.min(chars.len());
    while at < chars.len() && chars[at].is_whitespace() {
        at += 1;
    }
    while at < chars.len() && !chars[at].is_whitespace() {
        at += 1;
    }
    at
}
