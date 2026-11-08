//! Selection at the edges: an empty view, where two blocks meet, a lost
//! capture and the triple-click time limit.

#[path = "view/rig.rs"]
mod rig;

use std::time::Duration;

use rig::*;
use xui_core::backend::Event;

const LIST: &str = "- one\n- two words\n- three";
/// Where a list item's text starts: padding plus one list indent.
const TEXT_LEFT: i32 = 16 + 24;

/// The y of the line whose word double-clicked at `x` is `word`.
fn line_of(stage: &xui_canvas::snapshot::Stage<'_, String>, rig: &Rig, x: i32, word: &str) -> i32 {
    (10..HEIGHT)
        .step_by(2)
        .find(|&y| {
            double_click(stage, x, y);
            rig.view.selected_text() == word
        })
        .unwrap_or_else(|| panic!("no line shows {word:?}"))
}

#[test]
fn double_and_triple_clicks_in_an_empty_view_select_nothing() {
    run("", |stage, rig| {
        double_click(stage, 100, 50);
        assert!(!rig.view.has_selection());
        stage.click(100, 50);
        assert!(!rig.view.has_selection());
        drag(stage, (20, 20), (200, 120));
        assert_eq!(rig.view.selected_text(), "");
    });
}

#[test]
fn a_word_at_the_start_of_a_block_belongs_to_that_block() {
    // "one" ends at the very offset "two" starts at: a click on the left
    // edge of "two" must select "two", not the "one" before it.
    run(LIST, |stage, rig| {
        let y = line_of(stage, rig, TEXT_LEFT + 10, "two");
        rig.view.clear_selection();
        double_click(stage, TEXT_LEFT + 1, y);
        assert_eq!(rig.view.selected_text(), "two");
        stage.click(TEXT_LEFT + 1, y);
        assert_eq!(
            rig.view.selected_text(),
            "two words",
            "a triple-click takes the item"
        );
    });
}

#[test]
fn a_press_after_the_double_click_time_is_an_ordinary_press() {
    run(LIST, |stage, rig| {
        let y = line_of(stage, rig, TEXT_LEFT + 10, "two");
        std::thread::sleep(Duration::from_millis(600));
        stage.click(TEXT_LEFT + 10, y);
        assert!(!rig.view.has_selection(), "{:?}", rig.view.selected_text());
    });
}

#[test]
fn losing_the_capture_ends_a_drag() {
    run(LIST, |stage, rig| {
        mouse(stage, "down", TEXT_LEFT, 20);
        mouse(stage, "move", TEXT_LEFT + 20, 20);
        let before = rig.view.selected_text();
        stage.inject(Event::CaptureChanged);
        paint(stage);
        mouse(stage, "move", WIDTH - 20, HEIGHT - 20);
        assert_eq!(rig.view.selected_text(), before, "the drag went on");
    });
}

#[test]
fn shift_click_with_nothing_selected_starts_at_the_click() {
    run(LIST, |stage, rig| {
        shift_click(stage, TEXT_LEFT + 10, 20);
        assert!(!rig.view.has_selection(), "{:?}", rig.view.selected_text());
    });
}

#[test]
fn shift_click_after_escape_starts_a_new_selection() {
    run(LIST, |stage, rig| {
        focus(stage, rig);
        drag(stage, (TEXT_LEFT, 20), (TEXT_LEFT + 30, 20));
        assert!(rig.view.has_selection());
        key(stage, xui_core::message::Key::ESCAPE);
        assert!(!rig.view.has_selection());
        shift_click(stage, TEXT_LEFT + 10, 20);
        assert!(!rig.view.has_selection(), "{:?}", rig.view.selected_text());
        rig.view.select_all();
        rig.view.clear_selection();
        shift_click(stage, TEXT_LEFT + 10, 20);
        assert!(!rig.view.has_selection(), "{:?}", rig.view.selected_text());
    });
}
