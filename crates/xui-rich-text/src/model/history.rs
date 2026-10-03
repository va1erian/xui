#![forbid(unsafe_code)]

//! Undo and redo: a stack of transactions, with consecutive typing or
//! backspacing folded into one step.
//!
//! Each entry stores the inverse ops of one user action, so undoing applies
//! them newest first and collects the inverses of those as the redo entry. The
//! selection before and after is stored with it, so undo puts the caret back.
//!
//! Time is passed in (as a [`Duration`] since any fixed origin) so coalescing
//! is deterministic in tests.

use std::time::Duration;

use super::grapheme::next_grapheme;
use super::ops::{EditError, EditOp};
use super::selection::Selection;
use super::{DocPos, Document};

/// The longest pause after which typing starts a new undo step.
pub const COALESCE_GAP: Duration = Duration::from_millis(1500);

/// The most undo steps kept.
const MAX_ENTRIES: usize = 1000;

/// The selection around an edit and when it happened.
#[derive(Clone, Copy, Debug)]
pub struct EditContext {
    /// The selection before the edit.
    pub before: Selection,
    /// The selection after it.
    pub after: Selection,
    /// When the edit happened.
    pub now: Duration,
}

impl EditContext {
    /// A context at time zero.
    pub fn new(before: Selection, after: Selection) -> EditContext {
        EditContext {
            before,
            after,
            now: Duration::ZERO,
        }
    }

    /// The same context at time `now`.
    pub fn at(mut self, now: Duration) -> EditContext {
        self.now = now;
        self
    }
}

/// What kind of run an entry can still grow into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Run {
    Typing,
    Backspace,
    ForwardDelete,
}

#[derive(Clone, Copy, Debug)]
struct Open {
    run: Run,
    para: usize,
    time: Duration,
    ended_in_blank: bool,
}

#[derive(Clone, Debug)]
struct Entry {
    inverses: Vec<EditOp>,
    before: Selection,
    after: Selection,
    open: Option<Open>,
}

/// An undo/redo stack.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

/// The ops of one transaction in progress; see [`History::transact`].
pub struct Transaction<'a> {
    doc: &'a mut Document,
    inverses: Vec<EditOp>,
}

impl Transaction<'_> {
    /// The document.
    pub fn doc(&self) -> &Document {
        self.doc
    }

    /// Applies `op` as part of the transaction.
    pub fn apply(&mut self, op: EditOp) -> Result<(), EditError> {
        self.inverses.push(self.doc.apply(op)?);
        Ok(())
    }

    /// Inserts `fragment` at `at` as part of the transaction and returns the
    /// position after it (see [`Document::insert_fragment`]).
    pub fn insert_fragment(
        &mut self,
        at: DocPos,
        fragment: &super::Fragment,
    ) -> Result<DocPos, EditError> {
        let (inverse, end) = self.doc.insert_fragment(at, fragment)?;
        self.inverses.push(inverse);
        Ok(end)
    }

    /// Replaces the selection with `text` (see [`Document::insert_text`]) and
    /// returns the caret after it.
    pub fn insert_text(&mut self, sel: &Selection, text: &str) -> Result<DocPos, EditError> {
        let (inverses, caret) = self.doc.insert_text(sel, text)?;
        self.inverses.extend(inverses);
        Ok(caret)
    }
}

impl History {
    /// An empty history.
    pub fn new() -> History {
        History::default()
    }

    /// Whether there is something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forgets everything (after loading a new document).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Ends the current typing run, so the next edit starts a new step.
    pub fn seal(&mut self) {
        if let Some(top) = self.undo.last_mut() {
            top.open = None;
        }
    }

    /// Applies `op` as one undo step, folding it into the previous step when
    /// both are part of one run of typing or backspacing: the same paragraph,
    /// no caret jump, no longer than [`COALESCE_GAP`] apart, and (for typing) no
    /// blank typed right after a word.
    pub fn edit(
        &mut self,
        doc: &mut Document,
        op: EditOp,
        ctx: EditContext,
    ) -> Result<(), EditError> {
        let next = classify(doc, &op, &ctx);
        let inverse = doc.apply(op)?;
        self.redo.clear();
        if let (Some(next), Some(top)) = (next, self.undo.last_mut())
            && top.after == ctx.before
            && top.open.is_some_and(|open| open.continues(&next))
            && top
                .inverses
                .last_mut()
                .is_some_and(|last| last.merge_inverse(&inverse))
        {
            top.after = ctx.after;
            top.open = Some(next);
            return Ok(());
        }
        self.push(Entry {
            inverses: vec![inverse],
            before: ctx.before,
            after: ctx.after,
            open: next,
        });
        Ok(())
    }

    /// Runs `f` as one undo step. If it fails, its ops are rolled back and
    /// nothing is recorded. `f` returns the selection after the step.
    pub fn transact<F>(
        &mut self,
        doc: &mut Document,
        before: Selection,
        f: F,
    ) -> Result<Selection, EditError>
    where
        F: FnOnce(&mut Transaction<'_>) -> Result<Selection, EditError>,
    {
        let mut tx = Transaction {
            doc,
            inverses: Vec::new(),
        };
        match f(&mut tx) {
            Ok(after) => {
                if !tx.inverses.is_empty() {
                    self.seal();
                    self.redo.clear();
                    self.push(Entry {
                        inverses: tx.inverses,
                        before,
                        after,
                        open: None,
                    });
                }
                Ok(after)
            }
            Err(error) => {
                for op in tx.inverses.into_iter().rev() {
                    let _ = tx.doc.apply(op);
                }
                Err(error)
            }
        }
    }

    /// Undoes the last step, returning the selection from before it.
    pub fn undo(&mut self, doc: &mut Document) -> Option<Selection> {
        let entry = self.undo.pop()?;
        let flipped = Self::run(doc, &entry)?;
        self.redo.push(flipped);
        self.seal();
        Some(entry.before)
    }

    /// Redoes the last undone step, returning the selection from after it.
    pub fn redo(&mut self, doc: &mut Document) -> Option<Selection> {
        let entry = self.redo.pop()?;
        let flipped = Self::run(doc, &entry)?;
        self.undo.push(flipped);
        Some(entry.after)
    }

    /// Applies an entry's ops and returns the entry that reverses it.
    fn run(doc: &mut Document, entry: &Entry) -> Option<Entry> {
        let mut inverses = Vec::with_capacity(entry.inverses.len());
        for op in entry.inverses.iter().rev() {
            inverses.push(doc.apply(op.clone()).ok()?);
        }
        Some(Entry {
            inverses,
            before: entry.before,
            after: entry.after,
            open: None,
        })
    }

    fn push(&mut self, entry: Entry) {
        if self.undo.len() == MAX_ENTRIES {
            self.undo.remove(0);
        }
        self.undo.push(entry);
    }
}

impl Open {
    fn continues(&self, next: &Open) -> bool {
        self.run == next.run
            && self.para == next.para
            && next.time.saturating_sub(self.time) <= COALESCE_GAP
            && !(next.run == Run::Typing && next.ended_in_blank && !self.ended_in_blank)
    }
}

/// Which run `op` extends, judged before it is applied.
fn classify(doc: &Document, op: &EditOp, ctx: &EditContext) -> Option<Open> {
    let (run, para, ended_in_blank) = match op {
        EditOp::InsertText { at, text, .. }
            if !text.is_empty() && !text.contains('\n') && next_grapheme(text, 0) == text.len() =>
        {
            let blank = text.chars().all(char::is_whitespace);
            (Run::Typing, at.para, blank)
        }
        EditOp::Delete { range } if range.start.para == range.end.para => {
            let text = doc.paragraphs().get(range.start.para)?.text();
            let piece = text.get(range.start.byte..range.end.byte)?;
            if piece.is_empty() || next_grapheme(piece, 0) != piece.len() {
                return None;
            }
            let caret = ctx.before.head();
            if caret == Some(range.end) {
                (Run::Backspace, range.start.para, false)
            } else if caret == Some(range.start) {
                (Run::ForwardDelete, range.start.para, false)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(Open {
        run,
        para,
        time: ctx.now,
        ended_in_blank,
    })
}
