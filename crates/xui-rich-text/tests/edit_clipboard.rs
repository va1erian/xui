//! Cut, copy and paste, rich and plain.

#[path = "model_common.rs"]
mod common;
#[path = "edit_common.rs"]
mod harness;

use common::{object, runs};
use harness::{Ed, undo};
use xui_rich_text::DocPos;
use xui_rich_text::edit::Command;

#[test]
fn rich_copy_paste_keeps_formatting() {
    let mut ed = Ed::new("abc def");
    ed.select((0, 0), (0, 3));
    ed.run(Command::ToggleBold);
    ed.select((0, 0), (0, 3));
    let effect = ed.run(Command::Copy);
    assert_eq!(effect.copied.as_deref(), Some("abc"));
    assert!(!effect.doc_changed());
    ed.caret_at(0, 7);
    ed.run(Command::Paste);
    assert_eq!(ed.text(), "abc defabc");
    assert_eq!(
        runs(&ed.state.doc, 0),
        vec![
            ("abc".to_owned(), true),
            (" def".to_owned(), false),
            ("abc".to_owned(), true)
        ]
    );
    undo(&mut ed);
    assert_eq!(ed.text(), "abc def");
}

#[test]
fn paste_falls_back_to_plain_text_when_the_clipboard_changed() {
    use xui_rich_text::edit::Clipboard;
    let mut ed = Ed::new("abc");
    ed.select((0, 0), (0, 3));
    ed.run(Command::ToggleBold);
    ed.select((0, 0), (0, 3));
    ed.run(Command::Copy);
    ed.clipboard.set_text("x\r\ny");
    ed.caret_at(0, 3);
    ed.run(Command::Paste);
    assert_eq!(ed.text(), "abcx\ny");
    assert_eq!(runs(&ed.state.doc, 0), vec![("abcx".to_owned(), true)]);
    assert_eq!(ed.caret(), DocPos::new(1, 1));
    ed.clipboard.set_text("same");
    ed.select((0, 0), (0, 1));
    ed.run(Command::Paste);
    assert_eq!(ed.text(), "samebcx\ny");
}

#[test]
fn cut_removes_and_paste_restores_with_images() {
    let mut ed = Ed::new("ab\ncd");
    ed.caret_at(0, 1);
    ed.run(Command::InsertImage(object(4)));
    ed.select((0, 0), (1, 1));
    let effect = ed.run(Command::Cut);
    assert_eq!(effect.copied.as_deref(), Some("ab\nc"));
    assert_eq!(ed.text(), "d");
    assert!(ed.state.doc.objects().is_empty());
    ed.run(Command::Paste);
    assert_eq!(ed.text(), "ab\ncd");
    assert_eq!(ed.state.doc.objects().len(), 1);
    assert_eq!(ed.caret(), DocPos::new(1, 1));
    undo(&mut ed);
    assert_eq!(ed.text(), "d");
    undo(&mut ed);
    assert_eq!(ed.text(), "ab\ncd");
    assert_eq!(ed.state.doc.objects().len(), 1);
}

#[test]
fn copying_nothing_does_nothing_and_an_image_copies_as_text() {
    let mut ed = Ed::new("ab");
    assert!(ed.run(Command::Copy).is_none());
    assert!(ed.run(Command::Cut).is_none());
    ed.caret_at(0, 1);
    ed.run(Command::InsertImage(object(2)));
    assert_eq!(ed.run(Command::Copy).copied.as_deref(), Some("image 2"));
    ed.caret_at(0, 0);
    ed.run(Command::Paste);
    assert_eq!(ed.state.doc.objects().len(), 2);
}
