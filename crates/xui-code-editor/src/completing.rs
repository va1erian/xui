#![forbid(unsafe_code)]

//! How completion hooks into the editor's input: when the popup opens, which
//! keys and clicks it takes while open, and when it closes.
//!
//! The functions work on the [`EditorState`] alone (the mouse ones also take the
//! [`Ui`] to measure the grid), so the event mapper only has to call them at the
//! right moments and the rules stay unit-testable.

use xui_core::app::Ui;
use xui_core::backend::WidgetId;
use xui_core::message::Key;

use crate::completion::{MAX_ROWS, Popup};
use crate::events::{Outcome, viewport};
use crate::popup::Layout;
use crate::state::EditorState;
use crate::text::is_word_char;

/// The shortest word, in chars, that opens the popup on its own.
pub(crate) const AUTO_TRIGGER_CHARS: usize = 2;
/// Rows a wheel notch scrolls the popup's list.
const WHEEL_ROWS: i32 = 3;
/// The wheel delta of one notch.
const WHEEL_NOTCH: i32 = 120;

/// Asks the completer for candidates at the caret and opens the popup over
/// them, replacing any popup already open. Returns whether a popup is open.
///
/// Nothing opens without a completer, with a selection, when the completer has
/// nothing to offer, or when no candidate matches the text before the caret.
pub(crate) fn trigger(state: &mut EditorState) -> bool {
    state.completion = None;
    let Some(completer) = state.completer.clone() else {
        return false;
    };
    if state.view.selection().is_some() {
        return false;
    }
    let caret = state.view.caret;
    let text = state.buffer.text();
    let Some(mut completion) = completer.complete(&text, caret) else {
        return false;
    };
    completion.start = completion.start.min(caret);
    let prefix = state.buffer.slice(completion.start..caret);
    state.completion = Popup::open(completion, &prefix);
    state.completion.is_some()
}

/// Closes the popup. Returns whether one was open.
pub(crate) fn close(state: &mut EditorState) -> bool {
    state.completion.take().is_some()
}

/// The char offset where the identifier ending at `caret` starts.
fn word_start(state: &EditorState, caret: usize) -> usize {
    let mut start = caret;
    while start > 0 && state.buffer.char_at(start - 1).is_some_and(is_word_char) {
        start -= 1;
    }
    start
}

/// Keeps the popup in step with the caret and the text before it: it filters by
/// the text from the word's start to the caret, and closes when the caret left
/// the word, a selection appeared, or nothing matches any more.
pub(crate) fn refresh(state: &mut EditorState) {
    let Some(popup) = state.completion.as_mut() else {
        return;
    };
    let caret = state.view.caret;
    let start = popup.start();
    let prefix = (state.view.selection().is_none() && start <= caret)
        .then(|| state.buffer.slice(start..caret))
        .filter(|prefix| prefix.chars().all(is_word_char));
    let alive = prefix.is_some_and(|prefix| popup.refilter(&prefix));
    if !alive {
        state.completion = None;
    }
}

/// Reacts to a typed character that has just been inserted.
///
/// An identifier char filters an open popup, or opens one once the word has
/// [`AUTO_TRIGGER_CHARS`] chars. A `.` or `::` opens one at once. Any other
/// character closes the popup.
pub(crate) fn after_char(state: &mut EditorState, character: char) {
    if state.completer.is_none() {
        return;
    }
    let caret = state.view.caret;
    let member = character == '.'
        || (character == ':' && caret >= 2 && state.buffer.char_at(caret - 2) == Some(':'));
    if member {
        trigger(state);
    } else if is_word_char(character) {
        if state.completion.is_some() {
            refresh(state);
        } else if caret - word_start(state, caret) >= AUTO_TRIGGER_CHARS {
            trigger(state);
        }
    } else {
        close(state);
    }
}

/// Reacts to a key the editor has just handled in the ordinary way.
///
/// Moving the caret re-checks it against the word; an edit other than
/// Backspace (which narrows the word) closes the popup.
pub(crate) fn after_key(state: &mut EditorState, key: Key, revision_before: u64) {
    if state.completion.is_none() {
        return;
    }
    if state.buffer.revision() != revision_before && key != Key::BACK {
        close(state);
    } else {
        refresh(state);
    }
}

/// Takes a key before the editor's ordinary handling, when completion owns it.
///
/// Ctrl+Space opens the popup. While it is open, Up and Down move the selection
/// (wrapping at the ends), PageUp and PageDown move by a page and stop at the
/// ends, Enter and Tab accept the selection, and Escape closes it. Shift+Tab
/// closes it and then outdents as usual. Returns `None` for any other key.
pub(crate) fn intercept_key(
    state: &mut EditorState,
    key: Key,
    ctrl: bool,
    shift: bool,
) -> Option<Outcome> {
    if key == Key::SPACE && ctrl {
        state.completer.as_ref()?;
        trigger(state);
        return Some(Outcome::default());
    }
    let popup = state.completion.as_mut()?;
    match key {
        Key::UP => popup.move_by(-1, true),
        Key::DOWN => popup.move_by(1, true),
        Key::PAGE_UP => popup.move_by(-(MAX_ROWS as isize), false),
        Key::PAGE_DOWN => popup.move_by(MAX_ROWS as isize, false),
        Key::RETURN => return Some(accept(state)),
        Key::TAB if !shift => return Some(accept(state)),
        Key::TAB => {
            close(state);
            return None;
        }
        Key::ESCAPE => {
            close(state);
        }
        _ => return None,
    }
    Some(Outcome::default())
}

/// Replaces the word being completed (its start to the caret) with the
/// selected candidate's text as one undo step, and closes the popup.
pub(crate) fn accept(state: &mut EditorState) -> Outcome {
    let Some(popup) = state.completion.take() else {
        return Outcome::default();
    };
    let Some(insert) = popup.current().map(|item| item.insert.clone()) else {
        return Outcome::default();
    };
    let caret = state.view.caret;
    let start = popup.start().min(caret);
    let revision = state.buffer.revision();
    state.buffer.break_coalescing();
    state.buffer.replace(start..caret, &insert, false);
    state.view.caret = start + insert.chars().count();
    state.view.anchor = state.view.caret;
    state.view.goal_col = None;
    Outcome {
        changed: state.buffer.revision() != revision,
    }
}

/// Takes a left press on the popup: a click on a candidate accepts it, a click
/// elsewhere on the popup does nothing, and a click outside closes the popup
/// and goes on to the editor. Returns `None` when the editor should handle the
/// press.
pub(crate) fn mouse_down<M: 'static>(
    state: &mut EditorState,
    ui: &Ui<M>,
    id: WidgetId,
    x: i32,
    y: i32,
) -> Option<Outcome> {
    let layout = current_layout(state, ui, id);
    let Some(layout) = layout else {
        close(state);
        return None;
    };
    if let Some(row) = layout.row_at(x, y) {
        state.completion.as_mut()?.select(row);
        Some(accept(state))
    } else if layout.contains(x, y) {
        Some(Outcome::default())
    } else {
        close(state);
        None
    }
}

/// Takes a wheel event over the popup, scrolling its list. Returns whether it
/// was over the popup (and so is not the editor's).
pub(crate) fn wheel<M: 'static>(
    state: &mut EditorState,
    ui: &Ui<M>,
    id: WidgetId,
    x: i32,
    y: i32,
    delta: i16,
) -> bool {
    let Some(layout) = current_layout(state, ui, id) else {
        return false;
    };
    if !layout.contains(x, y) {
        return false;
    }
    // A positive delta is away from the user: up the list.
    let notches = (i32::from(delta).abs() * WHEEL_ROWS / WHEEL_NOTCH).max(1);
    let rows = -i32::from(delta).signum() * notches;
    if let Some(popup) = state.completion.as_mut() {
        popup.scroll_by(rows as isize);
    }
    true
}

/// Where the open popup sits now, or `None` when none is open or its caret line
/// is off screen.
fn current_layout<M: 'static>(state: &EditorState, ui: &Ui<M>, id: WidgetId) -> Option<Layout> {
    let popup = state.completion.as_ref()?;
    Layout::compute(state, popup, &viewport(ui, id, state), ui.dpi())
}
