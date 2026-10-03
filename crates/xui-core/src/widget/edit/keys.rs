#![forbid(unsafe_code)]

//! The Windows CUA keyboard map for [`Edit`](super::Edit): which key and
//! modifier combination runs which [`EditModel`] operation, and how clipboard
//! verbs reach the backend.
//!
//! Split from the widget so the map is a pure function of the key, the
//! modifiers and the clipboard text: a unit test feeds it a key and checks the
//! model, with no backend.
//!
//! A masked (password) field keeps its text in: copy and cut do nothing, and
//! word keys treat the whole text as one word, since stopping at a space would
//! reveal where the password's words break.

use super::model::EditModel;
use crate::message::{Key, Modifiers};

/// What a key did, so the widget knows whether to repaint and whether to raise
/// a change message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyResult {
    /// The key is not an edit key; the widget leaves it alone.
    Ignored,
    /// The key moved the caret or changed the selection: repaint, no message.
    Redraw,
    /// The key changed the text: repaint and raise the change.
    Changed,
}

/// The text clipboard a key operation reads and writes.
pub(super) trait Clipboard {
    /// The clipboard's current text, or `None` when it holds none.
    fn text(&self) -> Option<String>;
    /// Replaces the clipboard's text.
    fn set(&self, text: &str);
}

/// Applies `key` with `modifiers` to `model`. A `masked` field refuses copy
/// and cut and moves by whole text instead of by word.
pub(super) fn apply(
    model: &mut EditModel,
    key: Key,
    modifiers: Modifiers,
    clipboard: &dyn Clipboard,
    masked: bool,
) -> KeyResult {
    let (ctrl, shift) = (modifiers.ctrl, modifiers.shift);
    if masked && let Some(result) = masked_key(model, key, ctrl, shift) {
        return result;
    }
    match key {
        Key::A if ctrl => {
            model.select_all();
            KeyResult::Redraw
        }
        Key::C if ctrl => {
            if model.has_selection() {
                clipboard.set(&model.selected_text());
            }
            KeyResult::Redraw
        }
        Key::X if ctrl => match model.cut() {
            Some(text) => {
                clipboard.set(&text);
                KeyResult::Changed
            }
            None => KeyResult::Redraw,
        },
        Key::V if ctrl => paste(model, clipboard),
        Key::Z if ctrl && shift => redo(model),
        Key::Z if ctrl => undo(model),
        Key::Y if ctrl => redo(model),
        // Shift+Delete cuts and Shift+Insert pastes, the classic CUA pair.
        Key::DELETE if shift && !ctrl => match model.cut() {
            Some(text) => {
                clipboard.set(&text);
                KeyResult::Changed
            }
            None => KeyResult::Redraw,
        },
        Key::INSERT if shift && !ctrl => paste(model, clipboard),
        Key::BACK if ctrl => changed(model.delete_word_back()),
        Key::DELETE if ctrl => changed(model.delete_word_forward()),
        Key::LEFT if ctrl => {
            model.move_word_left(shift);
            KeyResult::Redraw
        }
        Key::RIGHT if ctrl => {
            model.move_word_right(shift);
            KeyResult::Redraw
        }
        Key::LEFT => {
            model.move_left(shift);
            KeyResult::Redraw
        }
        Key::RIGHT => {
            model.move_right(shift);
            KeyResult::Redraw
        }
        Key::HOME => {
            model.move_home(shift);
            KeyResult::Redraw
        }
        Key::END => {
            model.move_end(shift);
            KeyResult::Redraw
        }
        Key::BACK => changed(model.backspace()),
        Key::DELETE => changed(model.delete()),
        _ => KeyResult::Ignored,
    }
}

/// The keys a masked field handles differently, or `None` for the rest.
fn masked_key(model: &mut EditModel, key: Key, ctrl: bool, shift: bool) -> Option<KeyResult> {
    let result = match key {
        // Copy and cut would put the secret on the clipboard.
        Key::C | Key::X if ctrl => KeyResult::Redraw,
        Key::DELETE if shift && !ctrl => KeyResult::Redraw,
        Key::LEFT if ctrl => {
            model.move_home(shift);
            KeyResult::Redraw
        }
        Key::RIGHT if ctrl => {
            model.move_end(shift);
            KeyResult::Redraw
        }
        Key::BACK if ctrl => delete_through(model, EditModel::move_home, EditModel::backspace),
        Key::DELETE if ctrl => delete_through(model, EditModel::move_end, EditModel::delete),
        _ => return None,
    };
    Some(result)
}

/// Deletes the selection, or else from the caret to the end `extend` reaches:
/// a masked field's Ctrl+Backspace (to the start) and Ctrl+Delete (to the end).
fn delete_through(
    model: &mut EditModel,
    extend: fn(&mut EditModel, bool),
    remove: fn(&mut EditModel) -> bool,
) -> KeyResult {
    if !model.has_selection() {
        extend(model, true);
    }
    changed(remove(model))
}

/// Pastes the clipboard over the selection.
fn paste(model: &mut EditModel, clipboard: &dyn Clipboard) -> KeyResult {
    let text = clipboard.text().unwrap_or_default();
    if model.insert_text(&text) {
        KeyResult::Changed
    } else {
        KeyResult::Redraw
    }
}

/// Undoes one step.
fn undo(model: &mut EditModel) -> KeyResult {
    if model.undo() {
        KeyResult::Changed
    } else {
        KeyResult::Redraw
    }
}

/// Redoes one step.
fn redo(model: &mut EditModel) -> KeyResult {
    if model.redo() {
        KeyResult::Changed
    } else {
        KeyResult::Redraw
    }
}

/// Maps a model operation's "changed" flag to a [`KeyResult`].
fn changed(changed: bool) -> KeyResult {
    if changed {
        KeyResult::Changed
    } else {
        KeyResult::Redraw
    }
}
