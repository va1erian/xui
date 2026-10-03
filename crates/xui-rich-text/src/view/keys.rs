#![forbid(unsafe_code)]

//! The keyboard bindings: a key press to the [`Command`] it runs.

use xui_core::message::{Key, Modifiers};

use crate::edit::{Command, Motion};

/// Where Tab goes: what the caret is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabTarget {
    /// Plain text: Tab types a tab.
    Text,
    /// A list item: Tab indents, Shift+Tab outdents.
    List,
    /// A table cell: Tab and Shift+Tab move between cells.
    Table,
}

/// The command for `key` pressed with `mods`, or `None` when the key does
/// nothing in an editor. `tab` says what Tab does.
pub(crate) fn command_for(key: Key, mods: Modifiers, tab: TabTarget) -> Option<Command> {
    // AltGr arrives as Ctrl+Alt and types characters, not shortcuts.
    let ctrl = mods.ctrl && !mods.alt;
    let extend = mods.shift;
    let motion = |motion| Some(Command::Move { motion, extend });
    match key {
        Key::LEFT if ctrl => motion(Motion::WordLeft),
        Key::LEFT => motion(Motion::Left),
        Key::RIGHT if ctrl => motion(Motion::WordRight),
        Key::RIGHT => motion(Motion::Right),
        Key::UP => motion(Motion::Up),
        Key::DOWN => motion(Motion::Down),
        Key::HOME if ctrl => motion(Motion::DocStart),
        Key::HOME => motion(Motion::LineStart),
        Key::END if ctrl => motion(Motion::DocEnd),
        Key::END => motion(Motion::LineEnd),
        Key::PAGE_UP => motion(Motion::PageUp),
        Key::PAGE_DOWN => motion(Motion::PageDown),
        Key::BACK if ctrl => Some(Command::DeleteWordBack),
        Key::BACK => Some(Command::Backspace),
        Key::DELETE if ctrl => Some(Command::DeleteWordForward),
        Key::DELETE => Some(Command::Delete),
        Key::RETURN if ctrl => Some(Command::InsertPageBreak),
        Key::RETURN if mods.shift => Some(Command::InsertLineBreak),
        Key::RETURN => Some(Command::InsertParagraph),
        Key::TAB if ctrl => None,
        Key::TAB if tab == TabTarget::Table && mods.shift => Some(Command::PrevCell),
        Key::TAB if tab == TabTarget::Table => Some(Command::NextCell),
        Key::TAB if tab == TabTarget::List && mods.shift => Some(Command::Outdent),
        Key::TAB if tab == TabTarget::List => Some(Command::Indent),
        Key::TAB if mods.shift => None,
        Key::TAB => Some(Command::InsertText("\t".into())),
        Key::A if ctrl => Some(Command::SelectAll),
        Key::Z if ctrl && mods.shift => Some(Command::Redo),
        Key::Z if ctrl => Some(Command::Undo),
        Key::Y if ctrl => Some(Command::Redo),
        Key::X if ctrl => Some(Command::Cut),
        Key::C if ctrl => Some(Command::Copy),
        Key::V if ctrl => Some(Command::Paste),
        Key::B if ctrl => Some(Command::ToggleBold),
        Key::I if ctrl => Some(Command::ToggleItalic),
        Key::U if ctrl => Some(Command::ToggleUnderline),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Modifiers = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };

    #[test]
    fn arrows_move_and_shift_extends() {
        assert!(matches!(
            command_for(Key::LEFT, SHIFT, TabTarget::Text),
            Some(Command::Move {
                motion: Motion::Left,
                extend: true
            })
        ));
        assert!(matches!(
            command_for(Key::RIGHT, CTRL, TabTarget::Text),
            Some(Command::Move {
                motion: Motion::WordRight,
                extend: false
            })
        ));
        assert!(matches!(
            command_for(Key::HOME, CTRL, TabTarget::Text),
            Some(Command::Move {
                motion: Motion::DocStart,
                ..
            })
        ));
    }

    #[test]
    fn tab_indents_in_a_list_and_types_a_tab_elsewhere() {
        assert!(matches!(
            command_for(Key::TAB, Modifiers::NONE, TabTarget::List),
            Some(Command::Indent)
        ));
        assert!(matches!(
            command_for(Key::TAB, SHIFT, TabTarget::List),
            Some(Command::Outdent)
        ));
        assert!(matches!(
            command_for(Key::TAB, Modifiers::NONE, TabTarget::Text),
            Some(Command::InsertText(t)) if t == "\t"
        ));
        assert!(command_for(Key::TAB, SHIFT, TabTarget::Text).is_none());
    }

    #[test]
    fn tab_moves_between_cells_in_a_table() {
        assert!(matches!(
            command_for(Key::TAB, Modifiers::NONE, TabTarget::Table),
            Some(Command::NextCell)
        ));
        assert!(matches!(
            command_for(Key::TAB, SHIFT, TabTarget::Table),
            Some(Command::PrevCell)
        ));
    }

    #[test]
    fn control_shortcuts() {
        assert!(matches!(
            command_for(Key::Z, CTRL, TabTarget::Text),
            Some(Command::Undo)
        ));
        let redo = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        assert!(matches!(
            command_for(Key::Z, redo, TabTarget::Text),
            Some(Command::Redo)
        ));
        assert!(matches!(
            command_for(Key::B, CTRL, TabTarget::Text),
            Some(Command::ToggleBold)
        ));
        assert!(command_for(Key::B, Modifiers::NONE, TabTarget::Text).is_none());
    }

    #[test]
    fn altgr_is_not_a_shortcut() {
        let altgr = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::NONE
        };
        assert!(command_for(Key::B, altgr, TabTarget::Text).is_none());
    }
}
