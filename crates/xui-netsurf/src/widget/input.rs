#![forbid(unsafe_code)]

//! Input for [`NetSurfWidget`](super::NetSurfWidget). The wheel and the
//! navigation keys scroll here; the pointer and typed characters go to
//! NetSurf in document pixels, which follows links and edits form fields
//! itself.

use xui_core::backend::Event;
use xui_core::message::{Key, MouseButton};

use super::NetSurfWidget;
use crate::engine::{Command, MouseAction};

/// CSS pixels one wheel notch scrolls.
const WHEEL_LINE: f32 = 60.0;
/// CSS pixels one arrow key scrolls.
const KEY_LINE: f32 = 40.0;
/// CSS pixels the pointer may move and still click.
const DRAG_SLOP: i32 = 3;

/// What handling an event asks of the host node.
#[derive(Default)]
pub(crate) struct Effects {
    pub(crate) invalidate: bool,
    pub(crate) focus: bool,
    pub(crate) capture: bool,
    pub(crate) release_capture: bool,
}

impl NetSurfWidget {
    pub(crate) fn handle_input(&self, event: &Event) -> Effects {
        let mut cx = Effects::default();
        match *event {
            Event::MouseWheel {
                delta,
                horizontal: false,
                ..
            } => {
                self.scroll_by(-delta as f32 / 120.0 * WHEEL_LINE);
                cx.invalidate = true;
            }
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let p = self.doc_point(x, y);
                self.press.set(Some(p));
                self.moved.set(false);
                self.mouse(MouseAction::Press, p);
                cx.focus = true;
                cx.capture = true;
            }
            Event::MouseMove { x, y, .. } => {
                let p = self.doc_point(x, y);
                if let Some(start) = self.press.get()
                    && ((p.0 - start.0).abs() > DRAG_SLOP || (p.1 - start.1).abs() > DRAG_SLOP)
                {
                    self.moved.set(true);
                }
                self.mouse(MouseAction::Move, p);
            }
            Event::MouseUp {
                x,
                y,
                button: MouseButton::Left,
                ..
            } if self.press.take().is_some() => {
                let action = if self.moved.get() {
                    MouseAction::Release
                } else {
                    MouseAction::Click
                };
                self.mouse(action, self.doc_point(x, y));
                cx.release_capture = true;
            }
            Event::KeyDown { key, .. } => {
                if let Some(delta) = self.scroll_delta(key) {
                    self.scroll_by(delta);
                    cx.invalidate = true;
                } else if let Some(code) = netsurf_key(key) {
                    self.send(Command::Key {
                        id: self.id(),
                        key: code,
                    });
                }
            }
            Event::Char(ch) if !ch.is_control() => {
                self.send(Command::Key {
                    id: self.id(),
                    key: ch as u32,
                });
            }
            _ => {}
        }
        cx
    }

    /// Client device pixels as document CSS pixels.
    fn doc_point(&self, x: i32, y: i32) -> (i32, i32) {
        let s = self.scale.get();
        (
            (x as f32 / s).round() as i32,
            (y as f32 / s + self.scroll.get()).round() as i32,
        )
    }

    fn mouse(&self, action: MouseAction, (x, y): (i32, i32)) {
        self.send(Command::Mouse {
            id: self.id(),
            action,
            x,
            y,
        });
    }

    fn scroll_delta(&self, key: Key) -> Option<f32> {
        let page = self.viewport_height.get();
        Some(match key {
            Key::UP => -KEY_LINE,
            Key::DOWN => KEY_LINE,
            Key::PAGE_UP => -page,
            Key::PAGE_DOWN => page,
            Key::HOME => -self.content_height(),
            Key::END => self.content_height(),
            _ => return None,
        })
    }
}

/// The NetSurf key code (`NS_KEY_*`) for an editing key.
fn netsurf_key(key: Key) -> Option<u32> {
    Some(match key {
        Key::BACK => 8,
        Key::TAB => 9,
        Key::RETURN => 13,
        Key::ESCAPE => 27,
        Key::LEFT => 28,
        Key::RIGHT => 29,
        Key::DELETE => 127,
        _ => return None,
    })
}
