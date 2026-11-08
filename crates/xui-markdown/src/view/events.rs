#![forbid(unsafe_code)]

//! Input: the pointer (in [`pointer`](super::pointer)), the wheel and the
//! keyboard, mapped to scrolling, selection, copying and links.

use xui_core::backend::{Cursor, Event};
use xui_core::geometry::{Point, Rect};
use xui_core::message::{Key, MouseButton};

use super::pointer;
use super::selection::Selection;
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
    /// A link was clicked: its destination ([`Document::link`](crate::Document::link)).
    pub link: Option<String>,
    /// Put the selected text on the clipboard.
    pub copy: bool,
    /// The context menu was asked for at this client point.
    pub context: Option<Point>,
    /// A drag is held past an edge: keep scrolling on a timer.
    pub autoscroll: bool,
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
        Event::SetFocus | Event::KillFocus => {
            // The selection's colour follows the focus.
            state.focused = matches!(event, Event::SetFocus);
            out.invalidate = !state.selection.is_empty();
        }
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers,
        } => pointer::press(state, Point::new(x, y), modifiers.shift, &mut out),
        Event::MouseDoubleClick {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => pointer::double_click(state, Point::new(x, y), &mut out),
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Right,
            ..
        } => {
            let point = Point::new(x, y);
            out.focus = true;
            if !state.on_bar(point) {
                out.context = Some(point);
            }
        }
        Event::MouseMove { x, y, .. } => pointer::moved(state, Point::new(x, y), &mut out),
        Event::MouseUp {
            x,
            y,
            button: MouseButton::Left,
            ..
        } => pointer::release(state, Point::new(x, y), &mut out),
        Event::CaptureChanged => {
            state.bar_drag = None;
            state.pressed = None;
            state.dragging = false;
            state.drag_point = None;
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
            // gone there, and a drag in progress follows it.
            pointer::moved(state, Point::new(x, y), &mut out);
            out.invalidate = true;
        }
        Event::KeyDown {
            key,
            modifiers,
            system: false,
            ..
        } if state.focused => {
            let line = WHEEL_DIP / 2.0 * state.dpi as f32 / 96.0;
            let page = state.viewport() * 0.9;
            match key {
                Key::A if modifiers.ctrl => {
                    let end = state.layout.doc().text().len() as u32;
                    state.selection = Selection::of(0..end);
                    state.anchored = true;
                }
                Key::C | Key::INSERT if modifiers.ctrl => {
                    out.copy = !state.selection.is_empty();
                    return Some(out);
                }
                Key::ESCAPE if !state.selection.is_empty() => state.clear_selection(),
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
