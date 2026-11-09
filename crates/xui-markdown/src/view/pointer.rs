#![forbid(unsafe_code)]

//! The pointer: dragging a selection, double- and triple-click selection,
//! following links, the scrollbar and the pointer's shape.
//!
//! A press on a link either follows it (released on the same link without
//! moving) or starts a selection (moved more than a few pixels first), so
//! link text can be selected like any other.

use std::time::{Duration, Instant};

use xui_core::Dip;
use xui_core::backend::Cursor;
use xui_core::geometry::Point;

use super::events::Out;
use super::selection::{Selection, block_text, word_at};
use super::state::State;

/// How far the pointer may move and still click.
const SLOP: Dip = Dip(4.0);
/// How soon after a double-click a press makes it a triple-click.
const TRIPLE: Duration = Duration::from_millis(500);

fn near(state: &State, a: Point, b: Point) -> bool {
    let slop = SLOP.to_px(state.dpi).value();
    (a.x - b.x).abs() <= slop && (a.y - b.y).abs() <= slop
}

pub(crate) fn press(state: &mut State, point: Point, shift: bool, out: &mut Out) {
    out.focus = true;
    out.invalidate = true;
    if state.on_bar(point) {
        if state.press_bar(point.y) {
            out.capture = Some(true);
        }
        return;
    }
    let Some((block, pos)) = state.hit(point) else {
        return;
    };
    if let Some((when, at)) = state.last_double.take()
        && when.elapsed() <= TRIPLE
        && near(state, at, point)
    {
        state.selection = Selection::of(block_text(&state.layout, block));
        state.anchored = true;
        return;
    }
    state.pressed = state.link_at(point);
    state.press_at = Some(point);
    // Shift extends a selection from its anchor; with none yet it starts one.
    if shift && state.anchored {
        state.selection.focus = pos;
    } else {
        state.selection = Selection::at(pos);
    }
    state.anchored = true;
    state.dragging = true;
    out.capture = Some(true);
}

pub(crate) fn double_click(state: &mut State, point: Point, out: &mut Out) {
    // On the bar the second press of a quick pair is a press like the first:
    // it grabs the thumb or pages, so a drag right after a click follows.
    if state.on_bar(point) {
        press(state, point, false, out);
        return;
    }
    let Some((block, pos)) = state.hit(point) else {
        return;
    };
    // An image is one character: a double-click on it selects all of it.
    let word = state
        .image_at(point)
        .unwrap_or_else(|| word_at(&state.layout, block, pos));
    state.selection = Selection::of(word);
    state.anchored = true;
    state.last_double = Some((Instant::now(), point));
    state.pressed = None;
    state.dragging = false;
    out.invalidate = true;
}

pub(crate) fn moved(state: &mut State, point: Point, out: &mut Out) {
    if state.drag_bar(point.y) {
        out.invalidate = true;
        return;
    }
    if state.dragging {
        if state.pressed.is_some() && state.press_at.is_some_and(|at| near(state, at, point)) {
            return;
        }
        state.pressed = None;
        state.drag_point = Some(point);
        // Past the top or bottom edge, the view scrolls toward the pointer,
        // and goes on scrolling on a timer while the pointer is held there.
        out.autoscroll = autoscroll(state);
        state.selection.focus = state.pos_at(point);
        out.invalidate = true;
        return;
    }
    hover(state, point, out);
}

/// One step of scrolling a drag held past the view's top or bottom edge, by
/// how far past it the pointer is; extends the selection to the pointer.
/// Returns whether the pointer is past an edge, so scrolling goes on.
pub(crate) fn autoscroll(state: &mut State) -> bool {
    let Some(point) = state.drag_point.filter(|_| state.dragging) else {
        return false;
    };
    let over = if point.y < state.bounds.top {
        point.y - state.bounds.top
    } else if point.y > state.bounds.bottom {
        point.y - state.bounds.bottom
    } else {
        0
    };
    if over == 0 {
        return false;
    }
    state.scroll_by(over as f32);
    state.selection.focus = state.pos_at(point);
    true
}

/// Sets the pointer's shape for what is under it.
fn hover(state: &mut State, point: Point, out: &mut Out) {
    state.hover = state.link_at(point);
    let cursor = if state.hover.is_some() {
        Cursor::Hand
    } else if state.on_bar(point) {
        Cursor::Default
    } else {
        Cursor::Text
    };
    if cursor != state.cursor {
        state.cursor = cursor;
        out.cursor = Some(cursor);
    }
}

pub(crate) fn release(state: &mut State, point: Point, out: &mut Out) {
    if state.bar_drag.take().is_some() {
        out.capture = Some(false);
        out.invalidate = true;
        return;
    }
    if !state.dragging {
        return;
    }
    state.dragging = false;
    state.drag_point = None;
    out.capture = Some(false);
    if let Some(pressed) = state.pressed.take()
        && state.link_at(point) == Some(pressed)
    {
        state.clear_selection();
        out.link = Some(state.layout.doc().link(pressed).to_owned());
        out.invalidate = true;
    }
}
