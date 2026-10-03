#![forbid(unsafe_code)]

//! Typing and deleting.

use std::time::Duration;

use super::controller::{EditorState, Effect};
use super::tx::Dirty;
use crate::model::grapheme::{next_grapheme, next_word, prev_grapheme, prev_word};
use crate::model::ops::text::{end_after, sanitize};
use crate::model::{
    BlockKind, CharStyleId, DocPos, DocRange, EditContext, EditOp, ParaStylePatch, Selection,
};

impl EditorState {
    /// The style typed text gets at `pos`, with the pending change applied.
    pub(super) fn pending_style(&mut self, pos: DocPos) -> CharStyleId {
        let base = self.doc.typing_style(pos);
        match &self.pending {
            None => base,
            Some(patch) => {
                let style = patch.apply(self.doc.styles().char(base));
                self.doc.styles_mut().intern_char(style)
            }
        }
    }

    /// Applies one op through `History::edit`, so typing and backspace runs
    /// coalesce. `span` is the first and last paragraph it touches.
    fn edit_one(
        &mut self,
        op: EditOp,
        after: Selection,
        span: (usize, usize),
        now: Duration,
        keep_pending: bool,
    ) -> Effect {
        let before_len = self.doc.paragraph_count();
        let ctx = EditContext::new(self.selection, after).at(now);
        if self.history.edit(&mut self.doc, op, ctx).is_err() {
            return Effect::NONE;
        }
        let pending = if keep_pending {
            self.pending.take()
        } else {
            None
        };
        let mut dirty = Dirty::default();
        dirty.note(span.0, span.1);
        let effect = self.finish(after, before_len, dirty);
        self.pending = pending;
        effect
    }

    /// Types `text` over the selection.
    pub(super) fn type_text(&mut self, text: &str, now: Duration) -> Effect {
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        let text = sanitize(text);
        if text.is_empty() && range.is_empty() {
            return Effect::NONE;
        }
        let style = self.pending_style(range.start);
        let end = end_after(range.start, &text);
        let after = Selection::caret(end);
        if range.is_empty() {
            let op = EditOp::InsertText {
                at: range.start,
                text,
                style: Some(style),
            };
            return self.edit_one(op, after, (range.start.para, end.para), now, true);
        }
        self.run(|cx| {
            cx.apply(EditOp::Delete { range })?;
            cx.apply(EditOp::InsertText {
                at: range.start,
                text,
                style: Some(style),
            })?;
            Ok(Some(after))
        })
    }

    /// Enter.
    pub(super) fn insert_paragraph(&mut self) -> Effect {
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        let at = range.start;
        let empty_item =
            range.is_empty() && self.doc.paragraphs()[at.para].text().is_empty() && self.in_list();
        self.run(|cx| {
            if empty_item {
                cx.apply(EditOp::SetParaStyle {
                    paras: at.para..at.para + 1,
                    patch: ParaStylePatch::list(None),
                })?;
                return Ok(Some(Selection::caret(at)));
            }
            if !range.is_empty() {
                cx.apply(EditOp::Delete { range })?;
            }
            let para = &cx.doc().paragraphs()[at.para];
            let at_end = at.byte == para.text().len();
            let heading = matches!(
                cx.doc().styles().para(para.style()).kind,
                BlockKind::Heading(_)
            );
            cx.apply(EditOp::SplitParagraph { at })?;
            if at_end && heading {
                cx.apply(EditOp::SetParaStyle {
                    paras: at.para + 1..at.para + 2,
                    patch: ParaStylePatch::kind(BlockKind::Body),
                })?;
            }
            Ok(Some(Selection::caret(DocPos::new(at.para + 1, 0))))
        })
    }

    /// Ctrl+Enter: Enter, with the second half starting a new page.
    pub(super) fn insert_page_break(&mut self) -> Effect {
        let Some(range) = self.selection_range() else {
            return Effect::NONE;
        };
        let at = range.start;
        self.run(|cx| {
            if !range.is_empty() {
                cx.apply(EditOp::Delete { range })?;
            }
            cx.apply(EditOp::SplitParagraph { at })?;
            cx.apply(EditOp::SetParaStyle {
                paras: at.para + 1..at.para + 2,
                patch: ParaStylePatch::page_break_before(true),
            })?;
            Ok(Some(Selection::caret(DocPos::new(at.para + 1, 0))))
        })
    }

    /// Deletes the selected text or image.
    pub(super) fn delete_selection_step(&mut self) -> Effect {
        let Some(range) = self.selection_range().filter(|r| !r.is_empty()) else {
            return Effect::NONE;
        };
        self.run(|cx| {
            cx.apply(EditOp::Delete { range })?;
            Ok(Some(Selection::caret(range.start)))
        })
    }

    fn delete_range(&mut self, range: DocRange, now: Duration) -> Effect {
        let after = Selection::caret(range.start);
        let op = EditOp::Delete { range };
        self.edit_one(op, after, (range.start.para, range.start.para), now, false)
    }

    /// Backspace and Ctrl+Backspace.
    pub(super) fn backspace(&mut self, word: bool, now: Duration) -> Effect {
        if !self.selection.is_collapsed() {
            return self.delete_selection_step();
        }
        let pos = self.head();
        if pos.byte == 0 {
            if self.in_list() {
                return self.run(|cx| {
                    cx.apply(EditOp::SetParaStyle {
                        paras: pos.para..pos.para + 1,
                        patch: ParaStylePatch::list(None),
                    })?;
                    Ok(None)
                });
            }
            let para_style = self.doc.paragraphs()[pos.para].style();
            if self.doc.styles().para(para_style).page_break_before {
                return self.run(|cx| {
                    cx.apply(EditOp::SetParaStyle {
                        paras: pos.para..pos.para + 1,
                        patch: ParaStylePatch::page_break_before(false),
                    })?;
                    Ok(None)
                });
            }
            if pos.para == 0 {
                return Effect::NONE;
            }
            return self.merge_with_previous(pos.para);
        }
        let text = self.doc.paragraphs()[pos.para].text();
        let start = if word {
            prev_word(text, pos.byte)
        } else {
            prev_grapheme(text, pos.byte)
        };
        self.delete_range(DocRange::new(DocPos::new(pos.para, start), pos), now)
    }

    fn merge_with_previous(&mut self, para: usize) -> Effect {
        let caret = DocPos::new(para - 1, self.doc.paragraphs()[para - 1].text().len());
        self.run(|cx| {
            cx.apply(EditOp::MergeParagraph { para: para - 1 })?;
            Ok(Some(Selection::caret(caret)))
        })
    }

    /// Delete and Ctrl+Delete.
    pub(super) fn delete_forward(&mut self, word: bool, now: Duration) -> Effect {
        if !self.selection.is_collapsed() {
            return self.delete_selection_step();
        }
        let pos = self.head();
        let text = self.doc.paragraphs()[pos.para].text();
        if pos.byte == text.len() {
            if pos.para + 1 >= self.doc.paragraph_count() {
                return Effect::NONE;
            }
            return self.run(|cx| {
                cx.apply(EditOp::MergeParagraph { para: pos.para })?;
                Ok(None)
            });
        }
        let end = if word {
            next_word(text, pos.byte)
        } else {
            next_grapheme(text, pos.byte)
        };
        self.delete_range(DocRange::new(pos, DocPos::new(pos.para, end)), now)
    }
}
