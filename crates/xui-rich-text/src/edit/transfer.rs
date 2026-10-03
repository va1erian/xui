#![forbid(unsafe_code)]

//! Cut, copy and paste.

use super::clipboard::{self, Clipboard};
use super::controller::{EditorState, Effect};
use crate::model::{EditOp, Selection};

impl EditorState {
    pub(super) fn copy(&mut self, clipboard: &dyn Clipboard) -> Effect {
        let Some(range) = self.selection_range().filter(|r| !r.is_empty()) else {
            return Effect::NONE;
        };
        let Ok(fragment) = self.doc.extract_fragment(range) else {
            return Effect::NONE;
        };
        let mut plain = fragment.to_plain_text();
        if plain.is_empty() {
            plain = match self.selection {
                Selection::Object(id) => self
                    .doc
                    .objects()
                    .get(id)
                    .map(|o| o.alt.clone())
                    .filter(|alt| !alt.is_empty())
                    .unwrap_or_else(|| "image".to_owned()),
                Selection::Text { .. } => return Effect::NONE,
            };
        }
        clipboard.set_text(&plain);
        clipboard::store(&plain, fragment);
        Effect {
            copied: Some(plain),
            ..Effect::NONE
        }
    }

    pub(super) fn cut(&mut self, clipboard: &dyn Clipboard) -> Effect {
        let copy = self.copy(clipboard);
        if copy.copied.is_none() {
            return Effect::NONE;
        }
        let mut effect = self.delete_selection_step();
        effect.copied = copy.copied;
        effect
    }

    pub(super) fn paste(&mut self, clipboard: &dyn Clipboard) -> Effect {
        let Some(text) = clipboard.text() else {
            return Effect::NONE;
        };
        let fragment = clipboard::matching(&text);
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        let style = self.pending_style(range.start);
        self.run(|cx| {
            if !range.is_empty() {
                cx.apply(EditOp::Delete { range })?;
            }
            let end = match &fragment {
                Some(fragment) => cx.insert_fragment(range.start, fragment)?,
                None => {
                    let text = crate::model::ops::text::sanitize(&text);
                    let end = crate::model::ops::text::end_after(range.start, &text);
                    cx.apply(EditOp::InsertText {
                        at: range.start,
                        text,
                        style: Some(style),
                    })?;
                    end
                }
            };
            Ok(Some(Selection::caret(end)))
        })
    }
}
