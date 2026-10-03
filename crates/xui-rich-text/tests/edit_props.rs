//! Random command sequences keep the document and selection valid.

#[path = "model_common.rs"]
mod common;
#[path = "edit_common.rs"]
mod harness;

use common::object;
use harness::Ed;
use proptest::prelude::*;
use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::{Align, BlockKind, CharStylePatch, ListKind, ParaStylePatch, Selection};

const MOTIONS: [Motion; 12] = [
    Motion::Left,
    Motion::Right,
    Motion::WordLeft,
    Motion::WordRight,
    Motion::Up,
    Motion::Down,
    Motion::LineStart,
    Motion::LineEnd,
    Motion::DocStart,
    Motion::DocEnd,
    Motion::PageUp,
    Motion::PageDown,
];

fn command() -> impl Strategy<Value = Command> {
    let pos = (0..6usize, 0..12usize).prop_map(|(p, b)| DocPos::new(p, b));
    prop_oneof![
        4 => common::text().prop_map(Command::InsertText),
        1 => Just(Command::InsertParagraph),
        1 => Just(Command::InsertLineBreak),
        2 => Just(Command::Backspace),
        2 => Just(Command::Delete),
        1 => Just(Command::DeleteWordBack),
        1 => Just(Command::DeleteWordForward),
        4 => (0..12usize, any::<bool>())
            .prop_map(|(m, extend)| Command::Move { motion: MOTIONS[m], extend }),
        1 => Just(Command::SelectAll),
        1 => pos.clone().prop_map(Command::SelectWord),
        1 => pos.clone().prop_map(Command::SelectParagraph),
        2 => (pos, any::<bool>()).prop_map(|(pos, extend)| Command::SetCaret { pos, extend }),
        1 => Just(Command::ToggleBold),
        1 => Just(Command::ToggleItalic),
        1 => Just(Command::ToggleUnderline),
        1 => Just(Command::ToggleStrike),
        1 => Just(Command::SetCharStyle(CharStylePatch::strike(true))),
        1 => Just(Command::SetParaStyle(ParaStylePatch::align(Align::Right))),
        1 => Just(Command::SetAlign(Align::Center)),
        1 => Just(Command::ToggleList(ListKind::Bullet)),
        1 => Just(Command::ToggleList(ListKind::Numbered)),
        1 => Just(Command::Indent),
        1 => Just(Command::Outdent),
        1 => Just(Command::SetBlockKind(BlockKind::Quote)),
        1 => (0..8u8).prop_map(|s| Command::InsertImage(object(s))),
        1 => Just(Command::Undo),
        1 => Just(Command::Redo),
        1 => Just(Command::Cut),
        1 => Just(Command::Copy),
        2 => Just(Command::Paste),
    ]
}

proptest! {
    #[test]
    fn random_commands_keep_the_document_and_selection_valid(
        initial in common::text(),
        commands in prop::collection::vec(command(), 1..60),
    ) {
        let mut ed = Ed::new(&initial);
        for command in commands {
            let effect = ed.run(command);
            if let Some(dirty) = effect.dirty {
                prop_assert!(dirty.start < dirty.end.max(1));
                prop_assert!(dirty.end <= ed.state.doc.paragraph_count());
            }
            if let Selection::Text { anchor, head } = ed.state.selection {
                prop_assert!(anchor.para < ed.state.doc.paragraph_count());
                prop_assert!(head.para < ed.state.doc.paragraph_count());
            }
        }
        while ed.state.history.can_undo() {
            ed.run(Command::Undo);
        }
        prop_assert_eq!(ed.text(), initial.replace('\r', ""));
    }
}
