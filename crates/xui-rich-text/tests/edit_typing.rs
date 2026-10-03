//! Typing, Enter, deleting and undo/redo commands.

#[path = "edit_common.rs"]
mod harness;

use harness::{Ed, undo};
use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::{BlockKind, Selection};

#[test]
fn typing_is_one_step_and_undo_restores_the_caret() {
    let mut ed = Ed::new("");
    ed.type_str("hello");
    assert_eq!(ed.text(), "hello");
    assert_eq!(ed.caret(), DocPos::new(0, 5));
    undo(&mut ed);
    assert_eq!(ed.text(), "");
    assert_eq!(ed.caret(), DocPos::new(0, 0));
    assert!(!ed.state.history.can_undo());
    ed.run(Command::Redo);
    assert_eq!(ed.text(), "hello");
    assert_eq!(ed.caret(), DocPos::new(0, 5));
}

#[test]
fn typing_effect_names_the_dirty_paragraph() {
    let mut ed = Ed::new("a\nb\nc");
    ed.caret_at(1, 1);
    let effect = ed.run(Command::InsertText("x".into()));
    assert_eq!(effect.dirty, Some(1..2));
    assert!(effect.selection);
    let effect = ed.run(Command::InsertText("y\nz".into()));
    assert_eq!(effect.dirty, Some(1..4), "a split runs to the end");
}

#[test]
fn typing_over_a_selection_is_one_undo_step() {
    let mut ed = Ed::new("hello world");
    ed.select((0, 0), (0, 5));
    ed.run(Command::InsertText("bye".into()));
    assert_eq!(ed.text(), "bye world");
    assert_eq!(ed.caret(), DocPos::new(0, 3));
    undo(&mut ed);
    assert_eq!(ed.text(), "hello world");
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(0, 0), DocPos::new(0, 5))
    );
}

#[test]
fn enter_splits_and_shift_enter_breaks() {
    let mut ed = Ed::new("abcd");
    ed.caret_at(0, 2);
    let effect = ed.run(Command::InsertParagraph);
    assert_eq!(ed.text(), "ab\ncd");
    assert_eq!(ed.caret(), DocPos::new(1, 0));
    assert_eq!(effect.dirty, Some(0..2));
    ed.run(Command::InsertLineBreak);
    assert_eq!(ed.text(), "ab\n\u{2028}cd");
    undo(&mut ed);
    undo(&mut ed);
    assert_eq!(ed.text(), "abcd");
    assert_eq!(ed.caret(), DocPos::new(0, 2));
}

#[test]
fn enter_over_a_selection_replaces_it() {
    let mut ed = Ed::new("abcd");
    ed.select((0, 1), (0, 3));
    ed.run(Command::InsertParagraph);
    assert_eq!(ed.text(), "a\nd");
    undo(&mut ed);
    assert_eq!(ed.text(), "abcd");
}

#[test]
fn enter_at_the_end_of_a_heading_starts_body_text() {
    let mut ed = Ed::new("Title");
    ed.run(Command::SetBlockKind(BlockKind::Heading(1)));
    ed.caret_at(0, 5);
    ed.run(Command::InsertParagraph);
    let doc = &ed.state.doc;
    assert_eq!(
        doc.styles().para(doc.paragraphs()[0].style()).kind,
        BlockKind::Heading(1)
    );
    assert_eq!(
        doc.styles().para(doc.paragraphs()[1].style()).kind,
        BlockKind::Body
    );
}

#[test]
fn backspace_and_delete_remove_whole_graphemes() {
    let family = "\u{1F468}\u{200D}\u{1F469}";
    let mut ed = Ed::new(&format!("a{family}e\u{301}"));
    ed.run(Command::Move {
        motion: Motion::DocEnd,
        extend: false,
    });
    ed.run(Command::Backspace);
    assert_eq!(ed.text(), format!("a{family}"));
    ed.run(Command::Backspace);
    assert_eq!(ed.text(), "a");
    undo(&mut ed);
    assert_eq!(
        ed.text(),
        format!("a{family}e\u{301}"),
        "backspaces coalesce"
    );
    ed.caret_at(0, 1);
    ed.run(Command::Delete);
    assert_eq!(ed.text(), "ae\u{301}");
    undo(&mut ed);
}

#[test]
fn word_deletion() {
    let mut ed = Ed::new("one two three");
    ed.caret_at(0, 7);
    ed.run(Command::DeleteWordBack);
    assert_eq!(ed.text(), "one  three");
    assert_eq!(ed.caret(), DocPos::new(0, 4));
    ed.run(Command::DeleteWordForward);
    assert_eq!(ed.text(), "one three");
    undo(&mut ed);
    undo(&mut ed);
    assert_eq!(ed.text(), "one two three");
}

#[test]
fn delete_merges_at_a_paragraph_end_and_stops_at_the_document_end() {
    let mut ed = Ed::new("a\nb");
    ed.caret_at(0, 1);
    ed.run(Command::Delete);
    assert_eq!(ed.text(), "ab");
    assert_eq!(ed.caret(), DocPos::new(0, 1));
    ed.mv(Motion::DocEnd);
    assert!(ed.run(Command::Delete).is_none());
    ed.caret_at(0, 0);
    assert!(ed.run(Command::Backspace).is_none());
    undo(&mut ed);
    assert_eq!(ed.text(), "a\nb");
}

#[test]
fn deleting_a_selection_and_undo() {
    let mut ed = Ed::new("abc\ndef");
    ed.select((0, 1), (1, 2));
    ed.run(Command::Delete);
    assert_eq!(ed.text(), "af");
    undo(&mut ed);
    assert_eq!(ed.text(), "abc\ndef");
    ed.run(Command::Backspace);
    assert_eq!(ed.text(), "af");
}

#[test]
fn undo_and_redo_commands_report_the_whole_document() {
    let mut ed = Ed::new("a");
    assert!(ed.run(Command::Undo).is_none());
    ed.caret_at(0, 1);
    ed.type_str("b");
    let effect = ed.run(Command::Undo);
    assert_eq!(effect.dirty, Some(0..1));
    assert!(effect.selection);
    assert!(ed.run(Command::Redo).doc_changed());
    assert!(ed.run(Command::Redo).is_none());
}

#[test]
fn replacing_paragraphs_with_as_many_dirties_all_of_them() {
    // The paragraph count is unchanged, so only the inserted span can say
    // that paragraph 2 changed too.
    let mut ed = Ed::new("abc\ndef\nghi\njkl");
    ed.select((0, 1), (2, 1));
    let effect = ed.run(Command::InsertText("x\ny\nz".into()));
    assert_eq!(ed.text(), "ax\ny\nzhi\njkl");
    let dirty = effect.dirty.expect("dirty");
    assert!(
        dirty.start == 0 && dirty.end >= 3,
        "{dirty:?} misses paragraph 2"
    );
}
