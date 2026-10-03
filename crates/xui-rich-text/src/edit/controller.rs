#![forbid(unsafe_code)]

//! The editor's state and the command dispatcher.

use std::ops::Range;
use std::time::Duration;

use super::clipboard::Clipboard;
use super::command::Command;
use super::nav::LineNav;
use super::tx::{Cx, Dirty};
use crate::model::grapheme::{next_grapheme, prev_grapheme};
use crate::model::{
    CharStylePatch, DocPos, DocRange, Document, EditError, EditOp, History, Selection,
};

/// What a command changed, so the view refreshes only what it must.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// The paragraphs (in the new document) to lay out again, when the
    /// document changed. Paragraph numbers after a split or merge shifted, so
    /// the range then runs to the end of the document. Undo and redo report
    /// the whole document.
    pub dirty: Option<Range<usize>>,
    /// Whether the selection (or the pending style) changed, so the caret and
    /// the toolbar state need refreshing.
    pub selection: bool,
    /// The plain text a copy or cut put on the clipboard.
    pub copied: Option<String>,
}

impl Effect {
    /// Nothing changed.
    pub const NONE: Effect = Effect {
        dirty: None,
        selection: false,
        copied: None,
    };

    /// Whether the document changed.
    pub fn doc_changed(&self) -> bool {
        self.dirty.is_some()
    }

    /// Whether anything changed.
    pub fn is_none(&self) -> bool {
        *self == Effect::NONE
    }
}

/// The document being edited with its history and selection.
#[derive(Debug)]
pub struct EditorState {
    /// The document.
    pub doc: Document,
    /// Undo and redo.
    pub history: History,
    /// The selection.
    pub selection: Selection,
    /// A style change made at a collapsed caret, applied to the next typed
    /// text; cleared when the caret moves.
    pub pending: Option<CharStylePatch>,
    /// The x that Up and Down keep while moving through short lines.
    pub sticky_x: Option<f32>,
    /// An image shown at a trial size by `preview_object_size`.
    pub(super) preview: Option<(crate::model::ObjectId, (xui_core::Dip, xui_core::Dip))>,
}

impl EditorState {
    /// An editor on `doc` with the caret at its start.
    pub fn new(doc: Document) -> EditorState {
        EditorState {
            doc,
            history: History::new(),
            selection: Selection::default(),
            pending: None,
            sticky_x: None,
            preview: None,
        }
    }

    /// Runs `command` as one undoable step. `now` is the event time (any
    /// monotonic origin) used to break typing runs after a pause.
    pub fn exec(
        &mut self,
        command: Command,
        nav: &dyn LineNav,
        now: Duration,
        clipboard: &dyn Clipboard,
    ) -> Effect {
        match command {
            Command::InsertText(text) => self.type_text(&text, now),
            Command::InsertParagraph => self.insert_paragraph(),
            Command::InsertLineBreak => self.type_text("\u{2028}", now),
            Command::InsertPageBreak => self.insert_page_break(),
            Command::Backspace => self.backspace(false, now),
            Command::DeleteWordBack => self.backspace(true, now),
            Command::Delete => self.delete_forward(false, now),
            Command::DeleteWordForward => self.delete_forward(true, now),
            Command::Move { motion, extend } => self.move_caret(motion, extend, nav),
            Command::SelectAll => self.select_all(),
            Command::SelectWord(pos) => self.select_word(pos),
            Command::SelectParagraph(pos) => self.select_paragraph(pos),
            Command::SetCaret { pos, extend } => self.set_caret(pos, extend),
            Command::ToggleBold => self.toggle(super::format::Attr::Bold),
            Command::ToggleItalic => self.toggle(super::format::Attr::Italic),
            Command::ToggleUnderline => self.toggle(super::format::Attr::Underline),
            Command::ToggleStrike => self.toggle(super::format::Attr::Strike),
            Command::SetCharStyle(patch) => self.set_char_style(patch),
            Command::SetParaStyle(patch) => self.set_para_style(patch),
            Command::SetAlign(align) => {
                self.set_para_style(crate::model::ParaStylePatch::align(align))
            }
            Command::SetBlockKind(kind) => {
                self.set_para_style(crate::model::ParaStylePatch::kind(kind))
            }
            Command::ToggleList(kind) => self.toggle_list(kind),
            Command::SetPageSetup(page) if page != *self.doc.page() => self.run(|cx| {
                cx.apply(EditOp::SetPage(page))?;
                Ok(None)
            }),
            Command::SetPageSetup(_) => Effect::NONE,
            Command::Indent => self.shift_indent(true),
            Command::Outdent => self.shift_indent(false),
            Command::InsertImage(image) => self.insert_image(image),
            Command::SetObject { id, object } => self.run(|cx| {
                cx.apply(EditOp::SetObject { id, object })?;
                Ok(None)
            }),
            Command::ResizeObject { id, size } => self.resize_object(id, size),
            Command::MoveObject { id, to } => self.move_object(id, to),
            Command::SetWrap { id, wrap } => self.set_wrap(id, wrap),
            Command::SelectObject(id) => self.select_object(id),
            Command::InsertTable { rows, columns } => self.insert_table(rows, columns),
            Command::InsertRow { below } => self.insert_row(below),
            Command::InsertColumn { right } => self.insert_column(right),
            Command::DeleteRows => self.delete_rows(),
            Command::DeleteColumns => self.delete_columns(),
            Command::DeleteTable => self.delete_table(),
            Command::SetTable { id, table } => self.set_table(id, table),
            Command::NextCell => self.next_cell(true),
            Command::PrevCell => self.next_cell(false),
            Command::Undo => self.undo(true),
            Command::Redo => self.undo(false),
            Command::Copy => self.copy(clipboard),
            Command::Cut => self.cut(clipboard),
            Command::Paste => self.paste(clipboard),
        }
    }

    /// Whether the caret (or selection start) is in a list item, which is
    /// where Tab should indent rather than type a tab.
    pub fn in_list(&self) -> bool {
        self.selection_range()
            .and_then(|r| self.doc.paragraphs().get(r.start.para))
            .is_some_and(|p| self.doc.styles().para(p.style()).list.is_some())
    }

    /// Whether the caret is in a table, which is where Tab moves between
    /// cells.
    pub fn in_table(&self) -> bool {
        self.doc.region(self.head().para).is_some()
    }

    /// Whether the selection lies inside the document on grapheme boundaries.
    pub fn selection_is_valid(&self) -> bool {
        match self.selection {
            Selection::Text { anchor, head } => [anchor, head]
                .iter()
                .all(|&pos| self.snap(pos) == pos && self.doc.paragraphs().len() > pos.para),
            Selection::Object(id) => self.doc.object_pos(id).is_some(),
        }
    }

    /// The range the selection covers (an image covers its anchor).
    pub(super) fn selection_range(&self) -> Option<DocRange> {
        self.doc.selection_range(&self.selection)
    }

    /// The caret end of the selection, or the start of a selected image.
    pub(super) fn head(&self) -> DocPos {
        self.selection
            .head()
            .or_else(|| self.selection_range().map(|r| r.start))
            .unwrap_or_default()
    }

    /// `pos` moved onto the nearest valid grapheme boundary at or before it.
    pub(super) fn snap(&self, pos: DocPos) -> DocPos {
        let last = self.doc.paragraph_count() - 1;
        let para = pos.para.min(last);
        let text = self.doc.paragraphs()[para].text();
        let byte = if pos.para > last || pos.byte >= text.len() {
            text.len()
        } else {
            prev_grapheme(text, next_grapheme(text, pos.byte))
        };
        DocPos::new(para, byte)
    }

    /// `sel` with each text end moved forward onto a grapheme boundary: text
    /// typed before a combining mark leaves the caret after the whole cluster.
    fn snap_selection(&self, sel: Selection) -> Selection {
        let up = |pos: DocPos| {
            let down = self.snap(pos);
            if down == pos {
                return pos;
            }
            let text = self.doc.paragraphs()[down.para].text();
            DocPos::new(down.para, next_grapheme(text, down.byte))
        };
        match sel {
            Selection::Text { anchor, head } => Selection::text(up(anchor), up(head)),
            Selection::Object(_) => sel,
        }
    }

    /// Runs a transaction. The body returns the new selection, or `None` to
    /// keep the current one.
    pub(super) fn run<F>(&mut self, body: F) -> Effect
    where
        F: FnOnce(&mut Cx<'_, '_>) -> Result<Option<Selection>, EditError>,
    {
        let before_len = self.doc.paragraph_count();
        let before = self.selection;
        let mut dirty = Dirty::default();
        let result = self.history.transact(&mut self.doc, before, |tx| {
            let mut cx = Cx::new(tx, &mut dirty);
            Ok(body(&mut cx)?.unwrap_or(before))
        });
        match result {
            Ok(after) => self.finish(after, before_len, dirty),
            Err(_) => Effect::NONE,
        }
    }

    /// Records the outcome of an edit made outside `run`.
    pub(super) fn finish(&mut self, after: Selection, before_len: usize, dirty: Dirty) -> Effect {
        let changed = after != self.selection || self.pending.is_some();
        self.selection = self.snap_selection(after);
        self.pending = None;
        self.sticky_x = None;
        let len = self.doc.paragraph_count();
        let dirty = (!dirty.is_clean()).then(|| {
            let end = if len != before_len {
                len
            } else {
                (dirty.last + 1).min(len)
            };
            dirty.first.min(len.saturating_sub(1))..end
        });
        Effect {
            selection: changed,
            dirty,
            copied: None,
        }
    }

    fn undo(&mut self, undo: bool) -> Effect {
        let sel = if undo {
            self.history.undo(&mut self.doc)
        } else {
            self.history.redo(&mut self.doc)
        };
        let Some(sel) = sel else {
            return Effect::NONE;
        };
        self.selection = self.snap_selection(sel);
        self.pending = None;
        self.sticky_x = None;
        Effect {
            dirty: Some(0..self.doc.paragraph_count()),
            selection: true,
            copied: None,
        }
    }

    fn select_object(&mut self, id: crate::model::ObjectId) -> Effect {
        if self.doc.object_pos(id).is_none() {
            return Effect::NONE;
        }
        self.set_selection(Selection::Object(id))
    }

    /// Replaces the selection without editing; true effect if it differs.
    pub(super) fn set_selection(&mut self, sel: Selection) -> Effect {
        let changed = sel != self.selection;
        if changed {
            self.pending = None;
        }
        self.selection = sel;
        Effect {
            selection: changed,
            ..Effect::NONE
        }
    }
}
