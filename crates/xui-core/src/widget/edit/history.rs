#![forbid(unsafe_code)]

//! The edit model's undo and redo history.
//!
//! A history is a stack of whole-text [`Snapshot`]s: small enough for a text
//! field, and exact — restoring a snapshot also restores the selection. A run
//! of typed characters coalesces into one step; every other edit (delete,
//! paste, cut, a caret move) starts a new one. A new edit after an undo forks
//! the history and drops the redo stack.

/// The most undo steps kept; the oldest is dropped past this.
pub(super) const MAX_UNDO: usize = 128;

/// A point the edit can be restored to: the text, the caret and the anchor.
#[derive(Clone)]
pub(super) struct Snapshot {
    /// The text at the snapshot.
    pub(super) text: String,
    /// The caret's char index at the snapshot.
    pub(super) caret: usize,
    /// The selection anchor's char index at the snapshot.
    pub(super) anchor: usize,
}

/// The undo/redo stacks and the current typing run.
pub(super) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Whether the last change was a typed character, so the next one joins it.
    typing: bool,
}

impl History {
    /// An empty history.
    pub(super) fn new() -> History {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
        }
    }

    /// Forgets everything, so a programmatic text change cannot be undone.
    pub(super) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.typing = false;
    }

    /// Records `state` as an undo point before an edit. When `coalesce` is set
    /// and the previous change was a typed character, the run continues and
    /// nothing is pushed. Any pending redo is dropped: a new edit forks.
    pub(super) fn record(&mut self, state: Snapshot, coalesce: bool) {
        if coalesce && self.typing {
            return;
        }
        self.redo.clear();
        self.undo.push(state);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
    }

    /// Marks the edit as part of a typing run.
    pub(super) fn mark_typing(&mut self) {
        self.typing = true;
    }

    /// Ends the typing run, so the next typed character starts a new step.
    pub(super) fn break_run(&mut self) {
        self.typing = false;
    }

    /// Pops the previous state, pushing `current` onto the redo stack.
    pub(super) fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        self.typing = false;
        Some(previous)
    }

    /// Pops the next state, pushing `current` back onto the undo stack.
    pub(super) fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        self.typing = false;
        Some(next)
    }
}
