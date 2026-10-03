#![forbid(unsafe_code)]

//! The view state driven by hand: the timer tick (caret blink, autoscroll,
//! finishing the layout) and the first layout of a large document.

use xui_core::backend::Backend;
use xui_core::geometry::{Point, Rect};

use super::drag::Drag;
use super::state::State;
use super::timer::BLINK_TICKS;
use crate::edit::{Command, MemoryClipboard};
use crate::model::Document;

const VIEW: Rect = Rect::new(0, 0, 400, 300);

fn state_with(paragraphs: usize) -> State {
    let shaper = xui_canvas::OffscreenBackend::new().text_shaper();
    let mut state = State::new(shaper, Box::new(MemoryClipboard::default()), VIEW, 96);
    let text: Vec<String> = (0..paragraphs)
        .map(|i| format!("{i}: the quick brown fox jumps over the lazy dog, again and again"))
        .collect();
    state.set_document(Document::from_plain_text(&text.join("\n")));
    state.prepare(VIEW, 96);
    state
}

fn laid_out(state: &State) -> usize {
    state
        .layout
        .paragraphs()
        .iter()
        .filter(|p| !p.dirty)
        .count()
}

#[test]
fn the_caret_blinks_only_while_focused() {
    let mut state = state_with(3);
    assert!(state.caret_on);
    for _ in 0..BLINK_TICKS * 3 {
        assert!(
            !state.tick() || !state.focused,
            "an unfocused editor does not repaint"
        );
    }
    assert!(state.caret_on, "unfocused: the phase never changes");

    state.focused = true;
    let mut phases = Vec::new();
    for _ in 0..BLINK_TICKS * 4 {
        if state.tick() {
            phases.push(state.caret_on);
        }
    }
    assert_eq!(
        phases,
        [false, true, false, true],
        "one flip per blink period"
    );
}

#[test]
fn an_edit_makes_the_caret_visible_again() {
    let mut state = state_with(3);
    state.focused = true;
    for _ in 0..BLINK_TICKS {
        state.tick();
    }
    assert!(!state.caret_on);
    state.run(Command::InsertText("x".into()));
    assert!(state.caret_on);
}

#[test]
fn dragging_below_the_view_scrolls_down_and_extends_the_selection() {
    let mut state = state_with(60);
    state.run(Command::SetCaret {
        pos: crate::model::DocPos::new(0, 0),
        extend: false,
    });
    state.drag = Some(Drag::Select(super::drag::Unit::Char));
    state.pointer = Point::new(50, 290);
    assert!(!state.tick(), "inside the view nothing scrolls");
    assert_eq!(state.scroll, 0.0);

    state.pointer = Point::new(50, 340);
    let mut last = state.scroll;
    for _ in 0..3 {
        assert!(state.tick());
        assert!(state.scroll > last, "scrolls on every tick");
        last = state.scroll;
    }
    let head = state.ed.selection.head().expect("a text selection");
    assert!(
        head.para > 0,
        "the selection followed the pointer: {head:?}"
    );
}

#[test]
fn dragging_above_the_view_scrolls_up_and_stops_at_the_top() {
    let mut state = state_with(60);
    state.scroll = 200.0;
    state.drag = Some(Drag::Select(super::drag::Unit::Char));
    state.pointer = Point::new(50, -30);
    assert!(state.tick());
    assert!(state.scroll < 200.0);
    for _ in 0..100 {
        state.tick();
    }
    assert_eq!(state.scroll, 0.0);
    assert!(!state.tick(), "no change at the top");
}

#[test]
fn a_large_document_is_laid_out_only_around_the_view_at_first() {
    let state = state_with(2000);
    let laid = laid_out(&state);
    assert!((1..=128).contains(&laid), "{laid} paragraphs laid out");
    assert!(
        state.content_height() > 2000.0 * 18.0,
        "the rest is estimated"
    );
}

#[test]
fn the_timer_finishes_the_layout_and_keeps_the_view_in_place() {
    let mut state = state_with(2000);
    // Scroll far down: those paragraphs are laid out against estimates.
    state.scroll = 20_000.0;
    state.prepare(VIEW, 96);
    let shift = state.shift() as f32;
    let top = state.layout.visible(shift, shift + 1.0).start;
    let on_screen = |s: &State| s.layout.paragraphs()[top].y - s.shift() as f32;
    let before = on_screen(&state);

    let mut ticks = 0;
    while !state.layout.is_complete() {
        assert!(state.tick());
        let moved = on_screen(&state);
        assert!(
            (moved - before).abs() < 0.51,
            "the top paragraph stayed at {before}: {moved}"
        );
        ticks += 1;
        assert!(ticks < 200, "layout must finish");
    }
    assert_eq!(laid_out(&state), 2000);

    let mut whole = state_with(2000);
    while !whole.layout.is_complete() {
        whole.tick();
    }
    assert!((whole.layout.height() - state.layout.height()).abs() < 0.01);
}

#[test]
fn a_command_far_from_the_view_lays_out_its_target() {
    let mut state = state_with(2000);
    state.run(Command::Move {
        motion: crate::edit::Motion::DocEnd,
        extend: false,
    });
    let last = state.layout.paragraphs().last().expect("paragraphs");
    assert!(
        !last.dirty && !last.lines.is_empty(),
        "the caret's paragraph is real"
    );
    let caret = state
        .layout
        .caret_rect(&state.ed.doc, state.ed.selection.head().unwrap());
    let view = state.to_view(caret);
    assert!(
        view.top >= 0 && view.bottom <= 300,
        "scrolled to the caret: {view:?}"
    );
    assert!(laid_out(&state) <= 300);
}
