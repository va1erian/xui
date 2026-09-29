#![forbid(unsafe_code)]

//! The edit's shared coordinate arithmetic.
//!
//! Painting and hit-testing must agree on where a character sits, so both use
//! these: [`text_x`] places a text position inside the field, and
//! [`pointer_text_x`] turns a pointer back into a text position. They are
//! inverses, so the caret's painted x and a click's target cannot drift.

use super::PADDING;

/// The padding inside the field border, in device pixels.
pub(super) fn padding(dpi: u32) -> i32 {
    PADDING.to_px(dpi).value()
}

/// The device-pixel x of a text position `prefix_width` pixels into the text,
/// for a field whose left edge is `left` and whose text is scrolled by
/// `scroll`. Painting uses it for the caret and the selection edges.
pub(super) fn text_x(left: i32, prefix_width: i32, dpi: u32, scroll: i32) -> i32 {
    left + padding(dpi) + prefix_width - scroll
}

/// The text-space x a node-local pointer `local_x` points at: the inverse of
/// [`text_x`], with the field's left edge removed. Hit-testing uses it.
pub(super) fn pointer_text_x(local_x: i32, dpi: u32, scroll: i32) -> i32 {
    local_x - padding(dpi) + scroll
}
