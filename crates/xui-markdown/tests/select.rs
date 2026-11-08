//! Selecting, copying and the context menu, driven headlessly.
//!
//! The offscreen backend keeps an in-process clipboard, so a copy is checked
//! on the clipboard itself. It has no pointer shape to read back, so the
//! cursor is not tested here.

#[path = "view/rig.rs"]
mod rig;

use rig::*;
use xui_canvas::snapshot::Stage;
use xui_core::message::Key;

const TWO: &str = "alpha beta\n\ngamma delta";
/// Y of the first and second paragraph's line in [`TWO`].
const FIRST: i32 = 24;
const SECOND: i32 = 54;
const RIGHT: i32 = WIDTH - 10;

#[test]
fn dragging_across_two_paragraphs_selects_both_with_a_blank_line_between() {
    run(TWO, |stage, rig| {
        assert!(!rig.view.has_selection());
        drag(stage, (0, FIRST), (RIGHT, SECOND));
        assert!(rig.view.has_selection());
        assert_eq!(rig.view.selected_text(), "alpha beta\n\ngamma delta");
    });
}

#[test]
fn a_click_without_a_drag_selects_nothing() {
    run(TWO, |stage, rig| {
        stage.click(40, FIRST);
        assert!(!rig.view.has_selection());
        assert_eq!(rig.view.selected_text(), "");
    });
}

#[test]
fn shift_click_extends_the_selection_from_its_anchor() {
    run(TWO, |stage, rig| {
        stage.click(0, FIRST);
        assert!(!rig.view.has_selection());
        shift_click(stage, 300, SECOND);
        assert_eq!(rig.view.selected_text(), "alpha beta\n\ngamma delta");
        // Shift-clicking back on the first line keeps the same anchor.
        shift_click(stage, 300, FIRST);
        assert_eq!(rig.view.selected_text(), "alpha beta");
    });
}

#[test]
fn double_click_selects_the_word_under_the_pointer() {
    run(TWO, |stage, rig| {
        double_click(stage, 20, FIRST);
        assert_eq!(rig.view.selected_text(), "alpha");
        double_click(stage, 20, SECOND);
        assert_eq!(rig.view.selected_text(), "gamma");
    });
}

#[test]
fn a_press_just_after_a_double_click_selects_the_whole_block() {
    run(TWO, |stage, rig| {
        double_click(stage, 20, FIRST);
        assert_eq!(rig.view.selected_text(), "alpha");
        mouse(stage, "down", 20, FIRST);
        mouse(stage, "up", 20, FIRST);
        assert_eq!(rig.view.selected_text(), "alpha beta");
    });
}

#[test]
fn a_press_elsewhere_after_a_double_click_is_an_ordinary_press() {
    run(TWO, |stage, rig| {
        double_click(stage, 20, FIRST);
        mouse(stage, "down", 20, SECOND);
        mouse(stage, "up", 20, SECOND);
        assert!(!rig.view.has_selection());
    });
}

const LINK: &str = "see [the link](next.md) here\n\nsecond";

/// Where the link in [`LINK`] is, found by clicking until it is followed;
/// the log and the selection are left empty.
fn link_at(stage: &Stage<'_, String>, rig: &Rig) -> (i32, i32) {
    let at = find(stage, || clicked_next(rig)).expect("the link is on screen");
    rig.log.borrow_mut().clear();
    rig.view.clear_selection();
    at
}

#[test]
fn pressing_on_a_link_and_dragging_away_selects_text_and_follows_nothing() {
    run(LINK, |stage, rig| {
        let at = link_at(stage, rig);
        drag(stage, at, (RIGHT, at.1));
        assert!(rig.view.has_selection());
        assert!(rig.view.selected_text().contains("link"));
        assert!(rig.log.borrow().is_empty(), "{:?}", rig.log.borrow());
    });
}

#[test]
fn pressing_and_releasing_on_a_link_follows_it_and_clears_the_selection() {
    run(LINK, |stage, rig| {
        let at = link_at(stage, rig);
        drag(stage, (0, at.1), (RIGHT, at.1));
        assert!(rig.view.has_selection());
        stage.click(at.0, at.1);
        assert_eq!(rig.log.borrow().as_slice(), ["next.md"]);
        assert!(!rig.view.has_selection());
    });
}

#[test]
fn a_small_wobble_on_a_link_still_follows_it() {
    run(LINK, |stage, rig| {
        let at = link_at(stage, rig);
        mouse(stage, "down", at.0, at.1);
        mouse(stage, "move", at.0 + 1, at.1);
        mouse(stage, "up", at.0 + 1, at.1);
        assert_eq!(rig.log.borrow().as_slice(), ["next.md"]);
        assert!(!rig.view.has_selection());
    });
}

#[test]
fn ctrl_a_then_ctrl_c_copies_the_whole_document() {
    run(TWO, |stage, rig| {
        focus(stage, rig);
        ctrl(stage, Key::A);
        assert_eq!(rig.view.selected_text(), "alpha beta\n\ngamma delta");
        assert_eq!(clipboard(stage), None, "selecting does not copy");
        ctrl(stage, Key::C);
        assert_eq!(
            clipboard(stage).as_deref(),
            Some("alpha beta\n\ngamma delta")
        );
    });
}

#[test]
fn ctrl_insert_copies_too() {
    run(TWO, |stage, rig| {
        focus(stage, rig);
        ctrl(stage, Key::A);
        ctrl(stage, Key::INSERT);
        assert_eq!(
            clipboard(stage).as_deref(),
            Some("alpha beta\n\ngamma delta")
        );
    });
}

#[test]
fn ctrl_c_with_nothing_selected_leaves_the_clipboard_alone() {
    run(TWO, |stage, rig| {
        stage.ui().set_clipboard_text("before");
        focus(stage, rig);
        ctrl(stage, Key::C);
        assert_eq!(clipboard(stage).as_deref(), Some("before"));
    });
}

#[test]
fn keys_do_nothing_without_focus() {
    run(TWO, |stage, rig| {
        ctrl(stage, Key::A);
        assert!(!rig.view.has_selection());
    });
}

#[test]
fn escape_clears_the_selection() {
    run(TWO, |stage, rig| {
        focus(stage, rig);
        ctrl(stage, Key::A);
        assert!(rig.view.has_selection());
        key(stage, Key::ESCAPE);
        assert!(!rig.view.has_selection());
    });
}

#[test]
fn a_right_click_delivers_the_context_point_and_keeps_the_selection() {
    run(TWO, |stage, rig| {
        drag(stage, (0, FIRST), (RIGHT, FIRST));
        let before = rig.view.selected_text();
        assert_eq!(before, "alpha beta");
        right_click(stage, 50, SECOND);
        assert_eq!(
            rig.log.borrow().as_slice(),
            [format!("context:50,{SECOND}")]
        );
        assert_eq!(rig.view.selected_text(), before);
    });
}

#[test]
fn the_api_selects_clears_and_copies() {
    run(TWO, |stage, rig| {
        assert!(!rig.view.copy(), "nothing selected, nothing copied");
        assert_eq!(clipboard(stage), None);
        rig.view.select_all();
        assert!(rig.view.has_selection());
        assert_eq!(rig.view.selected_text(), "alpha beta\n\ngamma delta");
        assert!(rig.view.copy());
        assert_eq!(
            clipboard(stage).as_deref(),
            Some("alpha beta\n\ngamma delta")
        );
        rig.view.clear_selection();
        assert!(!rig.view.has_selection());
        assert_eq!(rig.view.selected_text(), "");
        assert!(!rig.view.copy());
    });
}

#[test]
fn set_markdown_clears_the_selection() {
    run(TWO, |_stage, rig| {
        rig.view.select_all();
        assert!(rig.view.has_selection());
        rig.view.set_markdown("something else");
        assert!(!rig.view.has_selection());
        assert_eq!(rig.view.selected_text(), "");
    });
}

fn long() -> String {
    (0..60)
        .map(|i| format!("paragraph number {i}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[test]
fn dragging_below_the_bottom_edge_scrolls_and_extends_the_selection() {
    run(&long(), |stage, rig| {
        assert_eq!(rig.view.scroll(), 0.0);
        mouse(stage, "down", 0, FIRST);
        mouse(stage, "move", 100, HEIGHT - 10);
        let inside = rig.view.scroll();
        mouse(stage, "move", 100, HEIGHT + 40);
        mouse(stage, "move", 100, HEIGHT + 40);
        let scrolled = rig.view.scroll();
        assert!(scrolled > inside, "{scrolled} vs {inside}");
        mouse(stage, "up", 100, HEIGHT + 40);
        let text = rig.view.selected_text();
        assert!(text.starts_with("paragraph number 0\n\n"), "{text:?}");
        assert!(text.lines().count() > 10, "{text:?}");
    });
}

#[test]
fn dragging_above_the_top_edge_scrolls_back_up() {
    run_raw(&long(), |stage, rig| {
        rig.view.set_scroll(400.0);
        paint(stage);
        let from = rig.view.scroll();
        assert!(from > 0.0);
        mouse(stage, "down", 0, FIRST);
        mouse(stage, "move", 100, -30);
        mouse(stage, "move", 100, -30);
        assert!(rig.view.scroll() < from);
        mouse(stage, "up", 100, -30);
        assert!(rig.view.has_selection());
    });
}

#[test]
fn code_with_a_tab_is_copied_as_written() {
    run("```\nx\ty\n```", |stage, rig| {
        drag(stage, (0, 28), (RIGHT, 28));
        assert_eq!(rig.view.selected_text(), "x\ty");
    });
}

#[test]
fn a_list_is_copied_with_its_markers() {
    run("- one\n- two\n- three", |stage, rig| {
        drag(stage, (0, 20), (RIGHT, 100));
        assert_eq!(rig.view.selected_text(), "- one\n- two\n- three");
    });
}

#[test]
fn an_ordered_list_is_copied_with_its_numbers() {
    run("1. one\n2. two", |_stage, rig| {
        rig.view.select_all();
        assert_eq!(rig.view.selected_text(), "1. one\n2. two");
    });
}

#[test]
fn a_table_is_copied_as_tab_separated_rows() {
    run("| a | b |\n|---|---|\n| c | d |", |stage, rig| {
        drag(stage, (0, 18), (RIGHT, 80));
        assert_eq!(rig.view.selected_text(), "a\tb\nc\td");
        focus(stage, rig);
        ctrl(stage, Key::C);
        assert_eq!(clipboard(stage).as_deref(), Some("a\tb\nc\td"));
    });
}
