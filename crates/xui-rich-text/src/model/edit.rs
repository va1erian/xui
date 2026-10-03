#![forbid(unsafe_code)]

//! Conveniences the view calls on a [`Document`]: sizes, plain text, and the
//! compound "type over the selection" edit.

use super::object::OBJECT_CHAR;
use super::ops::text::{end_after, sanitize};
use super::ops::{EditError, EditOp};
use super::paragraph::Paragraph;
use super::selection::{DocRange, Selection};
use super::{DocPos, Document, ObjectId};

/// The text of `paragraphs` without object anchors, joined by `\n`.
pub(crate) fn plain_text(paragraphs: &[Paragraph]) -> String {
    let lines: Vec<String> = paragraphs
        .iter()
        .map(|p| p.text.replace(OBJECT_CHAR, ""))
        .collect();
    lines.join("\n")
}

impl Document {
    /// The number of paragraphs (at least one).
    pub fn paragraph_count(&self) -> usize {
        self.paragraphs.len()
    }

    /// The whole text, objects dropped and paragraphs joined by `\n`.
    pub fn to_plain_text(&self) -> String {
        plain_text(&self.paragraphs)
    }

    /// The text in `range`, objects dropped and paragraph breaks as `\n`.
    pub fn text_in(&self, range: DocRange) -> Result<String, EditError> {
        self.check_range(range)?;
        Ok(plain_text(&self.slice_range(range).paras))
    }

    /// The position of the character anchoring object `id`.
    pub fn object_pos(&self, id: ObjectId) -> Option<DocPos> {
        self.paragraphs.iter().enumerate().find_map(|(para, p)| {
            p.objects()
                .find(|&(_, anchor)| anchor == id)
                .map(|(byte, _)| DocPos::new(para, byte))
        })
    }

    /// The range a selection covers; an object selection covers its anchor
    /// character.
    pub fn selection_range(&self, sel: &Selection) -> Option<DocRange> {
        match *sel {
            Selection::Text { anchor, head } => Some(DocRange::new(anchor, head)),
            Selection::Object(id) => {
                let start = self.object_pos(id)?;
                let end = DocPos::new(start.para, start.byte + OBJECT_CHAR.len_utf8());
                Some(DocRange { start, end })
            }
        }
    }

    /// Replaces the selection with `text` in the style typed text would get at
    /// its start. Returns the inverse ops, in the order they were produced
    /// (apply them in reverse to undo), and the caret after the text.
    pub fn insert_text(
        &mut self,
        sel: &Selection,
        text: &str,
    ) -> Result<(Vec<EditOp>, DocPos), EditError> {
        let range = self
            .selection_range(sel)
            .ok_or(EditError::BadPosition(DocPos::default()))?;
        self.check_range(range)?;
        let style = self.typing_style(range.start);
        let mut inverses = Vec::new();
        if !range.is_empty() {
            inverses.push(self.apply(EditOp::Delete { range })?);
        }
        let text = sanitize(text);
        let caret = end_after(range.start, &text);
        inverses.push(self.apply(EditOp::InsertText {
            at: range.start,
            text,
            style: Some(style),
        })?);
        Ok((inverses, caret))
    }
}
