//! Undo/redo: transactions and coalescing.

#[path = "model_common.rs"]
mod common;

use std::time::Duration;

use common::range;
use xui_rich_text::model::{DocPos, Document, EditContext, EditError, EditOp, History, Selection};

fn caret(para: usize, byte: usize) -> Selection {
    Selection::caret(DocPos::new(para, byte))
}

/// Types `text` one character at a time, `gap` apart, from the end of the
/// first paragraph.
fn type_text(doc: &mut Document, history: &mut History, text: &str, start: u64, gap: u64) -> u64 {
    let mut now = start;
    for ch in text.chars() {
        let at = doc.paragraphs()[0].text().len();
        let ctx = EditContext::new(caret(0, at), caret(0, at + ch.len_utf8()))
            .at(Duration::from_millis(now));
        let op = EditOp::InsertText {
            at: DocPos::new(0, at),
            text: ch.to_string(),
            style: None,
        };
        history.edit(doc, op, ctx).unwrap();
        now += gap;
    }
    now
}

fn undo_all(doc: &mut Document, history: &mut History) -> usize {
    let mut steps = 0;
    while history.undo(doc).is_some() {
        steps += 1;
    }
    steps
}

#[test]
fn typing_in_a_word_is_one_step() {
    let mut doc = Document::new();
    let mut history = History::new();
    type_text(&mut doc, &mut history, "hello", 0, 100);
    assert_eq!(doc.to_plain_text(), "hello");
    assert_eq!(undo_all(&mut doc, &mut history), 1);
    assert_eq!(doc.to_plain_text(), "");
    assert!(history.redo(&mut doc).is_some());
    assert_eq!(doc.to_plain_text(), "hello");
}

#[test]
fn a_blank_after_a_word_starts_a_new_step() {
    let mut doc = Document::new();
    let mut history = History::new();
    type_text(&mut doc, &mut history, "one two", 0, 100);
    assert_eq!(undo_all(&mut doc, &mut history), 2);
    type_text(&mut doc, &mut history, "ab  cd", 0, 100);
    doc.check().unwrap();
    assert_eq!(history.undo(&mut doc), Some(caret(0, 2)));
    assert_eq!(doc.to_plain_text(), "ab");
}

#[test]
fn a_pause_starts_a_new_step() {
    let mut doc = Document::new();
    let mut history = History::new();
    let now = type_text(&mut doc, &mut history, "ab", 0, 100);
    type_text(&mut doc, &mut history, "cd", now + 5000, 100);
    assert_eq!(undo_all(&mut doc, &mut history), 2);
}

#[test]
fn a_caret_jump_starts_a_new_step() {
    let mut doc = Document::from_plain_text("abc");
    let mut history = History::new();
    for (at, ch) in [(1, "x"), (3, "y")] {
        let ctx = EditContext::new(caret(0, at), caret(0, at + 1));
        let op = EditOp::InsertText {
            at: DocPos::new(0, at),
            text: ch.into(),
            style: None,
        };
        history.edit(&mut doc, op, ctx).unwrap();
    }
    assert_eq!(doc.to_plain_text(), "axbyc");
    assert_eq!(undo_all(&mut doc, &mut history), 2);
    assert_eq!(doc.to_plain_text(), "abc");
}

#[test]
fn backspaces_and_forward_deletes_coalesce() {
    let mut doc = Document::from_plain_text("abcdef");
    let mut history = History::new();
    for end in (3..=5).rev() {
        let op = EditOp::Delete {
            range: range(0, end - 1, 0, end),
        };
        history
            .edit(
                &mut doc,
                op,
                EditContext::new(caret(0, end), caret(0, end - 1)),
            )
            .unwrap();
    }
    assert_eq!(doc.to_plain_text(), "abf");
    assert_eq!(history.undo(&mut doc), Some(caret(0, 5)));
    assert_eq!(doc.to_plain_text(), "abcdef");
    assert!(!history.can_undo());

    for _ in 0..3 {
        let op = EditOp::Delete {
            range: range(0, 1, 0, 2),
        };
        history
            .edit(&mut doc, op, EditContext::new(caret(0, 1), caret(0, 1)))
            .unwrap();
    }
    assert_eq!(doc.to_plain_text(), "aef");
    assert_eq!(undo_all(&mut doc, &mut history), 1);
    assert_eq!(doc.to_plain_text(), "abcdef");
}

#[test]
fn typing_after_undo_does_not_merge_into_older_typing() {
    let mut doc = Document::new();
    let mut history = History::new();
    type_text(&mut doc, &mut history, "ab", 0, 100);
    history.seal();
    type_text(&mut doc, &mut history, "cd", 400, 100);
    assert_eq!(history.undo(&mut doc), Some(caret(0, 2)));
    type_text(&mut doc, &mut history, "x", 800, 100);
    assert_eq!(doc.to_plain_text(), "abx");
    assert!(!history.can_redo());
    assert_eq!(undo_all(&mut doc, &mut history), 2);
}

#[test]
fn undo_restores_the_selection_around_a_transaction() {
    let mut doc = Document::from_plain_text("hello world");
    let mut history = History::new();
    let before = Selection::text(DocPos::new(0, 0), DocPos::new(0, 5));
    let after = history
        .transact(&mut doc, before, |tx| {
            let caret = tx.insert_text(&before, "bye")?;
            tx.apply(EditOp::SplitParagraph { at: caret })?;
            Ok(Selection::caret(DocPos::new(1, 0)))
        })
        .unwrap();
    assert_eq!(doc.to_plain_text(), "bye\n world");
    assert_eq!(history.undo(&mut doc), Some(before));
    assert_eq!(doc.to_plain_text(), "hello world");
    assert_eq!(history.redo(&mut doc), Some(after));
    assert_eq!(doc.to_plain_text(), "bye\n world");
}

#[test]
fn a_failed_transaction_rolls_back() {
    let mut doc = Document::from_plain_text("abc");
    let mut history = History::new();
    let result = history.transact(&mut doc, caret(0, 0), |tx| {
        tx.apply(EditOp::Delete {
            range: range(0, 0, 0, 2),
        })?;
        tx.apply(EditOp::MergeParagraph { para: 9 })?;
        Ok(caret(0, 0))
    });
    assert!(matches!(result, Err(EditError::BadParagraphs(_))));
    assert_eq!(doc.to_plain_text(), "abc");
    assert!(!history.can_undo());
}
