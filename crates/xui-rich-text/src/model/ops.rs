#![forbid(unsafe_code)]

//! Invertible edit operations.
//!
//! [`Document::apply`] performs one [`EditOp`] and returns its inverse, which
//! `apply` again restores the document exactly: same text, same spans, same
//! paragraph styles and the same [`ObjectId`]s. Every text edit is expressed
//! through two primitives, [`EditOp::Remove`] and [`EditOp::Reinsert`], which are
//! each other's inverse; the user-facing ops are thin constructors over them.

mod merge;
mod style;
pub(crate) mod text;

use std::fmt;
use std::ops::Range;

use super::paragraph::Span;
use super::patch::{CharStylePatch, ParaStylePatch};
use super::selection::DocRange;
use super::style::{CharStyleId, ParaStyleId};
use super::{DocPos, Document, InlineImage, ObjectId, PageSetup};

pub use text::Slice;

/// One edit of a [`Document`].
///
/// The first eight variants and `SetPage` are what editing commands create;
/// `Remove` to `RestoreParaStyles` are the exact inverses `apply` hands back, which a [`History`](super::History)
/// stores. They may also be applied directly.
#[derive(Clone, Debug)]
pub enum EditOp {
    /// Inserts `text` at `at`; `\n` starts a new paragraph. The text takes
    /// `style`, or the style a typed character would get there.
    InsertText {
        /// Where to insert.
        at: DocPos,
        /// The text; [`OBJECT_CHAR`](super::OBJECT_CHAR) and `\r` are dropped.
        text: String,
        /// The character style, or `None` for the style at `at`.
        style: Option<CharStyleId>,
    },
    /// Deletes a range, which may span paragraphs (the first paragraph keeps
    /// its style) and removes the objects anchored in it.
    Delete {
        /// The range to delete.
        range: DocRange,
    },
    /// Splits the paragraph at `at`; the new second paragraph has the same
    /// paragraph style and the typing style at `at`.
    SplitParagraph {
        /// Where to split.
        at: DocPos,
    },
    /// Merges paragraph `para` with the one after it, keeping `para`'s style.
    MergeParagraph {
        /// The first of the two paragraphs.
        para: usize,
    },
    /// Changes the attributes `patch` names over `range`.
    SetCharStyle {
        /// The range to restyle.
        range: DocRange,
        /// The attributes to change.
        patch: CharStylePatch,
    },
    /// Changes the attributes `patch` names of the paragraphs in `paras`.
    SetParaStyle {
        /// The paragraph indices.
        paras: Range<usize>,
        /// The attributes to change.
        patch: ParaStylePatch,
    },
    /// Anchors a new object at `at`.
    InsertObject {
        /// Where to anchor it.
        at: DocPos,
        /// The object.
        object: InlineImage,
    },
    /// Replaces an object's size, wrap, alt text or pixels.
    SetObject {
        /// The object.
        id: ObjectId,
        /// Its new value.
        object: InlineImage,
    },
    /// Removes a range exactly; `keep` is the typing style of the paragraph if
    /// it ends up empty.
    Remove {
        /// The range to remove.
        range: DocRange,
        /// The style an emptied paragraph keeps.
        keep: CharStyleId,
    },
    /// Puts removed content back at `at`, objects included.
    Reinsert {
        /// Where to put it.
        at: DocPos,
        /// What was removed.
        content: Slice,
    },
    /// Restores the exact spans of the listed paragraphs.
    RestoreSpans(Vec<(usize, Vec<Span>)>),
    /// Restores the paragraph styles of the listed paragraphs.
    RestoreParaStyles(Vec<(usize, ParaStyleId)>),
    /// Replaces the page setup; its own inverse (with the old setup).
    SetPage(PageSetup),
}

/// Why an edit could not be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// A position is outside the document or inside a character.
    BadPosition(DocPos),
    /// A range's end comes before its start.
    BadRange(DocRange),
    /// A paragraph index or range is outside the document.
    BadParagraphs(Range<usize>),
    /// The object is not in the document.
    UnknownObject(ObjectId),
    /// The style id is not in the document's style table.
    UnknownStyle,
    /// The page setup leaves no room for text or is not finite.
    BadPage,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::BadPosition(p) => write!(f, "bad position {}:{}", p.para, p.byte),
            EditError::BadRange(r) => write!(f, "bad range {:?}", r),
            EditError::BadParagraphs(r) => write!(f, "bad paragraphs {r:?}"),
            EditError::UnknownObject(id) => write!(f, "unknown object {id:?}"),
            EditError::UnknownStyle => f.write_str("unknown style"),
            EditError::BadPage => f.write_str("bad page setup"),
        }
    }
}

impl std::error::Error for EditError {}

impl Document {
    /// Applies `op` and returns the op that undoes it.
    ///
    /// On error the document is unchanged.
    pub fn apply(&mut self, op: EditOp) -> Result<EditOp, EditError> {
        match op {
            EditOp::InsertText { at, text, style } => self.op_insert_text(at, &text, style),
            EditOp::Delete { range } => {
                self.check_range(range)?;
                let keep = self.typing_style(range.start);
                Ok(self.remove(range, keep))
            }
            EditOp::SplitParagraph { at } => self.op_split(at),
            EditOp::MergeParagraph { para } => self.op_merge(para),
            EditOp::SetCharStyle { range, patch } => self.op_set_char_style(range, &patch),
            EditOp::SetParaStyle { paras, patch } => self.op_set_para_style(paras, &patch),
            EditOp::InsertObject { at, object } => self.op_insert_object(at, object),
            EditOp::SetObject { id, object } => self.op_set_object(id, object),
            EditOp::Remove { range, keep } => {
                self.check_range(range)?;
                self.check_char_style(keep)?;
                Ok(self.remove(range, keep))
            }
            EditOp::Reinsert { at, content } => {
                self.check_pos(at)?;
                self.reinsert(at, content)
            }
            EditOp::RestoreSpans(entries) => self.op_restore_spans(entries),
            EditOp::RestoreParaStyles(entries) => self.op_restore_para_styles(entries),
            EditOp::SetPage(page) => {
                page.check().map_err(|_| EditError::BadPage)?;
                Ok(EditOp::SetPage(std::mem::replace(&mut self.page, page)))
            }
        }
    }

    /// Checks that `pos` is inside the document on a `char` boundary.
    pub(crate) fn check_pos(&self, pos: DocPos) -> Result<(), EditError> {
        match self.paragraphs.get(pos.para) {
            Some(p) if pos.byte <= p.text.len() && p.text.is_char_boundary(pos.byte) => Ok(()),
            _ => Err(EditError::BadPosition(pos)),
        }
    }

    pub(crate) fn check_range(&self, range: DocRange) -> Result<(), EditError> {
        self.check_pos(range.start)?;
        self.check_pos(range.end)?;
        if range.start > range.end {
            return Err(EditError::BadRange(range));
        }
        Ok(())
    }

    pub(crate) fn check_char_style(&self, id: CharStyleId) -> Result<(), EditError> {
        if (id.0 as usize) < self.styles.chars().len() {
            Ok(())
        } else {
            Err(EditError::UnknownStyle)
        }
    }

    /// The style a character typed at `pos` gets: that of the character
    /// before it, or of the first one at the start of a paragraph.
    ///
    /// # Panics
    /// If `pos` is outside the document.
    pub fn typing_style(&self, pos: DocPos) -> CharStyleId {
        self.paragraphs[pos.para].typing_style(pos.byte)
    }

    /// The per-paragraph byte ranges `range` covers. An empty paragraph the
    /// range extends past is covered by an empty range, so its typing style
    /// follows a restyle of the selection.
    pub(crate) fn covered(&self, range: DocRange) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for para in range.start.para..=range.end.para.min(self.paragraphs.len() - 1) {
            let len = self.paragraphs[para].text.len();
            let lo = if para == range.start.para {
                range.start.byte
            } else {
                0
            };
            let hi = if para == range.end.para {
                range.end.byte
            } else {
                len
            };
            if lo < hi || (len == 0 && para < range.end.para) {
                out.push((para, lo, hi));
            }
        }
        out
    }
}
