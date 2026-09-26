#![forbid(unsafe_code)]

//! The `winit` [`ApplicationHandler`] that drives [`WinitBackend`]: it creates
//! the real windows and their `softbuffer` surfaces, translates platform input
//! into portable events, and presents a composited frame on redraw.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton as WinitButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, Window, WindowAttributes};

use xui_core::backend::{Event, TimerId, WidgetId, WindowId};
use xui_core::message::{Key, Modifiers, MouseButton};

use super::{Shared, SharedWindow, UserEvent, render};
use crate::Surface;

/// A real window and its software presentation surface.
struct RealWindow {
    _context: softbuffer::Context<SharedWindow>,
    surface: softbuffer::Surface<SharedWindow, SharedWindow>,
}

/// Drives one `winit` event loop for a [`super::WinitBackend`].
struct App {
    shared: Rc<Shared>,
    windows: HashMap<u64, RealWindow>,
    /// The last pointer position in window pixels, for button messages `winit`
    /// sends without one.
    cursor: (f64, f64),
    modifiers: Modifiers,
}

impl App {
    fn window_id(raw: u64) -> WindowId {
        WindowId::from_raw(raw)
    }

    /// Creates the OS window and surface for any backend window that lacks one.
    fn create_windows(&mut self, event_loop: &ActiveEventLoop) {
        let ids: Vec<u64> = self.shared.windows.borrow().keys().copied().collect();
        for raw in ids {
            if self.windows.contains_key(&raw) {
                continue;
            }
            let (title, size) = {
                let windows = self.shared.windows.borrow();
                let Some(state) = windows.get(&raw) else {
                    continue;
                };
                (state.title.clone(), state.size)
            };
            let attributes = WindowAttributes::default()
                .with_title(title)
                .with_inner_size(LogicalSize::new(f64::from(size.0), f64::from(size.1)));
            let Ok(window) = event_loop.create_window(attributes) else {
                continue;
            };
            let window = Rc::new(window);
            let Ok(context) = softbuffer::Context::new(SharedWindow(Rc::clone(&window))) else {
                continue;
            };
            let Ok(surface) = softbuffer::Surface::new(&context, SharedWindow(Rc::clone(&window)))
            else {
                continue;
            };
            let metrics = window_metrics(&window);
            if let Some(state) = self.shared.windows.borrow_mut().get_mut(&raw) {
                state.size = metrics.0;
                state.dpi = metrics.1;
                state.window = Some(Rc::clone(&window));
            }
            self.windows.insert(
                raw,
                RealWindow {
                    _context: context,
                    surface,
                },
            );
            self.redraw(raw);
        }
    }

    /// Composites and presents one frame of `raw`.
    fn redraw(&mut self, raw: u64) {
        let window_id = Self::window_id(raw);
        let (width, height) = match self.shared.windows.borrow().get(&raw) {
            Some(state) => state.size,
            None => return,
        };
        let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return;
        };
        let Some(real) = self.windows.get_mut(&raw) else {
            return;
        };
        if real.surface.resize(width, height).is_err() {
            return;
        }
        let mut surface = Surface::new(width.get(), height.get());
        render::composite(&self.shared, window_id, &mut surface);
        let image = surface.to_image();
        let Ok(mut buffer) = real.surface.buffer_mut() else {
            return;
        };
        for (destination, pixel) in buffer.iter_mut().zip(image.pixels.as_chunks::<4>().0) {
            // softbuffer wants `0x00RRGGBB`; the pixmap is premultiplied RGBA
            // over an opaque background, so the channels are final.
            *destination =
                (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
        }
        let _ = buffer.present();
    }

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
    fn apply_cursor(&self, raw: u64, target: Option<WidgetId>) {
        let icon = match target {
            Some(id) => match self.shared.cursors.borrow().get(&id.raw()) {
                Some(xui_core::backend::Cursor::Hand) => CursorIcon::Pointer,
                Some(xui_core::backend::Cursor::Text) => CursorIcon::Text,
                _ => CursorIcon::Default,
            },
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

    fn cursor_moved(&mut self, raw: u64, x: f64, y: f64) {
        self.cursor = (x, y);
        let window = Self::window_id(raw);
        let (x, y) = (x as i32, y as i32);
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

    fn mouse_input(&mut self, raw: u64, state: ElementState, button: WinitButton) {
        let window = Self::window_id(raw);
        let (x, y) = (self.cursor.0 as i32, self.cursor.1 as i32);
        let Some((id, lx, ly)) = self.shared.hit_test(window, x, y) else {
            return;
        };
        let Some(button) = mouse_button(button) else {
            return;
        };
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

    fn mouse_wheel(&mut self, raw: u64, delta: MouseScrollDelta) {
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

    fn keyboard_input(
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

    /// Fires any due timers and arms the next wake-up.
    fn timers(&self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let due: Vec<(usize, WindowId)> = self
            .shared
            .timers
            .borrow()
            .iter()
            .filter(|(_, (_, at))| *at <= now)
            .map(|(id, (window, _))| (*id, *window))
            .collect();
        for (id, window) in due {
            self.shared.timers.borrow_mut().remove(&id);
            self.shared
                .deliver(window, WidgetId::NONE, &Event::Timer { id: TimerId(id) });
        }
        let next = self
            .shared
            .timers
            .borrow()
            .values()
            .map(|(_, at)| *at)
            .min();
        match next {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

/// The window's `(size, dpi)` from its scale factor.
fn window_metrics(window: &Window) -> ((u32, u32), u32) {
    let size = window.inner_size();
    let dpi = (window.scale_factor() * 96.0).round().max(1.0) as u32;
    ((size.width.max(1), size.height.max(1)), dpi)
}

fn mouse_button(button: WinitButton) -> Option<MouseButton> {
    Some(match button {
        WinitButton::Left => MouseButton::Left,
        WinitButton::Right => MouseButton::Right,
        WinitButton::Middle => MouseButton::Middle,
        WinitButton::Back => MouseButton::X1,
        WinitButton::Forward => MouseButton::X2,
        _ => return None,
    })
}

/// Maps a `winit` logical key to the `VK_*` code the portable widgets use.
fn virtual_key(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(named) => match named {
            NamedKey::Enter => Key::RETURN,
            NamedKey::Space => Key::SPACE,
            NamedKey::Escape => Key::ESCAPE,
            NamedKey::Tab => Key::TAB,
            NamedKey::Backspace => Key::BACK,
            NamedKey::Delete => Key::DELETE,
            NamedKey::Insert => Key::INSERT,
            NamedKey::Home => Key::HOME,
            NamedKey::End => Key::END,
            NamedKey::PageUp => Key::PAGE_UP,
            NamedKey::PageDown => Key::PAGE_DOWN,
            NamedKey::ArrowLeft => Key::LEFT,
            NamedKey::ArrowRight => Key::RIGHT,
            NamedKey::ArrowUp => Key::UP,
            NamedKey::ArrowDown => Key::DOWN,
            NamedKey::F1 => Key::F1,
            NamedKey::F2 => Key::F2,
            NamedKey::F3 => Key::F3,
            NamedKey::F4 => Key::F4,
            _ => return None,
        },
        WinitKey::Character(text) => {
            let character = text.chars().next()?;
            if character.is_ascii_alphabetic() {
                Key::from_code(character.to_ascii_uppercase() as u16)
            } else if character.is_ascii_digit() {
                Key::from_code(character as u16)
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.create_windows(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let raw = window_id_index(self, window_id);
        let Some(raw) = raw else {
            return;
        };
        match event {
            WindowEvent::RedrawRequested => self.redraw(raw),
            WindowEvent::CloseRequested => {
                let handled =
                    self.shared
                        .deliver(Self::window_id(raw), WidgetId::NONE, &Event::Close);
                if !handled {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = self.shared.windows.borrow_mut().get_mut(&raw) {
                    state.size = (size.width.max(1), size.height.max(1));
                }
                self.shared.deliver(
                    Self::window_id(raw),
                    WidgetId::NONE,
                    &Event::Resize {
                        width: size.width as i32,
                        height: size.height as i32,
                    },
                );
                self.redraw(raw);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let dpi = (scale_factor * 96.0).round().max(1.0) as u32;
                if let Some(state) = self.shared.windows.borrow_mut().get_mut(&raw) {
                    state.dpi = dpi;
                }
                self.shared.deliver(
                    Self::window_id(raw),
                    WidgetId::NONE,
                    &Event::DpiChanged {
                        dpi,
                        suggested: xui_core::Rect::default(),
                    },
                );
                self.redraw(raw);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_moved(raw, position.x, position.y);
            }
            WindowEvent::CursorLeft { .. } => self.apply_cursor(raw, None),
            WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_input(raw, state, button);
            }
            WindowEvent::MouseWheel { delta, .. } => self.mouse_wheel(raw, delta),
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = Modifiers {
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                    alt: state.alt_key(),
                    win: state.super_key(),
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.keyboard_input(
                    raw,
                    event.state,
                    event.repeat,
                    &event.logical_key,
                    event.text.as_deref(),
                );
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        if self.shared.quit.get() {
            event_loop.exit();
            return;
        }
        match event {
            UserEvent::Wake(window) => {
                self.shared.deliver(window, WidgetId::NONE, &Event::Wake);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.shared.quit.get() {
            event_loop.exit();
            return;
        }
        self.timers(event_loop);
    }
}

/// The raw id of the window `winit` handed the event for, or `None` for a
/// window the backend does not track.
fn window_id_index(app: &App, window_id: winit::window::WindowId) -> Option<u64> {
    app.shared
        .windows
        .borrow()
        .iter()
        .find(|(_, state)| {
            state
                .window
                .as_ref()
                .is_some_and(|window| window.id() == window_id)
        })
        .map(|(raw, _)| *raw)
}

/// Runs the event loop until the windows close or the app quits.
pub(crate) fn run(event_loop: EventLoop<UserEvent>, shared: Rc<Shared>) {
    let mut app = App {
        shared,
        windows: HashMap::new(),
        cursor: (0.0, 0.0),
        modifiers: Modifiers::NONE,
    };
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_map_to_their_vk_codes() {
        assert_eq!(
            virtual_key(&WinitKey::Named(NamedKey::Enter)),
            Some(Key::RETURN)
        );
        assert_eq!(
            virtual_key(&WinitKey::Named(NamedKey::ArrowLeft)),
            Some(Key::LEFT)
        );
        assert_eq!(
            virtual_key(&WinitKey::Named(NamedKey::Escape)),
            Some(Key::ESCAPE)
        );
        assert_eq!(virtual_key(&WinitKey::Named(NamedKey::F1)), Some(Key::F1));
    }

    #[test]
    fn character_keys_map_case_insensitively() {
        assert_eq!(virtual_key(&WinitKey::Character("a".into())), Some(Key::A));
        assert_eq!(
            virtual_key(&WinitKey::Character("7".into())),
            Some(Key::DIGIT7)
        );
        assert_eq!(virtual_key(&WinitKey::Character("-".into())), None);
    }

    #[test]
    fn only_the_mouse_buttons_the_core_models_are_mapped() {
        assert_eq!(mouse_button(WinitButton::Left), Some(MouseButton::Left));
        assert_eq!(mouse_button(WinitButton::Back), Some(MouseButton::X1));
        assert_eq!(mouse_button(WinitButton::Forward), Some(MouseButton::X2));
    }
}
