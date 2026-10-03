//! Caret motion and selection commands.

#[path = "edit_common.rs"]
mod harness;

use harness::Ed;
use xui_rich_text::DocPos;
use xui_rich_text::edit::{Command, Motion};
use xui_rich_text::model::Selection;

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
