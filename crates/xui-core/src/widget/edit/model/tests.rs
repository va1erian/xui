//! Pure unit tests for the edit model: every movement, selection, word
//! boundary and undo rule, with no backend.

use super::*;
use crate::widget::edit::history::MAX_UNDO;
use crate::widget::edit::word::{word_left, word_right};

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

#[test]
fn a_new_model_puts_the_caret_at_the_end() {
    let model = EditModel::new("abc");
    assert_eq!(model.text(), "abc");
    assert_eq!(model.caret(), 3);
    assert_eq!(model.selection(), (3, 3));
    assert!(!model.has_selection());
    assert_eq!(model.selected_text(), "");
}

#[test]
fn an_empty_model_is_well_defined() {
    let mut model = EditModel::new("");
    assert_eq!(model.caret(), 0);
    assert!(!model.backspace());
    assert!(!model.delete());
    assert!(!model.delete_word_back());
    assert!(!model.delete_word_forward());
    assert!(!model.undo());
    assert!(!model.redo());
    model.select_all();
    assert_eq!(model.selection(), (0, 0));
    assert_eq!(model.text(), "");
}

#[test]
fn typing_inserts_at_the_caret_and_moves_it() {
    let mut model = EditModel::new("");
    model.insert_char('a');
    model.insert_char('b');
    assert_eq!(model.text(), "ab");
    assert_eq!(model.caret(), 2);

    model.move_home(false);
    model.insert_char('x');
    assert_eq!(model.text(), "xab");
    assert_eq!(model.caret(), 1);
}

#[test]
fn consecutive_typing_is_one_undo_step() {
    let mut model = EditModel::new("");
    model.insert_char('a');
    model.insert_char('b');
    model.insert_char('c');
    assert_eq!(model.text(), "abc");

    assert!(model.undo());
    assert_eq!(model.text(), "", "the whole typing run is undone");
    assert!(model.redo());
    assert_eq!(model.text(), "abc");
}

#[test]
fn moving_the_caret_breaks_a_typing_run() {
    let mut model = EditModel::new("");
    model.insert_char('a');
    model.move_left(false);
    model.insert_char('b');
    assert_eq!(model.text(), "ba");

    assert!(model.undo());
    assert_eq!(model.text(), "a", "only the second run is undone");
}

#[test]
fn a_new_edit_drops_the_redo_history() {
    let mut model = EditModel::new("");
    model.insert_char('a');
    assert!(model.undo());
    model.insert_char('b');
    assert!(!model.redo(), "a fork discards the redo stack");
    assert_eq!(model.text(), "b");
}

#[test]
fn undo_and_redo_restore_the_selection_too() {
    let mut model = EditModel::new("abc");
    model.select_all();
    model.insert_char('x');
    assert_eq!(model.text(), "x");
    assert!(model.undo());
    assert_eq!(model.text(), "abc");
    assert_eq!(
        model.selection(),
        (0, 3),
        "the selection before typing returns"
    );
}

#[test]
fn backspace_and_delete_remove_one_character() {
    let mut model = EditModel::new("abc");
    assert!(model.backspace());
    assert_eq!(model.text(), "ab");
    model.move_home(false);
    assert!(model.delete());
    assert_eq!(model.text(), "b");
    model.move_end(false);
    assert!(model.backspace());
    assert_eq!(model.text(), "");
    assert!(!model.backspace());
}

#[test]
fn backspace_and_delete_remove_the_selection_first() {
    let mut model = EditModel::new("abcdef");
    model.move_to(1, false);
    model.move_to(4, true);
    assert!(model.backspace());
    assert_eq!(model.text(), "aef");
    assert_eq!(model.caret(), 1);
}

#[test]
fn control_backspace_and_delete_remove_a_word() {
    let mut model = EditModel::new("hello brave world");
    assert!(model.delete_word_back());
    assert_eq!(model.text(), "hello brave ");
    model.move_to(0, false);
    assert!(model.delete_word_forward());
    assert_eq!(model.text(), " brave ", "the space after the word stays");
}

#[test]
fn a_plain_move_collapses_a_selection_to_its_edge() {
    let mut model = EditModel::new("abc");
    model.move_to(0, false);
    model.move_to(2, true);
    assert!(model.has_selection());

    model.move_left(false);
    assert_eq!(model.selection(), (0, 0), "plain Left goes to the start");

    model.move_to(2, true);
    model.move_right(false);
    assert_eq!(model.selection(), (2, 2), "plain Right goes to the end");
}

#[test]
fn shift_moves_extend_the_selection() {
    let mut model = EditModel::new("abcd");
    model.move_home(false);
    model.move_right(true);
    model.move_right(true);
    assert_eq!(model.selection(), (0, 2));
    assert_eq!(model.selected_text(), "ab");

    model.move_end(true);
    assert_eq!(model.selection(), (0, 4));
    model.move_home(false);
    assert_eq!(model.selection(), (0, 0));
}

#[test]
fn home_and_end_jump_to_the_line_ends() {
    let mut model = EditModel::new("abcd");
    model.move_home(false);
    assert_eq!(model.caret(), 0);
    model.move_end(false);
    assert_eq!(model.caret(), 4);
}

#[test]
fn word_moves_cross_whitespace_delimited_runs() {
    let text = "hello brave world";
    let chars = chars(text);
    assert_eq!(word_left(&chars, 17), 12);
    assert_eq!(word_left(&chars, 12), 6);
    assert_eq!(word_left(&chars, 6), 0);
    assert_eq!(word_right(&chars, 0), 5);
    assert_eq!(word_right(&chars, 5), 11, "to the end of `brave`");
    assert_eq!(word_right(&chars, 11), 17, "across the space to `world`");
    assert_eq!(word_right(&chars, 17), 17, "the end is a fixed point");
}

#[test]
fn word_boundaries_treat_punctuation_as_part_of_the_token() {
    let chars = chars("foo, bar");
    assert_eq!(word_right(&chars, 0), 4, "past `foo,`");
    assert_eq!(word_right(&chars, 4), 8, "past the following `bar`");
    assert_eq!(word_left(&chars, 8), 5);
    assert_eq!(word_left(&chars, 5), 0);
}

#[test]
fn word_moves_collapse_a_selection_or_cross_words() {
    let mut model = EditModel::new("hello brave world");
    model.move_to(3, false);
    model.move_to(14, true);
    model.move_word_left(false);
    assert_eq!(
        model.selection(),
        (3, 3),
        "a plain word move collapses first"
    );

    model.move_to(14, false);
    model.move_word_left(false);
    assert_eq!(model.caret(), 12, "then it crosses to the word's start");

    model.move_to(3, false);
    model.move_word_right(false);
    assert_eq!(model.caret(), 5, "right crosses to the word's end");

    model.move_to(3, false);
    model.move_word_right(true);
    assert_eq!(model.selection(), (3, 5), "Shift grows the selection");
}

#[test]
fn select_all_then_typing_replaces_everything() {
    let mut model = EditModel::new("abc");
    model.select_all();
    assert_eq!(model.selected_text(), "abc");
    model.insert_char('z');
    assert_eq!(model.text(), "z");
    assert_eq!(model.selection(), (1, 1));
}

#[test]
fn select_word_at_selects_a_word_or_the_whitespace() {
    let mut model = EditModel::new("hello brave world");
    model.select_word_at(2);
    assert_eq!(model.selected_text(), "hello");
    model.select_word_at(7);
    assert_eq!(model.selected_text(), "brave");
    model.select_word_at(17);
    assert_eq!(model.selected_text(), "world");
    model.select_word_at(5);
    assert_eq!(model.selected_text(), " ", "a click on the gap selects it");
}

#[test]
fn select_word_at_picks_the_whitespace_run_between_words() {
    let mut model = EditModel::new("a  b");
    model.select_word_at(1);
    assert_eq!(model.selected_text(), "  ");
}

#[test]
fn pasting_ignores_control_characters_and_is_one_step() {
    let mut model = EditModel::new("");
    assert!(model.insert_text("a\r\nb"));
    assert_eq!(model.text(), "ab", "the newline is dropped");
    assert!(model.undo());
    assert_eq!(model.text(), "");
    assert!(!model.insert_text("\n"), "nothing to paste");
}

#[test]
fn cut_returns_and_removes_the_selection() {
    let mut model = EditModel::new("abcdef");
    model.move_to(1, false);
    model.move_to(4, true);
    assert_eq!(model.cut().as_deref(), Some("bcd"));
    assert_eq!(model.text(), "aef");
    assert_eq!(model.cut(), None);
}

#[test]
fn the_history_past_cap_drops_the_oldest_step() {
    let mut model = EditModel::new("");
    // Each step is separated by a movement, so none coalesce.
    for _ in 0..(MAX_UNDO + 5) {
        model.insert_char('a');
        model.move_left(false);
        model.move_end(false);
    }
    let mut steps = 0;
    while model.undo() {
        steps += 1;
    }
    assert!(steps <= MAX_UNDO, "the older steps were dropped: {steps}");
}

#[test]
fn caret_positions_are_character_indices_not_bytes() {
    let mut model = EditModel::new("héllo");
    assert_eq!(model.caret(), 5);
    model.move_left(false);
    assert_eq!(model.caret(), 4);
    model.move_home(false);
    model.move_right(true);
    model.move_right(true);
    assert_eq!(model.selected_text(), "hé");
    assert!(model.delete());
    assert_eq!(model.text(), "llo");
}
