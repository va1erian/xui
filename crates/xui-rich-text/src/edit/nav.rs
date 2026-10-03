#![forbid(unsafe_code)]

//! The layout questions caret movement needs answered.

use crate::model::DocPos;

/// Line-based navigation, answered by the layout (a paragraph can wrap onto
/// several lines, so "line" is not a model concept).
pub trait LineNav {
    /// The first position of the line containing `pos`.
    fn line_start(&self, pos: DocPos) -> DocPos;

    /// The last position of the line containing `pos`.
    fn line_end(&self, pos: DocPos) -> DocPos;

    /// The position `lines` lines away (negative is up), as close as possible
    /// to `sticky_x` (or to `pos`'s own x when `None`), with the x it landed
    /// at so the caller can keep it sticky.
    fn vertical(&self, pos: DocPos, sticky_x: Option<f32>, lines: i32) -> (DocPos, f32);

    /// How many lines a page movement covers (at least 1).
    fn page_lines(&self) -> i32;
}
