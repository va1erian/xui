#![forbid(unsafe_code)]

//! A paragraph: its text, the character style of every byte, and the objects
//! it anchors.

use std::ops::Range;

use super::object::{OBJECT_CHAR, ObjectId};
use super::style::{CharStyleId, ParaStyleId};
use super::table::CellMark;

/// A run of `len` bytes in one character style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// The run's length in bytes.
    pub len: usize,
    /// The run's style.
    pub style: CharStyleId,
}

/// One paragraph of a [`Document`](super::Document).
///
/// Invariants (see [`Paragraph::check`]):
/// * the spans cover `text` exactly, every span is non-empty and ends on a
///   `char` boundary, and adjacent spans have different styles;
/// * an empty paragraph has exactly one empty span, which carries the style
///   the next typed character gets;
/// * the `n`-th [`OBJECT_CHAR`] in `text` is anchored by `anchors[n]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    pub(crate) text: String,
    pub(crate) spans: Vec<Span>,
    pub(crate) anchors: Vec<ObjectId>,
    pub(crate) style: ParaStyleId,
    pub(crate) cell: Option<CellMark>,
}

impl Paragraph {
    /// A paragraph of `text` (which must not contain [`OBJECT_CHAR`]) in one
    /// character style.
    pub fn new(text: impl Into<String>, style: ParaStyleId, chars: CharStyleId) -> Paragraph {
        let text = text.into();
        debug_assert!(!text.contains(OBJECT_CHAR));
        Paragraph {
            spans: vec![Span {
                len: text.len(),
                style: chars,
            }],
            text,
            anchors: Vec::new(),
            style,
            cell: None,
        }
    }

    /// The same paragraph inside a table cell (`None` for outside any table).
    pub fn with_cell(mut self, cell: Option<CellMark>) -> Paragraph {
        self.cell = cell;
        self
    }

    /// The paragraph's text, with [`OBJECT_CHAR`] at each object's anchor.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The character-style runs, in order, covering the text.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The anchored objects, in text order.
    pub fn anchors(&self) -> &[ObjectId] {
        &self.anchors
    }

    /// The paragraph style.
    pub fn style(&self) -> ParaStyleId {
        self.style
    }

    /// The table cell the paragraph is in, if any.
    pub fn cell(&self) -> Option<CellMark> {
        self.cell
    }

    /// The byte range and style of each run, in order.
    pub fn runs(&self) -> impl Iterator<Item = (Range<usize>, CharStyleId)> + '_ {
        let mut start = 0;
        self.spans.iter().map(move |span| {
            let range = start..start + span.len;
            start = range.end;
            (range, span.style)
        })
    }

    /// The style of the character at `byte`, or of the one before it at the
    /// end of the text (what a character typed there would get).
    pub fn style_at(&self, byte: usize) -> CharStyleId {
        let mut end = 0;
        for span in &self.spans {
            end += span.len;
            if byte < end {
                return span.style;
            }
        }
        self.spans.last().map_or(CharStyleId::DEFAULT, |s| s.style)
    }

    /// The objects anchored in the text with their byte offsets, in order.
    pub fn objects(&self) -> impl Iterator<Item = (usize, ObjectId)> + '_ {
        self.text
            .match_indices(OBJECT_CHAR)
            .map(|(byte, _)| byte)
            .zip(self.anchors.iter().copied())
    }

    /// Checks the invariants, describing the first one broken.
    pub fn check(&self) -> Result<(), String> {
        if self.spans.is_empty() {
            return Err("no spans".into());
        }
        if self.text.is_empty() {
            if self.spans.len() != 1 || self.spans[0].len != 0 {
                return Err("an empty paragraph needs one empty span".into());
            }
        } else {
            let mut end = 0;
            for (i, span) in self.spans.iter().enumerate() {
                if span.len == 0 {
                    return Err(format!("span {i} is empty"));
                }
                if i > 0 && self.spans[i - 1].style == span.style {
                    return Err(format!("spans {} and {i} share a style", i - 1));
                }
                end += span.len;
                if !self.text.is_char_boundary(end.min(self.text.len())) {
                    return Err(format!("span {i} ends inside a character"));
                }
            }
            if end != self.text.len() {
                return Err(format!("spans cover {end} of {} bytes", self.text.len()));
            }
        }
        let count = self.text.matches(OBJECT_CHAR).count();
        if count != self.anchors.len() {
            return Err(format!(
                "{count} object chars, {} anchors",
                self.anchors.len()
            ));
        }
        Ok(())
    }
}
