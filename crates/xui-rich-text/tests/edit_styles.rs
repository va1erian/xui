//! Pending styles, toggles and character/paragraph style commands.

#[path = "model_common.rs"]
mod common;
#[path = "edit_common.rs"]
mod harness;

use common::runs;
use harness::{Ed, undo};
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::{Align, BlockKind, CharStylePatch, ParaStylePatch};

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
