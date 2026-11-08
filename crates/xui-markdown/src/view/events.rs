#![forbid(unsafe_code)]

//! Pointer, wheel and keyboard input: scrolling, the scrollbar and links.

use xui_core::backend::{Cursor, Event};
use xui_core::geometry::{Point, Rect};
use xui_core::message::{Key, MouseButton};

use super::state::{State, WHEEL_DIP};

/// What handling an event asks of the widget around it.
#[derive(Debug, Default)]
pub(crate) struct Out {
    pub focus: bool,
    /// Take (`Some(true)`) or give back (`Some(false)`) the mouse capture.
    pub capture: Option<bool>,
    pub invalidate: bool,
    /// The layout was dropped (a new size): lay the document out again.
    pub relayout: bool,
    pub cursor: Option<Cursor>,
    /// A link was clicked: its destination ([`Document::link`]).
    pub link: Option<String>,
}

/// Handles `event`, or returns `None` when the view has no use for it.
pub(crate) fn handle(state: &mut State, event: &Event) -> Option<Out> {
    let mut out = Out::default();
    match *event {
        Event::Resize { width, height } => {
            let dpi = state.dpi;
            state.fit(Rect::new(0, 0, width, height), dpi);
            out.invalidate = true;
            out.relayout = true;
        }
        Event::SetFocus => state.focused = true,
        Event::KillFocus => state.focused = false,
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => press(state, Point::new(x, y), &mut out),
        Event::MouseMove { x, y, .. } => {
            if state.drag_bar(y) {
                out.invalidate = true;
            } else {
                hover(state, Point::new(x, y), &mut out);
            }
        }
        Event::MouseUp {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => release(state, Point::new(x, y), &mut out),
        Event::CaptureChanged => {
            state.bar_drag = None;
            state.pressed = None;
            out.invalidate = true;
        }
        Event::MouseWheel {
            delta,
            horizontal: false,
            x,
            y,
            ..
        } => {
            let step = f32::from(delta) / 120.0 * WHEEL_DIP * state.dpi as f32 / 96.0;
            state.scroll_by(-step);
            // The text moved under a still pointer: a link may have come or
            // gone there.
            hover(state, Point::new(x, y), &mut out);
            out.invalidate = true;
        }
        Event::KeyDown {
            key, system: false, ..
        } if state.focused => {
            let line = WHEEL_DIP / 2.0 * state.dpi as f32 / 96.0;
            let page = state.viewport() * 0.9;
            match key {
                Key::UP => state.scroll_by(-line),
                Key::DOWN => state.scroll_by(line),
                Key::PAGE_UP => state.scroll_by(-page),
                Key::PAGE_DOWN | Key::SPACE => state.scroll_by(page),
                Key::HOME => state.set_scroll(0.0),
                Key::END => state.scroll_to_end(),
                _ => return None,
            }
            out.invalidate = true;
        }
        _ => return None,
    }
    Some(out)
}

fn press(state: &mut State, point: Point, out: &mut Out) {
    out.focus = true;
    if state.on_bar(point) {
        if state.press_bar(point.y) {
            out.capture = Some(true);
        }
        out.invalidate = true;
        return;
    }
    state.pressed = state.link_at(point);
    if state.pressed.is_some() {
        out.capture = Some(true);
    }
}

fn hover(state: &mut State, point: Point, out: &mut Out) {
    let link = state.link_at(point);
    if link.is_some() != state.hover.is_some() {
        out.cursor = Some(if link.is_some() {
            Cursor::Hand
        } else {
            Cursor::Default
        });
    }
    state.hover = link;
}

fn release(state: &mut State, point: Point, out: &mut Out) {
    if state.bar_drag.take().is_some() {
        out.capture = Some(false);
        out.invalidate = true;
        return;
    }
    let Some(pressed) = state.pressed.take() else {
        return;
    };
    out.capture = Some(false);
    if state.link_at(point) == Some(pressed) {
        out.link = Some(state.layout.doc().link(pressed).to_owned());
    }
}
