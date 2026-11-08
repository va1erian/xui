#![forbid(unsafe_code)]

//! Keyboard input: key presses, typed characters and Tab traversal.
//!
//! Windows' virtual-key codes are the portable [`Key`] codes, so a press maps
//! straight through.

use xui_core::backend::{Event, WidgetId, WindowId};
use xui_core::message::Key;

use super::Shared;
use crate::sys::window::{self, Msg};

impl Shared {
    /// Where keys go: the focused node of `window`, or no node.
    fn key_target(&self, window: WindowId) -> WidgetId {
        self.focus_holder(window).unwrap_or(WidgetId::NONE)
    }

    pub(super) fn key_down(&self, window: WindowId, msg: Msg, system: bool) -> Option<isize> {
        let key = Key::from_code(msg.wparam as u16);
        let modifiers = window::modifiers();
        if key == Key::ESCAPE
            && self
                .locked
                .get()
                .is_some_and(|id| self.window_of(id) == Some(window))
        {
            self.unlock_pointer(true);
            return Some(0);
        }
        let target = self.key_target(window);
        if key == Key::TAB && !modifiers.ctrl && !modifiers.alt && !self.wants_tab(target) {
            self.move_focus(window, target, modifiers.shift);
            return Some(0);
        }
        let event = Event::KeyDown {
            key,
            modifiers,
            repeat: repeat_of(msg),
            system,
        };
        // An unhandled system key (Alt+F4, Alt+Space) is left to Windows.
        let handled = self.deliver(window, target, &event);
        (handled || !system).then_some(0)
    }

    pub(super) fn key_up(&self, window: WindowId, msg: Msg, system: bool) -> Option<isize> {
        let event = Event::KeyUp {
            key: Key::from_code(msg.wparam as u16),
            modifiers: window::modifiers(),
            system,
        };
        let handled = self.deliver(window, self.key_target(window), &event);
        (handled || !system).then_some(0)
    }

    /// A `WM_CHAR` unit. A UTF-16 surrogate pair arrives as two messages, so
    /// the high half waits for its low half. Control characters are keys, not
    /// text.
    pub(super) fn character(&self, window: WindowId, unit: u16) {
        let character = if (0xD800..0xDC00).contains(&unit) {
            self.high_surrogate.set(unit);
            return;
        } else if (0xDC00..0xE000).contains(&unit) {
            let high = self.high_surrogate.replace(0);
            char::decode_utf16([high, unit]).next().and_then(Result::ok)
        } else {
            char::from_u32(u32::from(unit))
        };
        if let Some(character) = character.filter(|c| !c.is_control()) {
            self.deliver(window, self.key_target(window), &Event::Char(character));
        }
    }

    fn wants_tab(&self, target: WidgetId) -> bool {
        self.scene
            .borrow()
            .get(target)
            .is_some_and(|node| node.wants_tab)
    }

    /// Moves the focus to the next (or previous) tab stop of `window`,
    /// wrapping at the ends.
    fn move_focus(&self, window: WindowId, from: WidgetId, backwards: bool) {
        let stops: Vec<WidgetId> = {
            let scene = self.scene.borrow();
            scene
                .iter()
                .filter(|(id, node)| {
                    node.window == window
                        && node.tab_stop
                        && node.enabled
                        && scene.effectively_visible(*id)
                })
                .map(|(id, _)| id)
                .collect()
        };
        if stops.is_empty() {
            return;
        }
        let at = stops.iter().position(|id| *id == from);
        let next = match (at, backwards) {
            (Some(at), false) => stops[(at + 1) % stops.len()],
            (Some(at), true) => stops[(at + stops.len() - 1) % stops.len()],
            (None, false) => stops[0],
            (None, true) => stops[stops.len() - 1],
        };
        self.focus_node(next);
    }
}

/// The portable repeat count: 1 for a fresh press, 2 for an auto-repeat. Windows
/// reports a count of 1 for each auto-repeated message, so the repeat is read
/// from bit 30 of `lparam` (the key was already down), as the software backend
/// reports it.
pub(super) fn repeat_of(msg: Msg) -> u16 {
    if msg.lparam & (1 << 30) != 0 { 2 } else { 1 }
}
