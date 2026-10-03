//! Every edit command, with its undo.

#[path = "model_common.rs"]
mod common;
#[path = "edit_common.rs"]
mod harness;

use common::{object, runs};
use harness::Ed;
use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::{
    Align, BlockKind, CharStylePatch, ListItem, ListKind, ParaStylePatch, Selection, Side, Wrap,
};

fn undo(ed: &mut Ed) {
    assert!(ed.run(Command::Undo).doc_changed());
}

fn list_of(ed: &Ed, para: usize) -> Option<ListItem> {
    let doc = &ed.state.doc;
    doc.styles().para(doc.paragraphs()[para].style()).list
}

fn bullet(ed: &mut Ed, para: usize) {
    ed.caret_at(para, 0);
    ed.run(Command::ToggleList(ListKind::Bullet));
}

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
fn pending_style_applies_to_the_next_typed_text() {
    let mut ed = Ed::new("ab");
    ed.caret_at(0, 2);
    ed.run(Command::ToggleBold);
    assert!(ed.state.pending.is_some());
    ed.type_str("cd");
    assert_eq!(
        runs(&ed.state.doc, 0),
        vec![("ab".to_owned(), false), ("cd".to_owned(), true)]
    );
    ed.run(Command::ToggleBold);
    ed.type_str("e");
    assert_eq!(runs(&ed.state.doc, 0)[2], ("e".to_owned(), false));
    ed.run(Command::ToggleItalic);
    ed.mv(Motion::Left);
    assert!(ed.state.pending.is_none(), "moving the caret drops it");
    ed.mv(Motion::Right);
    ed.type_str("f");
    assert!(!runs(&ed.state.doc, 0).last().unwrap().1);
}

#[test]
fn pending_toggle_twice_cancels() {
    let mut ed = Ed::new("a");
    ed.caret_at(0, 1);
    ed.run(Command::ToggleBold);
    ed.run(Command::ToggleBold);
    ed.type_str("b");
    assert_eq!(runs(&ed.state.doc, 0), vec![("ab".to_owned(), false)]);
}

#[test]
fn toggles_format_a_selection_and_undo() {
    for (command, check) in [
        (Command::ToggleBold, 0),
        (Command::ToggleItalic, 1),
        (Command::ToggleUnderline, 2),
        (Command::ToggleStrike, 3),
    ] {
        let mut ed = Ed::new("abcdef");
        let flag = |ed: &Ed, byte: usize| {
            let doc = &ed.state.doc;
            let s = doc.styles().char(doc.paragraphs()[0].style_at(byte));
            [s.weight.value() >= 600, s.italic, s.underline, s.strike][check]
        };
        ed.select((0, 1), (0, 4));
        let effect = ed.run(command.clone());
        assert_eq!(effect.dirty, Some(0..1));
        assert!(flag(&ed, 1) && flag(&ed, 3) && !flag(&ed, 0) && !flag(&ed, 4));
        ed.run(command);
        assert!(!flag(&ed, 2), "toggling again clears it");
        undo(&mut ed);
        assert!(flag(&ed, 2));
        undo(&mut ed);
        assert!(!flag(&ed, 2));
    }
}

#[test]
fn mixed_selection_toggles_on_first() {
    let mut ed = Ed::new("abcd");
    ed.select((0, 0), (0, 2));
    ed.run(Command::ToggleBold);
    ed.select((0, 0), (0, 4));
    ed.run(Command::ToggleBold);
    assert_eq!(runs(&ed.state.doc, 0), vec![("abcd".to_owned(), true)]);
}

#[test]
fn set_char_style_and_para_style() {
    let mut ed = Ed::new("abc\ndef");
    ed.select((0, 0), (1, 2));
    ed.run(Command::SetCharStyle(CharStylePatch::bold(true)));
    assert!(runs(&ed.state.doc, 1)[0].1);
    ed.run(Command::SetAlign(Align::Center));
    ed.run(Command::SetBlockKind(BlockKind::Heading(2)));
    let doc = &ed.state.doc;
    for p in 0..2 {
        let s = doc.styles().para(doc.paragraphs()[p].style());
        assert_eq!((s.align, s.kind), (Align::Center, BlockKind::Heading(2)));
    }
    ed.run(Command::SetParaStyle(ParaStylePatch::align(Align::Right)));
    undo(&mut ed);
    undo(&mut ed);
    undo(&mut ed);
    undo(&mut ed);
    assert!(!runs(&ed.state.doc, 1)[0].1);
}

#[test]
fn a_range_ending_at_a_paragraph_start_leaves_that_paragraph_alone() {
    let mut ed = Ed::new("a\nb");
    ed.select((0, 0), (1, 0));
    ed.run(Command::SetAlign(Align::Right));
    let doc = &ed.state.doc;
    assert_eq!(
        doc.styles().para(doc.paragraphs()[0].style()).align,
        Align::Right
    );
    assert_eq!(
        doc.styles().para(doc.paragraphs()[1].style()).align,
        Align::Left
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
fn horizontal_motion() {
    let mut ed = Ed::new("ab cd\nef");
    ed.mv(Motion::Right);
    assert_eq!(ed.caret(), DocPos::new(0, 1));
    ed.mv(Motion::WordRight);
    assert_eq!(ed.caret(), DocPos::new(0, 3));
    ed.mv(Motion::WordRight);
    assert_eq!(ed.caret(), DocPos::new(0, 5));
    ed.mv(Motion::WordRight);
    assert_eq!(ed.caret(), DocPos::new(1, 0));
    ed.mv(Motion::Left);
    assert_eq!(ed.caret(), DocPos::new(0, 5));
    ed.mv(Motion::WordLeft);
    assert_eq!(ed.caret(), DocPos::new(0, 3));
    ed.mv(Motion::DocEnd);
    assert_eq!(ed.caret(), DocPos::new(1, 2));
    ed.mv(Motion::DocStart);
    assert_eq!(ed.caret(), DocPos::new(0, 0));
    assert!(ed.mv(Motion::Left).is_none());
}

#[test]
fn extending_and_collapsing_a_selection() {
    let mut ed = Ed::new("abcd");
    ed.caret_at(0, 1);
    ed.run(Command::Move {
        motion: Motion::Right,
        extend: true,
    });
    ed.run(Command::Move {
        motion: Motion::Right,
        extend: true,
    });
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(0, 1), DocPos::new(0, 3))
    );
    ed.mv(Motion::Left);
    assert_eq!(ed.state.selection, Selection::caret(DocPos::new(0, 1)));
    ed.run(Command::Move {
        motion: Motion::Right,
        extend: true,
    });
    ed.mv(Motion::Right);
    assert_eq!(ed.state.selection, Selection::caret(DocPos::new(0, 2)));
}

#[test]
fn line_motions_use_the_layout_and_keep_a_sticky_x() {
    let mut ed = Ed::new("abcdef\nab\nabcdef");
    ed.caret_at(0, 5);
    ed.mv(Motion::Down);
    assert_eq!(ed.caret(), DocPos::new(1, 2));
    assert_eq!(ed.state.sticky_x, Some(5.0));
    ed.mv(Motion::Down);
    assert_eq!(ed.caret(), DocPos::new(2, 5), "the x came back");
    ed.mv(Motion::Left);
    assert_eq!(ed.state.sticky_x, None);
    ed.mv(Motion::Up);
    assert_eq!(ed.caret(), DocPos::new(1, 2));
    ed.mv(Motion::LineStart);
    assert_eq!(ed.caret(), DocPos::new(1, 0));
    ed.mv(Motion::LineEnd);
    assert_eq!(ed.caret(), DocPos::new(1, 2));
    ed.mv(Motion::PageDown);
    assert_eq!(ed.caret().para, 2);
    ed.mv(Motion::PageUp);
    assert_eq!(ed.caret().para, 0);
}

#[test]
fn selection_commands() {
    let mut ed = Ed::new("one two\nthree");
    ed.run(Command::SelectAll);
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(0, 0), DocPos::new(1, 5))
    );
    ed.run(Command::SelectWord(DocPos::new(0, 5)));
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(0, 4), DocPos::new(0, 7))
    );
    ed.run(Command::SelectParagraph(DocPos::new(1, 2)));
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(1, 0), DocPos::new(1, 5))
    );
    ed.run(Command::SetCaret {
        pos: DocPos::new(0, 2),
        extend: false,
    });
    ed.run(Command::SetCaret {
        pos: DocPos::new(1, 99),
        extend: true,
    });
    assert_eq!(
        ed.state.selection,
        Selection::text(DocPos::new(0, 2), DocPos::new(1, 5))
    );
}

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
