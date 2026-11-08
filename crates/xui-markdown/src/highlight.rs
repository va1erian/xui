#![forbid(unsafe_code)]

//! Colouring code blocks: an app-supplied [`Highlighter`] cuts each line into
//! classed spans, and a [`CodePalette`](crate::CodePalette) gives each class
//! its colour.
//!
//! The crate has no lexer of its own: an app adapts the one it already has
//! (a code editor's, say) in a few lines.

use std::ops::Range;

/// What a span of code is, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CodeClass {
    /// A keyword (`fn`, `if`, `self`).
    Keyword,
    /// A function or message name.
    Function,
    /// A string or character literal.
    String,
    /// A comment.
    Comment,
    /// A number literal.
    Number,
    /// A type name.
    Type,
}

/// A classed span of one line of code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeSpan {
    /// Byte range in the line, on `char` boundaries.
    pub range: Range<usize>,
    /// What the span is.
    pub class: CodeClass,
}

/// Colours code blocks a line at a time, carrying state (an open block
/// comment, a multi-line string) from one line to the next.
pub trait Highlighter {
    /// The classed spans of `line` (no line terminator) of a block whose
    /// info string starts with `lang` (empty when none was given), given the
    /// `state` the previous line ended in (`0` for the first line). Returns
    /// the spans, in order and not overlapping, and the state the line ends
    /// in. Text outside every span is drawn plain. Spans out of order,
    /// overlapping, or off `char` boundaries are skipped.
    fn line(&self, lang: &str, line: &str, state: u32) -> (Vec<CodeSpan>, u32);
}
