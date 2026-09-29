#![forbid(unsafe_code)]

//! The pure single-line edit model behind [`Edit`](super::Edit): the text, the
//! caret and the selection anchor as char indices, word boundaries, and a
//! grouped undo/redo history.
//!
//! Nothing here touches a backend, a canvas or a message: every rule is a pure
//! function of the current state, so it is unit-tested directly. The widget
//! maps events to these methods and paints the result.

use super::history::{History, Snapshot};
use super::word::{word_left, word_right};

/// A single-line edit's text, caret and selection, with undo and redo.
///
/// The caret and selection are **char indices** into the text, never byte
/// offsets, so multi-byte characters move one step at a time. The selection is
/// the range between the anchor and the caret; a caret equal to the anchor is
/// an empty selection. The caret is the moving end, so a Shift+arrow extends
/// from the anchor.
pub(super) struct EditModel {
    text: String,
    caret: usize,
    anchor: usize,
    history: History,
}

impl EditModel {
    /// A model over `text` with the caret at the end.
    pub(super) fn new(text: &str) -> EditModel {
        let end = text.chars().count();
        EditModel {
            text: text.to_string(),
            caret: end,
            anchor: end,
            history: History::new(),
        }
    }

    /// The current text.
    pub(super) fn text(&self) -> &str {
        &self.text
    }

    /// The caret's char index.
    pub(super) fn caret(&self) -> usize {
        self.caret
    }

    /// The selection as `(start, end)` char indices, `start <= end`.
    pub(super) fn selection(&self) -> (usize, usize) {
        if self.caret < self.anchor {
            (self.caret, self.anchor)
        } else {
            (self.anchor, self.caret)
        }
    }

    /// Whether anything is selected.
    pub(super) fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    /// The selected text, empty when nothing is selected.
    pub(super) fn selected_text(&self) -> String {
        let (start, end) = self.selection();
        self.text.chars().skip(start).take(end - start).collect()
    }

    /// Replaces the whole text, puts the caret at the end and clears the
    /// history: a programmatic [`set_text`](EditModel::set_text) is not a user
    /// edit and must not be undone.
    pub(super) fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.caret = self.text.chars().count();
        self.anchor = self.caret;
        self.history.clear();
    }

    /// The number of chars in the text.
    fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// The text's characters.
    fn chars(&self) -> Vec<char> {
        self.text.chars().collect()
    }

    /// Replaces the text and moves both ends to `caret`.
    fn apply(&mut self, chars: Vec<char>, caret: usize) {
        self.text = chars.into_iter().collect();
        self.caret = caret;
        self.anchor = caret;
    }

    /// Pushes the current state so the next change can be undone. A typed
    /// character that follows another (`coalesce`) joins the run instead of
    /// starting a new step.
    fn begin(&mut self, coalesce: bool) {
        let snapshot = self.snapshot();
        self.history.record(snapshot, coalesce);
    }

    /// The current state as a snapshot.
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    /// Restores a snapshot.
    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.caret = snapshot.caret;
        self.anchor = snapshot.anchor;
    }

    /// Removes the selected range if any, leaving the caret at its start.
    fn delete_selection(&mut self) {
        let (start, end) = self.selection();
        let mut chars = self.chars();
        chars.drain(start..end);
        self.apply(chars, start);
    }

    /// Inserts `text` at the caret, replacing the selection. Not an undo step
    /// on its own; the callers record one.
    fn replace_selection(&mut self, text: &str) {
        let (start, end) = self.selection();
        let mut chars = self.chars();
        chars.splice(start..end, text.chars());
        self.apply(chars, start + text.chars().count());
    }

    /// Inserts a typed `character`, replacing the selection. Consecutive typed
    /// characters group into one undo step.
    pub(super) fn insert_char(&mut self, character: char) {
        self.begin(true);
        let mut buffer = [0u8; 4];
        self.replace_selection(character.encode_utf8(&mut buffer));
        self.history.mark_typing();
    }

    /// Inserts a run of text (a paste), replacing the selection, as one undo
    /// step. Control characters are dropped: a single-line field cannot hold
    /// them. Returns whether it changed anything.
    pub(super) fn insert_text(&mut self, text: &str) -> bool {
        let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
        if cleaned.is_empty() {
            return false;
        }
        self.begin(false);
        self.replace_selection(&cleaned);
        self.history.break_run();
        true
    }

    /// Removes the selection and returns its text, as a cut does, or `None`
    /// when nothing is selected.
    pub(super) fn cut(&mut self) -> Option<String> {
        if !self.has_selection() {
            return None;
        }
        let text = self.selected_text();
        self.begin(false);
        self.delete_selection();
        self.history.break_run();
        Some(text)
    }

    /// Deletes the character before the caret (or the selection). Returns
    /// whether it changed anything.
    pub(super) fn backspace(&mut self) -> bool {
        if self.has_selection() {
            self.begin(false);
            self.delete_selection();
            self.history.break_run();
            return true;
        }
        if self.caret == 0 {
            return false;
        }
        self.begin(false);
        let mut chars = self.chars();
        chars.remove(self.caret - 1);
        self.apply(chars, self.caret - 1);
        self.history.break_run();
        true
    }

    /// Deletes the character after the caret (or the selection). Returns
    /// whether it changed anything.
    pub(super) fn delete(&mut self) -> bool {
        if self.has_selection() {
            self.begin(false);
            self.delete_selection();
            self.history.break_run();
            return true;
        }
        if self.caret >= self.len() {
            return false;
        }
        self.begin(false);
        let mut chars = self.chars();
        chars.remove(self.caret);
        self.apply(chars, self.caret);
        self.history.break_run();
        true
    }

    /// Deletes from the previous word boundary to the caret (or the
    /// selection). Returns whether it changed anything.
    pub(super) fn delete_word_back(&mut self) -> bool {
        if self.has_selection() {
            self.begin(false);
            self.delete_selection();
            self.history.break_run();
            return true;
        }
        let start = word_left(&self.chars(), self.caret);
        if start == self.caret {
            return false;
        }
        self.begin(false);
        let mut chars = self.chars();
        chars.drain(start..self.caret);
        self.apply(chars, start);
        self.history.break_run();
        true
    }

    /// Deletes from the caret to the next word boundary (or the selection).
    /// Returns whether it changed anything.
    pub(super) fn delete_word_forward(&mut self) -> bool {
        if self.has_selection() {
            self.begin(false);
            self.delete_selection();
            self.history.break_run();
            return true;
        }
        let end = word_right(&self.chars(), self.caret);
        if end == self.caret {
            return false;
        }
        self.begin(false);
        let mut chars = self.chars();
        chars.drain(self.caret..end);
        self.apply(chars, self.caret);
        self.history.break_run();
        true
    }

    /// Moves the caret to `pos` (clamped to the text). `extend` keeps the
    /// anchor, so a Shift+move grows the selection; otherwise both ends move.
    pub(super) fn move_to(&mut self, pos: usize, extend: bool) {
        self.caret = pos.min(self.len());
        if !extend {
            self.anchor = self.caret;
        }
        self.history.break_run();
    }

    /// Moves one char left; a plain move first collapses a selection to its
    /// start.
    pub(super) fn move_left(&mut self, extend: bool) {
        let target = if self.has_selection() && !extend {
            self.selection().0
        } else {
            self.caret.saturating_sub(1)
        };
        self.move_to(target, extend);
    }

    /// Moves one char right; a plain move first collapses a selection to its
    /// end.
    pub(super) fn move_right(&mut self, extend: bool) {
        let target = if self.has_selection() && !extend {
            self.selection().1
        } else {
            self.caret + 1
        };
        self.move_to(target, extend);
    }

    /// Moves to the start of the line. A plain move collapses a selection.
    pub(super) fn move_home(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.move_to(self.selection().0, false);
        } else {
            self.move_to(0, extend);
        }
    }

    /// Moves to the end of the line. A plain move collapses a selection.
    pub(super) fn move_end(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.move_to(self.selection().1, false);
        } else {
            self.move_to(self.len(), extend);
        }
    }

    /// Moves one word left; a plain move first collapses a selection to its
    /// start.
    pub(super) fn move_word_left(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.move_to(self.selection().0, false);
            return;
        }
        let target = word_left(&self.chars(), self.caret);
        self.move_to(target, extend);
    }

    /// Moves one word right; a plain move first collapses a selection to its
    /// end.
    pub(super) fn move_word_right(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.move_to(self.selection().1, false);
            return;
        }
        let target = word_right(&self.chars(), self.caret);
        self.move_to(target, extend);
    }

    /// Selects the whole text.
    pub(super) fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.len();
        self.history.break_run();
    }

    /// Selects the maximal run of same-class characters around `at`, as a
    /// double-click does: a word, or a whitespace run between words.
    pub(super) fn select_word_at(&mut self, at: usize) {
        let chars = self.chars();
        let len = chars.len();
        if len == 0 {
            self.caret = 0;
            self.anchor = 0;
            return;
        }
        let at = at.min(len);
        // A click past the last character selects the last run.
        let anchor = if at == len { len - 1 } else { at };
        let whitespace = chars[anchor].is_whitespace();
        let mut start = anchor;
        while start > 0 && chars[start - 1].is_whitespace() == whitespace {
            start -= 1;
        }
        let mut end = anchor + 1;
        while end < len && chars[end].is_whitespace() == whitespace {
            end += 1;
        }
        self.anchor = start;
        self.caret = end;
        self.history.break_run();
    }

    /// Undoes the last step. Returns whether anything changed.
    pub(super) fn undo(&mut self) -> bool {
        let current = self.snapshot();
        match self.history.undo(current) {
            Some(previous) => {
                self.restore(previous);
                true
            }
            None => false,
        }
    }

    /// Redoes the last undone step. Returns whether anything changed.
    pub(super) fn redo(&mut self) -> bool {
        let current = self.snapshot();
        match self.history.redo(current) {
            Some(next) => {
                self.restore(next);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests;
