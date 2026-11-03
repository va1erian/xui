#![forbid(unsafe_code)]

//! Decoding window messages into portable events.

use windows::Win32::UI::WindowsAndMessaging::{
    WM_ACTIVATE, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_XBUTTONDBLCLK,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};
use xui_core::backend::{Event, WidgetId, WindowId};
use xui_core::message::MouseButton;

use super::Shared;
use crate::sys::window::{self, Hwnd, Msg, WM_WAKE};

/// `SIZE_MINIMIZED`, the `wparam` of a `WM_SIZE` for a minimised window.
const SIZE_MINIMIZED: usize = 1;
/// `WM_MOUSELEAVE` (0x02A3 in winuser.h), which the `windows` crate omits.
const WM_MOUSELEAVE: u32 = 0x02A3;
/// `HTCLIENT`, the hit-test code of the client area.
const HTCLIENT: isize = 1;

fn low(value: isize) -> i32 {
    i32::from(value as u16 as i16)
}

fn high(value: isize) -> i32 {
    i32::from((value >> 16) as u16 as i16)
}

enum Press {
    Down,
    Up,
    Double,
}

/// The button and press kind a mouse-button message reports.
fn button_of(msg: Msg) -> Option<(MouseButton, Press)> {
    let x_button = if (msg.wparam >> 16) & 0xFFFF == 1 {
        MouseButton::X1
    } else {
        MouseButton::X2
    };
    Some(match msg.id {
        WM_LBUTTONDOWN => (MouseButton::Left, Press::Down),
        WM_LBUTTONUP => (MouseButton::Left, Press::Up),
        WM_LBUTTONDBLCLK => (MouseButton::Left, Press::Double),
        WM_RBUTTONDOWN => (MouseButton::Right, Press::Down),
        WM_RBUTTONUP => (MouseButton::Right, Press::Up),
        WM_RBUTTONDBLCLK => (MouseButton::Right, Press::Double),
        WM_MBUTTONDOWN => (MouseButton::Middle, Press::Down),
        WM_MBUTTONUP => (MouseButton::Middle, Press::Up),
        WM_MBUTTONDBLCLK => (MouseButton::Middle, Press::Double),
        WM_XBUTTONDOWN => (x_button, Press::Down),
        WM_XBUTTONUP => (x_button, Press::Up),
        WM_XBUTTONDBLCLK => (x_button, Press::Double),
        _ => return None,
    })
}

impl Shared {
    /// Handles one message of `window`; `None` leaves it to Windows.
    pub(super) fn handle(&self, window: WindowId, hwnd: Hwnd, msg: Msg) -> Option<isize> {
        if !self.windows.borrow().contains_key(&window.raw()) {
            return None;
        }
        let modifiers = window::modifiers();
        let (x, y) = (low(msg.lparam), high(msg.lparam));
        if let Some((button, press)) = button_of(msg) {
            let event = match press {
                Press::Down => Event::MouseDown {
                    x,
                    y,
                    button,
                    modifiers,
                },
                Press::Up => Event::MouseUp {
                    x,
                    y,
                    button,
                    modifiers,
                },
                Press::Double => Event::MouseDoubleClick {
                    x,
                    y,
                    button,
                    modifiers,
                },
            };
            self.pointer(window, event);
            return Some(0);
        }
        match msg.id {
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                self.paint(window);
                Some(0)
            }
            WM_SIZE => self.on_size(window, msg),
            WM_DPICHANGED => self.on_dpi(window, hwnd, msg),
            WM_MOUSEMOVE => {
                self.track_leave(window, hwnd);
                self.pointer(window, Event::MouseMove { x, y, modifiers });
                Some(0)
            }
            WM_MOUSELEAVE => {
                self.leave(window);
                Some(0)
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                // The wheel reports screen coordinates.
                let (x, y) = window::screen_to_client(hwnd, x, y);
                let delta = (msg.wparam >> 16) as u16 as i16;
                let horizontal = msg.id == WM_MOUSEHWHEEL;
                self.pointer(
                    window,
                    Event::MouseWheel {
                        delta,
                        horizontal,
                        x,
                        y,
                        modifiers,
                    },
                );
                Some(0)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => self.key_down(window, msg, msg.id == WM_SYSKEYDOWN),
            WM_KEYUP | WM_SYSKEYUP => self.key_up(window, msg, msg.id == WM_SYSKEYUP),
            WM_CHAR => {
                self.character(window, msg.wparam as u16);
                Some(0)
            }
            WM_SETCURSOR if low(msg.lparam) as isize == HTCLIENT => self.on_set_cursor(window),
            WM_TIMER => {
                let owner = self.timers.borrow().get(&msg.wparam).copied();
                if owner == Some(window.raw()) {
                    let id = xui_core::backend::TimerId(msg.wparam);
                    self.deliver(window, WidgetId::NONE, &Event::Timer { id });
                }
                Some(0)
            }
            WM_ACTIVATE => {
                let active = msg.wparam & 0xFFFF != 0;
                let minimized = (msg.wparam >> 16) & 0xFFFF != 0;
                self.deliver(
                    window,
                    WidgetId::NONE,
                    &Event::Activate { active, minimized },
                );
                None
            }
            WM_WAKE => {
                self.deliver(window, WidgetId::NONE, &Event::Wake);
                Some(0)
            }
            WM_CLOSE => self
                .deliver(window, WidgetId::NONE, &Event::Close)
                .then_some(0),
            WM_DESTROY => {
                let primary = self
                    .windows
                    .borrow()
                    .get(&window.raw())
                    .is_some_and(|w| w.primary);
                if primary {
                    window::post_quit(self.exit_code.get());
                }
                None
            }
            _ => None,
        }
    }

    fn on_size(&self, window: WindowId, msg: Msg) -> Option<isize> {
        if msg.wparam == SIZE_MINIMIZED {
            return Some(0);
        }
        let (width, height) = (low(msg.lparam), high(msg.lparam));
        let target = {
            let mut windows = self.windows.borrow_mut();
            let state = windows.get_mut(&window.raw())?;
            state.size = (width, height);
            state.target.clone()
        };
        if let Some(target) = target
            && !target.resize(width, height)
            && let Some(state) = self.windows.borrow_mut().get_mut(&window.raw())
        {
            state.target = None;
        }
        self.invalidate_window(window);
        self.deliver(window, WidgetId::NONE, &Event::Resize { width, height });
        Some(0)
    }

    fn on_dpi(&self, window: WindowId, hwnd: Hwnd, msg: Msg) -> Option<isize> {
        let dpi = ((msg.wparam >> 16) & 0xFFFF) as u32;
        if let Some(state) = self.windows.borrow_mut().get_mut(&window.raw()) {
            state.dpi = dpi;
        }
        let suggested = window::apply_suggested_rect(hwnd, msg.lparam);
        self.invalidate_window(window);
        self.deliver(
            window,
            WidgetId::NONE,
            &Event::DpiChanged { dpi, suggested },
        );
        Some(0)
    }

    fn on_set_cursor(&self, window: WindowId) -> Option<isize> {
        let hover = self.windows.borrow().get(&window.raw())?.hover?;
        let cursor = self.scene.borrow().get(hover)?.cursor;
        window::set_cursor(cursor);
        Some(1)
    }

    fn track_leave(&self, window: WindowId, hwnd: Hwnd) {
        let mut windows = self.windows.borrow_mut();
        if let Some(state) = windows.get_mut(&window.raw())
            && !state.tracking
        {
            state.tracking = true;
            window::track_leave(hwnd);
        }
    }
}
