#![forbid(unsafe_code)]

//! Turning backend events into editor commands and view changes.

use xui_core::backend::{Cursor, Event};
use xui_core::geometry::{Point, Rect};
use xui_core::message::{Key, MouseButton};

use super::keys::{TabTarget, command_for};
use super::state::State;
use crate::edit::{Command, Effect};

/// How far one wheel notch scrolls, in design units.
const WHEEL_DIP: f32 = 48.0;

/// What handling an event asks of the widget around it.
#[derive(Debug, Default)]
pub(crate) struct Out {
    /// Take the keyboard focus.
    pub focus: bool,
    /// Take (`Some(true)`) or give back (`Some(false)`) the mouse capture.
    pub capture: Option<bool>,
    /// Repaint.
    pub invalidate: bool,
    /// The document changed.
    pub changed: bool,
    /// The selection or its formatting changed.
    pub selection: bool,
    /// A link was Ctrl+clicked.
    pub link: Option<String>,
    /// The mouse cursor to switch to.
    pub cursor: Option<Cursor>,
}

impl Out {
    /// Records what a command changed.
    pub fn absorb(&mut self, effect: &Effect) {
        self.changed |= effect.doc_changed();
        self.selection |= effect.selection || effect.doc_changed();
        self.invalidate |= !effect.is_none();
    }
}

/// Handles `event`, or returns `None` when the editor has no use for it.
pub(crate) fn handle(state: &mut State, event: &Event) -> Option<Out> {
    let mut out = Out::default();
    match *event {
        Event::Resize { width, height } => {
            state.bounds = Rect::new(0, 0, width, height);
            out.invalidate = true;
        }
        Event::SetFocus => {
            state.focused = true;
            state.caret_on = true;
            out.invalidate = true;
        }
        Event::KillFocus => {
            state.focused = false;
            out.invalidate = true;
        }
        Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            modifiers,
        } => state.press(Point::new(x, y), modifiers, false, &mut out),
        Event::MouseDoubleClick {
            x,
            y,
            button: MouseButton::Left,
            modifiers,
        } => state.press(Point::new(x, y), modifiers, true, &mut out),
        Event::MouseMove { x, y, modifiers } => {
            state.pointer_moved(Point::new(x, y), modifiers, &mut out);
        }
        Event::MouseUp {
            button: MouseButton::Left,
            ..
        } => state.released(&mut out),
        Event::CaptureChanged => state.capture_lost(&mut out),
        Event::MouseWheel {
            delta,
            horizontal: false,
            ..
        } => {
            state.ready();
            let step = f32::from(delta) / 120.0 * WHEEL_DIP * state.dpi as f32 / 96.0;
            state.scroll_by(-step);
            out.invalidate = true;
        }
        Event::KeyDown {
            key,
            modifiers,
            system: false,
            ..
        } if state.focused => {
            if key == Key::ESCAPE {
                if state.cancel_drag() {
                    out.capture = Some(false);
                    out.invalidate = true;
                }
                return Some(out);
            }
            let tab = if state.ed.in_table() {
                TabTarget::Table
            } else if state.ed.in_list() {
                TabTarget::List
            } else {
                TabTarget::Text
            };
            let command = command_for(key, modifiers, tab)?;
            if state.drag.is_some() {
                return Some(out);
            }
            out.absorb(&state.run(command));
        }
        Event::Char(c) if state.focused && !c.is_control() && state.drag.is_none() => {
            out.absorb(&state.run(Command::InsertText(c.to_string())));
        }
        _ => return None,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_effect_that_changes_the_document_changes_the_selection_too() {
        let mut out = Out::default();
        out.absorb(&Effect {
            dirty: Some(0..1),
            ..Effect::NONE
        });
        assert!(out.changed && out.selection && out.invalidate);
    }

    #[test]
    fn nothing_changed_asks_for_nothing() {
        let mut out = Out::default();
        out.absorb(&Effect::NONE);
        assert!(!out.invalidate && !out.changed && !out.selection);
    }
}
