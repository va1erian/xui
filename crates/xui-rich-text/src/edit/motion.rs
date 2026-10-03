#![forbid(unsafe_code)]

//! Caret movement and selection changes.

use super::command::Motion;
use super::controller::{EditorState, Effect};
use super::nav::LineNav;
use crate::model::grapheme::{next_pos, next_word, prev_pos, prev_word, word_range_at};
use crate::model::{DocPos, Selection};

impl EditorState {
    /// The selection as an anchor and a head (an image spans its character).
    fn ends(&self) -> (DocPos, DocPos) {
        match self.selection {
            Selection::Text { anchor, head } => (anchor, head),
            Selection::Object(_) => self
                .selection_range()
                .map_or_else(Default::default, |r| (r.start, r.end)),
        }
    }

    /// Moves or extends the selection.
    pub(super) fn move_caret(&mut self, motion: Motion, extend: bool, nav: &dyn LineNav) -> Effect {
        let (anchor, head) = self.ends();
        let (start, end) = (anchor.min(head), anchor.max(head));
        let collapsed = anchor == head;
        let doc = &self.doc;
        let mut sticky = None;
        let target = match motion {
            Motion::Left if !extend && !collapsed => start,
            Motion::Right if !extend && !collapsed => end,
            Motion::Left => prev_pos(doc, head).unwrap_or(head),
            Motion::Right => next_pos(doc, head).unwrap_or(head),
            Motion::WordLeft => {
                if head.byte == 0 {
                    prev_pos(doc, head).unwrap_or(head)
                } else {
                    let text = doc.paragraphs()[head.para].text();
                    DocPos::new(head.para, prev_word(text, head.byte))
                }
            }
            Motion::WordRight => {
                let text = doc.paragraphs()[head.para].text();
                if head.byte == text.len() {
                    next_pos(doc, head).unwrap_or(head)
                } else {
                    DocPos::new(head.para, next_word(text, head.byte))
                }
            }
            Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown => {
                let page = nav.page_lines().max(1);
                let lines = match motion {
                    Motion::Up => -1,
                    Motion::Down => 1,
                    Motion::PageUp => -page,
                    _ => page,
                };
                let (pos, x) = nav.vertical(head, self.sticky_x, lines);
                sticky = Some(x);
                pos
            }
            Motion::LineStart => nav.line_start(head),
            Motion::LineEnd => nav.line_end(head),
            Motion::DocStart => DocPos::default(),
            Motion::DocEnd => self.doc_end(),
        };
        let target = self.snap(target);
        self.sticky_x = sticky;
        self.set_selection(if extend {
            Selection::text(anchor, target)
        } else {
            Selection::caret(target)
        })
    }

    fn doc_end(&self) -> DocPos {
        let last = self.doc.paragraph_count() - 1;
        DocPos::new(last, self.doc.paragraphs()[last].text().len())
    }

    /// Ctrl+A.
    pub(super) fn select_all(&mut self) -> Effect {
        self.sticky_x = None;
        self.set_selection(Selection::text(DocPos::default(), self.doc_end()))
    }

    /// Double-click.
    pub(super) fn select_word(&mut self, pos: DocPos) -> Effect {
        let pos = self.snap(pos);
        let text = self.doc.paragraphs()[pos.para].text();
        let word = word_range_at(text, pos.byte);
        self.sticky_x = None;
        self.set_selection(Selection::text(
            DocPos::new(pos.para, word.start),
            DocPos::new(pos.para, word.end),
        ))
    }

    /// Triple-click.
    pub(super) fn select_paragraph(&mut self, pos: DocPos) -> Effect {
        let pos = self.snap(pos);
        let len = self.doc.paragraphs()[pos.para].text().len();
        self.sticky_x = None;
        self.set_selection(Selection::text(
            DocPos::new(pos.para, 0),
            DocPos::new(pos.para, len),
        ))
    }

    /// A click, or a Shift+click or drag when `extend`.
    pub(super) fn set_caret(&mut self, pos: DocPos, extend: bool) -> Effect {
        let pos = self.snap(pos);
        let (anchor, _) = self.ends();
        self.sticky_x = None;
        self.set_selection(if extend {
            Selection::text(anchor, pos)
        } else {
            Selection::caret(pos)
        })
    }
}
