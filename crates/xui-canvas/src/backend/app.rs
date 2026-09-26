#![forbid(unsafe_code)]

//! The `winit` [`ApplicationHandler`] that drives [`WinitBackend`]: it creates
//! the real windows, translates platform input into portable events, and
//! presents a frame on redraw — an OpenGL frame when the window has GL content,
//! otherwise a `softbuffer` copy of the software composite.

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

use xui_core::backend::{Decorations, Event, TimerId, WidgetId, WindowId};
use xui_core::geometry::Rect;
use xui_core::message::{Key, Modifiers, MouseButton};

use super::software::RealWindow;
use super::{Shared, UserEvent, render};
use crate::Surface;

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

    /// Creates the OS window for any backend window that lacks one. Its
    /// presentation surface is created lazily on the first frame.
    fn create_windows(&mut self, event_loop: &ActiveEventLoop) {
        let ids: Vec<u64> = self.shared.windows.borrow().keys().copied().collect();
        for raw in ids {
            if self.windows.contains_key(&raw) {
                continue;
            }
            let (title, size, decorations) = {
                let windows = self.shared.windows.borrow();
                let Some(state) = windows.get(&raw) else {
                    continue;
                };
                (state.title.clone(), state.size, state.decorations)
            };
            // A requested backdrop (Acrylic/Mica) is approximated by the opaque
            // theme background on platforms without the DWM material; softbuffer
            // presents an opaque surface, so there is nothing else to do.
            let attributes = WindowAttributes::default()
                .with_title(title)
                .with_inner_size(LogicalSize::new(f64::from(size.0), f64::from(size.1)))
                .with_decorations(decorations == Decorations::System);
            let Ok(window) = event_loop.create_window(attributes) else {
                continue;
            };
            let window = Rc::new(window);
            let metrics = window_metrics(&window);
            if let Some(state) = self.shared.windows.borrow_mut().get_mut(&raw) {
                state.size = metrics.0;
                state.window = Some(Rc::clone(&window));
            }
            self.set_dpi(raw, metrics.1);
            self.windows.insert(raw, RealWindow::new());
            self.redraw(raw);
        }
    }

    /// Records `raw`'s dots-per-inch, lifting the node bounds the app laid out
    /// at the old value to the new scale so the window stays filled.
    ///
    /// `run_app` builds the widgets before the real window exists, so they are
    /// laid out at [`super::DEFAULT_DPI`]; the first real window reports the
    /// monitor's scale factor (2.0 on a Retina display) and the bounds move to
    /// the backing pixels. Later scale changes (a window dragged to another
    /// monitor) rescale by the ratio, since the portable widgets do not reflow
    /// themselves.
    fn set_dpi(&self, raw: u64, dpi: u32) {
        let old = {
            let mut windows = self.shared.windows.borrow_mut();
            let Some(state) = windows.get_mut(&raw) else {
                return;
            };
            let old = state.dpi;
            state.dpi = dpi;
            old
        };
        if old != dpi {
            rescale_nodes(&self.shared, raw, dpi as f32 / old as f32);
        }
    }

    /// Presents one frame of `raw`: an OpenGL frame when the window has GL
    /// content and a context could be created, otherwise the software
    /// composite (including a GL widget's fallback paint).
    fn redraw(&mut self, raw: u64) {
        let window_id = Self::window_id(raw);
        let (width, height, background, gl, window) = match self.shared.windows.borrow().get(&raw) {
            Some(state) => (
                state.size.0,
                state.size.1,
                state.theme.background,
                state.gl.clone(),
                state.window.clone(),
            ),
            None => return,
        };
        let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return;
        };

        if let (Some(widget), Some(window)) = (gl, window) {
            let bounds = Rect::new(0, 0, width.get() as i32, height.get() as i32);
            let painted = {
                let mut windows = self.shared.windows.borrow_mut();
                let Some(state) = windows.get_mut(&raw) else {
                    return;
                };
                let theme = state.theme;
                state.renderer.frame(
                    &window,
                    width.get(),
                    height.get(),
                    background,
                    |gl| widget.paint_gl(gl, bounds, &theme),
                    |gl| widget.gl_teardown(gl),
                )
            };
            if painted {
                return;
            }
            // The context failed: fall through to the software fallback, which
            // paints the widget through `GlWidget::paint`.
        }

        let handle = self
            .shared
            .windows
            .borrow()
            .get(&raw)
            .and_then(|state| state.window.clone());
        let Some(real) = self.windows.get_mut(&raw) else {
            return;
        };
        if !handle.is_some_and(|handle| real.ensure_software(&handle)) {
            return;
        }
        let Some(software) = real.surface_mut() else {
            return;
        };
        if software.resize(width, height).is_err() {
            return;
        }
        let mut surface = Surface::new(width.get(), height.get());
        render::composite(&self.shared, window_id, &mut surface);
        let image = surface.to_image();
        let Ok(mut buffer) = software.buffer_mut() else {
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

    fn cursor_moved(&mut self, raw: u64, x: f64, y: f64) {
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

    fn mouse_input(&mut self, raw: u64, state: ElementState, button: WinitButton) {
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

/// The dots-per-inch a `winit` scale factor corresponds to (96 at 100%).
fn dpi_from_scale(scale_factor: f64) -> u32 {
    (scale_factor * f64::from(super::DEFAULT_DPI))
        .round()
        .max(1.0) as u32
}

/// The window's `(size, dpi)` from its scale factor.
fn window_metrics(window: &Window) -> ((u32, u32), u32) {
    let size = window.inner_size();
    let dpi = dpi_from_scale(window.scale_factor());
    ((size.width.max(1), size.height.max(1)), dpi)
}

/// Multiplies `rect`'s edges by `ratio`, rounding to the nearest pixel.
fn scale_rect(rect: Rect, ratio: f32) -> Rect {
    let scale = |value: i32| (value as f32 * ratio).round() as i32;
    Rect::new(
        scale(rect.left),
        scale(rect.top),
        scale(rect.right),
        scale(rect.bottom),
    )
}

/// Rescales every node of `raw` from one layout scale to another.
fn rescale_nodes(shared: &Shared, raw: u64, ratio: f32) {
    if (ratio - 1.0).abs() < f32::EPSILON {
        return;
    }
    for (_, node) in shared.nodes.borrow_mut().iter_mut() {
        if node.window.raw() == raw {
            node.bounds = scale_rect(node.bounds, ratio);
        }
    }
}

/// The `winit` icon for a portable pointer shape.
fn cursor_icon(cursor: xui_core::backend::Cursor) -> CursorIcon {
    use xui_core::backend::Cursor;
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Hand => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::SizeHorizontal => CursorIcon::EwResize,
        Cursor::SizeVertical => CursorIcon::NsResize,
    }
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
                let dpi = dpi_from_scale(scale_factor);
                self.set_dpi(raw, dpi);
                // `winit` may report the scale change before the matching
                // `Resized`, so adopt the fresh backing size now; the next
                // frame then presents at the new DPI.
                if let Some(size) = self
                    .shared
                    .windows
                    .borrow()
                    .get(&raw)
                    .and_then(|state| state.window.clone())
                    .map(|window| window.inner_size())
                    && let Some(state) = self.shared.windows.borrow_mut().get_mut(&raw)
                {
                    state.size = (size.width.max(1), size.height.max(1));
                }
                // `winit` gives only the scale factor, not the OS's suggested
                // window rectangle, so the suggestion is empty and the app
                // re-lays-out from `Ui::dpi` instead.
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
    fn the_resize_cursors_map_to_their_winit_icons() {
        use xui_core::backend::Cursor;
        assert_eq!(cursor_icon(Cursor::Hand), CursorIcon::Pointer);
        assert_eq!(cursor_icon(Cursor::SizeHorizontal), CursorIcon::EwResize);
        assert_eq!(cursor_icon(Cursor::SizeVertical), CursorIcon::NsResize);
    }

    #[test]
    fn only_the_mouse_buttons_the_core_models_are_mapped() {
        assert_eq!(mouse_button(WinitButton::Left), Some(MouseButton::Left));
        assert_eq!(mouse_button(WinitButton::Back), Some(MouseButton::X1));
        assert_eq!(mouse_button(WinitButton::Forward), Some(MouseButton::X2));
    }

    #[test]
    fn scale_factor_maps_to_a_dots_per_inch() {
        assert_eq!(dpi_from_scale(1.0), 96);
        assert_eq!(dpi_from_scale(1.25), 120);
        assert_eq!(dpi_from_scale(2.0), 192, "a Retina display is 2x");
        assert_eq!(dpi_from_scale(0.0), 1, "never zero");
    }

    #[test]
    fn scaling_a_rect_lifts_the_backing_store() {
        let rect = Rect::new(16, 12, 380, 40);
        assert_eq!(scale_rect(rect, 2.0), Rect::new(32, 24, 760, 80));
        assert_eq!(scale_rect(rect, 1.25), Rect::new(20, 15, 475, 50));
    }
}
