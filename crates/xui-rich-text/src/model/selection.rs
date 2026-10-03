#![forbid(unsafe_code)]

//! Selections and ranges of a document.

use super::{DocPos, ObjectId};

/// An ordered span of the document: `start <= end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocRange {
    /// The first position in the range.
    pub start: DocPos,
    /// The position just after the range.
    pub end: DocPos,
}

impl DocRange {
    /// The range between `a` and `b`, in whichever order they come.
    pub fn new(a: DocPos, b: DocPos) -> DocRange {
        if a <= b {
            DocRange { start: a, end: b }
        } else {
            DocRange { start: b, end: a }
        }
    }

    /// Whether the range covers nothing.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether `pos` lies in `start..=end`.
    pub fn contains(&self, pos: DocPos) -> bool {
        self.start <= pos && pos <= self.end
    }
}

/// What is selected: a run of text, or one image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selection {
    /// Text from `anchor` (where the selection started) to `head` (the caret,
    /// which may come before the anchor).
    Text {
        /// The fixed end.
        anchor: DocPos,
        /// The moving end, where the caret is drawn.
        head: DocPos,
    },
    /// An anchored image, selected as a whole.
    Object(ObjectId),
}

impl Selection {
    /// A collapsed selection: a caret at `pos`.
    pub fn caret(pos: DocPos) -> Selection {
        Selection::Text {
            anchor: pos,
            head: pos,
        }
    }

    /// A text selection from `anchor` to `head`.
    pub fn text(anchor: DocPos, head: DocPos) -> Selection {
        Selection::Text { anchor, head }
    }

    /// The selected text range in document order, or `None` for an image.
    pub fn ordered(&self) -> Option<DocRange> {
        match *self {
            Selection::Text { anchor, head } => Some(DocRange::new(anchor, head)),
            Selection::Object(_) => None,
        }
    }

    /// Whether this is a caret: text selected, but none of it.
    pub fn is_collapsed(&self) -> bool {
        matches!(self, Selection::Text { anchor, head } if anchor == head)
    }

    /// The caret end of a text selection.
    pub fn head(&self) -> Option<DocPos> {
        match *self {
            Selection::Text { head, .. } => Some(head),
            Selection::Object(_) => None,
        }
    }
}

impl Default for Selection {
    fn default() -> Selection {
        Selection::caret(DocPos::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_swaps_a_backwards_selection() {
        let sel = Selection::text(DocPos::new(1, 4), DocPos::new(0, 2));
        let range = sel.ordered().unwrap();
        assert_eq!(range.start, DocPos::new(0, 2));
        assert_eq!(range.end, DocPos::new(1, 4));
        assert!(!sel.is_collapsed());
        assert!(Selection::caret(DocPos::new(0, 0)).is_collapsed());
    }
}
