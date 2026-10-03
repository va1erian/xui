//! Image commands: insert, select, delete, resize.

#[path = "model_common.rs"]
mod common;
#[path = "edit_common.rs"]
mod harness;

use common::object;
use harness::{Ed, undo};
use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::{Selection, Side, Wrap};

#[test]
fn images_insert_select_move_past_and_delete() {
    let mut ed = Ed::new("ab");
    ed.caret_at(0, 1);
    ed.run(Command::InsertImage(object(2)));
    let Selection::Object(id) = ed.state.selection else {
        panic!("the inserted image is selected");
    };
    assert_eq!(ed.text(), "ab");
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 1)));

    ed.mv(Motion::Left);
    assert_eq!(ed.caret(), DocPos::new(0, 1));
    ed.mv(Motion::Right);
    assert_eq!(ed.caret(), DocPos::new(0, 4), "one step over the image");
    ed.mv(Motion::Left);
    assert_eq!(ed.caret(), DocPos::new(0, 1));

    ed.run(Command::SelectObject(id));
    ed.run(Command::Delete);
    assert!(ed.state.doc.objects().get(id).is_none());
    assert_eq!(ed.caret(), DocPos::new(0, 1));
    undo(&mut ed);
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 1)));
    assert_eq!(
        ed.state.selection,
        Selection::Object(id),
        "undo restores the selection"
    );
}

#[test]
fn backspace_over_an_image_removes_it_and_typing_replaces_a_selected_one() {
    let mut ed = Ed::new("ab");
    ed.caret_at(0, 1);
    ed.run(Command::InsertImage(object(1)));
    ed.mv(Motion::Right);
    ed.run(Command::Backspace);
    assert!(ed.state.doc.objects().is_empty());
    ed.run(Command::InsertImage(object(1)));
    ed.run(Command::InsertText("x".into()));
    assert_eq!(ed.text(), "axb");
    assert!(ed.state.doc.objects().is_empty());
}

#[test]
fn set_object_resizes_and_undoes() {
    let mut ed = Ed::new("ab");
    ed.run(Command::InsertImage(object(2)));
    let Selection::Object(id) = ed.state.selection else {
        panic!()
    };
    let mut bigger = object(2);
    bigger.wrap = Wrap::square(Side::Right);
    bigger.alt = "changed".into();
    ed.run(Command::SetObject { id, object: bigger });
    assert_eq!(ed.state.doc.objects().get(id).unwrap().alt, "changed");
    assert_eq!(ed.state.selection, Selection::Object(id));
    undo(&mut ed);
    assert_eq!(ed.state.doc.objects().get(id).unwrap().alt, "image 2");
}
