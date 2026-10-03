//! List commands: toggling, Enter, Backspace and indentation.

#[path = "edit_common.rs"]
mod harness;

use harness::{Ed, bullet, list_of, undo};
use xui_rich_text::DocPos;
use xui_rich_text::edit::Command;
use xui_rich_text::model::ListKind;

#[test]
fn lists_toggle_continue_and_exit() {
    let mut ed = Ed::new("one");
    ed.caret_at(0, 3);
    ed.run(Command::ToggleList(ListKind::Numbered));
    assert_eq!(list_of(&ed, 0).unwrap().kind, ListKind::Numbered);
    assert!(ed.state.in_list());
    ed.run(Command::InsertParagraph);
    assert_eq!(
        list_of(&ed, 1).unwrap().kind,
        ListKind::Numbered,
        "Enter continues"
    );
    ed.run(Command::InsertParagraph);
    assert_eq!(list_of(&ed, 1), None, "Enter on an empty item exits");
    assert_eq!(ed.text(), "one\n");
    undo(&mut ed);
    assert!(list_of(&ed, 1).is_some());
    ed.run(Command::ToggleList(ListKind::Bullet));
    assert_eq!(
        list_of(&ed, 1).unwrap().kind,
        ListKind::Bullet,
        "switches kind"
    );
    ed.run(Command::ToggleList(ListKind::Bullet));
    assert_eq!(list_of(&ed, 1), None, "same kind removes it");
}

#[test]
fn backspace_at_a_list_start_leaves_the_list_then_merges() {
    let mut ed = Ed::new("a\nb");
    bullet(&mut ed, 1);
    ed.caret_at(1, 0);
    ed.run(Command::Backspace);
    assert_eq!(list_of(&ed, 1), None);
    assert_eq!(ed.text(), "a\nb");
    ed.run(Command::Backspace);
    assert_eq!(ed.text(), "ab");
    assert_eq!(ed.caret(), DocPos::new(0, 1));
    undo(&mut ed);
    assert_eq!(ed.text(), "a\nb");
    undo(&mut ed);
    assert!(list_of(&ed, 1).is_some());
}

#[test]
fn indent_and_outdent_lists_and_plain_paragraphs() {
    let mut ed = Ed::new("a\nb");
    bullet(&mut ed, 0);
    ed.run(Command::Indent);
    assert_eq!(list_of(&ed, 0).unwrap().level, 1);
    ed.run(Command::Indent);
    ed.run(Command::Outdent);
    assert_eq!(list_of(&ed, 0).unwrap().level, 1);
    ed.run(Command::Outdent);
    ed.run(Command::Outdent);
    assert_eq!(list_of(&ed, 0).unwrap().level, 0);

    ed.caret_at(1, 0);
    assert!(!ed.state.in_list());
    ed.run(Command::Indent);
    let doc = &ed.state.doc;
    let indent = doc.styles().para(doc.paragraphs()[1].style()).indent_left.0;
    assert!(indent > 0.0);
    ed.run(Command::Outdent);
    ed.run(Command::Outdent);
    let doc = &ed.state.doc;
    assert_eq!(
        doc.styles().para(doc.paragraphs()[1].style()).indent_left.0,
        0.0
    );
}
