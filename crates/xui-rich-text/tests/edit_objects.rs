//! Image interaction commands: resize, preview, move and wrap.

#[path = "model_common.rs"]
mod common;
#[path = "edit_common.rs"]
mod harness;

use common::object;
use harness::Ed;
use xui_core::Dip;
use xui_rich_text::DocPos;
use xui_rich_text::edit::Command;
use xui_rich_text::model::{ObjectId, Selection, Side, Wrap};

/// "ab\ncd" with an image between a and b.
fn with_image() -> (Ed, ObjectId) {
    let mut ed = Ed::new("ab\ncd");
    ed.caret_at(0, 1);
    ed.run(Command::InsertImage(object(2)));
    let Selection::Object(id) = ed.state.selection else {
        panic!("the image is selected");
    };
    (ed, id)
}

fn size(ed: &Ed, id: ObjectId) -> (Dip, Dip) {
    ed.state.doc.objects().get(id).unwrap().size
}

fn undo(ed: &mut Ed) {
    assert!(ed.run(Command::Undo).doc_changed());
}

#[test]
fn resize_is_one_undo_step() {
    let (mut ed, id) = with_image();
    let original = size(&ed, id);
    let effect = ed.run(Command::ResizeObject {
        id,
        size: (Dip(120.0), Dip(80.0)),
    });
    assert!(effect.doc_changed());
    assert_eq!(size(&ed, id), (Dip(120.0), Dip(80.0)));
    assert_eq!(ed.state.selection, Selection::Object(id));
    undo(&mut ed);
    assert_eq!(size(&ed, id), original);
    ed.run(Command::Redo);
    assert_eq!(size(&ed, id), (Dip(120.0), Dip(80.0)));
}

#[test]
fn a_preview_is_not_recorded_and_the_commit_undoes_to_the_original() {
    let (mut ed, id) = with_image();
    let original = size(&ed, id);
    ed.state.history.seal();
    for w in [50.0, 70.0, 90.0] {
        let effect = ed.state.preview_object_size(id, (Dip(w), Dip(w)));
        assert!(effect.doc_changed());
        assert_eq!(size(&ed, id), (Dip(w), Dip(w)));
    }
    ed.run(Command::ResizeObject {
        id,
        size: (Dip(90.0), Dip(90.0)),
    });
    undo(&mut ed);
    assert_eq!(size(&ed, id), original, "one step back to before the drag");
    ed.run(Command::Redo);
    assert_eq!(size(&ed, id), (Dip(90.0), Dip(90.0)));

    ed.state.preview_object_size(id, (Dip(20.0), Dip(20.0)));
    let effect = ed.state.cancel_preview();
    assert!(effect.doc_changed());
    assert_eq!(size(&ed, id), (Dip(90.0), Dip(90.0)));
    assert!(ed.state.cancel_preview().is_none());
}

#[test]
fn resizing_to_the_same_size_or_a_missing_image_does_nothing() {
    let (mut ed, id) = with_image();
    let same = size(&ed, id);
    assert!(ed.run(Command::ResizeObject { id, size: same }).is_none());
    ed.run(Command::Delete);
    let effect = ed.run(Command::ResizeObject {
        id,
        size: (Dip(50.0), Dip(50.0)),
    });
    assert!(effect.is_none());
}

#[test]
fn set_wrap_changes_the_flow_and_undoes() {
    let (mut ed, id) = with_image();
    let wrap = Wrap::square(Side::Right);
    ed.run(Command::SetWrap { id, wrap });
    assert_eq!(ed.state.doc.objects().get(id).unwrap().wrap, wrap);
    assert!(ed.run(Command::SetWrap { id, wrap }).is_none());
    undo(&mut ed);
    assert_eq!(ed.state.doc.objects().get(id).unwrap().wrap, Wrap::Inline);
}

#[test]
fn moving_an_anchor_keeps_its_id_and_is_one_step() {
    let (mut ed, id) = with_image();
    let effect = ed.run(Command::MoveObject {
        id,
        to: DocPos::new(1, 1),
    });
    assert!(effect.doc_changed());
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(1, 1)));
    assert_eq!(ed.text(), "ab\ncd");
    assert_eq!(ed.state.selection, Selection::Object(id));
    assert_eq!(ed.state.doc.objects().len(), 1);
    undo(&mut ed);
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 1)));
    ed.run(Command::Redo);
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(1, 1)));
}

#[test]
fn moving_within_a_paragraph_accounts_for_the_removed_anchor() {
    let (mut ed, id) = with_image();
    let end = ed.state.doc.paragraphs()[0].text().len();
    ed.run(Command::MoveObject {
        id,
        to: DocPos::new(0, end),
    });
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 2)));
    let (mut ed, id) = with_image();
    ed.run(Command::MoveObject {
        id,
        to: DocPos::new(0, 0),
    });
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 0)));
    ed.run(Command::MoveObject {
        id,
        to: DocPos::new(0, 4),
    });
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 1)));
    ed.state.doc.check().unwrap();
}

#[test]
fn moving_next_to_itself_is_a_no_op() {
    let (mut ed, id) = with_image();
    for byte in [1, 4] {
        let effect = ed.run(Command::MoveObject {
            id,
            to: DocPos::new(0, byte),
        });
        assert!(effect.is_none(), "{byte}");
    }
    assert_eq!(ed.state.doc.object_pos(id), Some(DocPos::new(0, 1)));
}
