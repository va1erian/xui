#![forbid(unsafe_code)]

//! Pointer, cursor and keyboard translation: tracking the cursor, hit-testing,
//! focus, pointer capture and modifier state, and turning `winit` events into
//! the portable [`Event`] vocabulary.

use winit::event::{ElementState, MouseButton as WinitButton, MouseScrollDelta};
use winit::keyboard::Key as WinitKey;
use winit::window::CursorIcon;

use xui_core::backend::{Event, WidgetId};
use xui_core::message::MouseButton;

use super::App;
use super::keymap::{cursor_icon, mouse_button, virtual_key};

impl App {
    /// Moves the keyboard focus to `id`, telling the old and new holders.
    fn set_focus(&self, raw: u64, id: WidgetId) {
        let previous = self
            .shared
            .windows
            .borrow_mut()
            .get_mut(&raw)
            .and_then(|state| state.focused.replace(id));
        if let Some(previous) = previous
            && previous != id
        {
            self.shared
                .deliver(Self::window_id(raw), previous, &Event::KillFocus);
        }
        if previous != Some(id) {
            self.shared
                .deliver(Self::window_id(raw), id, &Event::SetFocus);
        }
    }

    /// Shows the cursor requested for the node under the pointer.
    pub(super) fn apply_cursor(&self, raw: u64, target: Option<WidgetId>) {
        let icon = match target {
            Some(id) => self
                .shared
                .cursors
                .borrow()
                .get(&id.raw())
                .copied()
                .map(cursor_icon)
                .unwrap_or(CursorIcon::Default),
            None => CursorIcon::Default,
        };
        let handle = self
            .shared
            .windows
            .borrow()
            .get(&raw)
            .and_then(|state| state.window.clone());
        if let Some(handle) = handle {
            handle.set_cursor(icon);
        }
    }

    pub(super) fn cursor_moved(&mut self, raw: u64, x: f64, y: f64) {
        self.cursor = (x, y);
        let window = Self::window_id(raw);
        let (x, y) = (x as i32, y as i32);
        // A captured node keeps receiving moves (and the cursor) even outside
        // its bounds, so a drag survives leaving it.
        if let Some(captured) = *self.shared.captured.borrow() {
            if let Some((lx, ly)) = self.shared.local_point(captured, x, y) {
                self.shared.deliver(
                    window,
                    captured,
                    &Event::MouseMove {
                        x: lx,
                        y: ly,
                        modifiers: self.modifiers,
                    },
                );
            }
            self.apply_cursor(raw, Some(captured));
            return;
        }
        let target = self.shared.hit_test(window, x, y);
        let target_id = target.map(|(id, _, _)| id);
        let previous = self
            .shared
            .windows
            .borrow_mut()
            .get_mut(&raw)
            .and_then(|state| std::mem::replace(&mut state.hover, target_id));
        if previous != target_id
            && let Some(previous) = previous
        {
            self.shared.deliver(window, previous, &Event::MouseLeave);
        }
        if let Some((id, lx, ly)) = target {
            self.shared.deliver(
                window,
                id,
                &Event::MouseMove {
                    x: lx,
                    y: ly,
                    modifiers: self.modifiers,
                },
            );
        }
        self.apply_cursor(raw, target_id);
    }

    pub(super) fn mouse_input(&mut self, raw: u64, state: ElementState, button: WinitButton) {
        let window = Self::window_id(raw);
        let (x, y) = (self.cursor.0 as i32, self.cursor.1 as i32);
        let captured = *self.shared.captured.borrow();
        let target = match captured {
            Some(id) => self
                .shared
                .local_point(id, x, y)
                .map(|(lx, ly)| (id, lx, ly)),
            None => self.shared.hit_test(window, x, y),
        };
        let Some((id, lx, ly)) = target else {
            return;
        };
        let Some(button) = mouse_button(button) else {
            return;
        };
        // A left-button press on a drag region moves the whole window, as a
        // title bar's empty area does, instead of reaching the node.
        if state == ElementState::Pressed
            && button == MouseButton::Left
            && captured.is_none()
            && self.shared.is_drag_region(id)
        {
            let handle = self
                .shared
                .windows
                .borrow()
                .get(&raw)
                .and_then(|state| state.window.clone());
            if let Some(handle) = handle {
                let _ = handle.drag_window();
            }
            return;
        }
        let event = if state == ElementState::Pressed {
            self.set_focus(raw, id);
            Event::MouseDown {
                x: lx,
                y: ly,
                button,
                modifiers: self.modifiers,
            }
        } else {
            Event::MouseUp {
                x: lx,
                y: ly,
                button,
                modifiers: self.modifiers,
            }
        };
        self.shared.deliver(window, id, &event);
    }

    pub(super) fn mouse_wheel(&mut self, raw: u64, delta: MouseScrollDelta) {
        let window = Self::window_id(raw);
        let (x, y) = (self.cursor.0 as i32, self.cursor.1 as i32);
        let Some((id, lx, ly)) = self.shared.hit_test(window, x, y) else {
            return;
        };
        // A line is one wheel notch; `winit` reports fractions, so scale.
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => ((x * 120.0) as i32, (y * 120.0) as i32),
            MouseScrollDelta::PixelDelta(position) => (position.x as i32, position.y as i32),
        };
        if dy == 0 && dx == 0 {
            return;
        }
        let horizontal = dy == 0;
        self.shared.deliver(
            window,
            id,
            &Event::MouseWheel {
                delta: (if horizontal { dx } else { dy }).clamp(-32768, 32767) as i16,
                horizontal,
                x: lx,
                y: ly,
                modifiers: self.modifiers,
            },
        );
    }

    pub(super) fn keyboard_input(
        &mut self,
        raw: u64,
        state: ElementState,
        repeat: bool,
        logical: &WinitKey,
        text: Option<&str>,
    ) {
        let window = Self::window_id(raw);
        let focused = self
            .shared
            .windows
            .borrow()
            .get(&raw)
            .and_then(|state| state.focused);
        let Some(id) = focused else {
            return;
        };
        let Some(key) = virtual_key(logical) else {
            return;
        };
        if state == ElementState::Pressed {
            self.shared.deliver(
                window,
                id,
                &Event::KeyDown {
                    key,
                    modifiers: self.modifiers,
                    repeat: if repeat { 2 } else { 1 },
                    system: false,
                },
            );
            if let Some(text) = text {
                for character in text.chars() {
                    if !character.is_control() {
                        self.shared.deliver(window, id, &Event::Char(character));
                    }
                }
            }
        } else {
            self.shared.deliver(
                window,
                id,
                &Event::KeyUp {
                    key,
                    modifiers: self.modifiers,
                    system: false,
                },
            );
        }
    }
}
