//! Input handling for [`HtmlWidget`](super::HtmlWidget): wheel and keyboard
//! scrolling, link clicks, selection drags and multi-clicks. Split out of
//! `mod.rs` so each file stays small; behaviour is unchanged.

use xui_core::backend::{Cursor, Event};
use xui_core::message::{Key, MouseButton};

use super::{DRAG_SLOP_DIP, HtmlWidget, KEY_LINE_DIP, WHEEL_LINE_DIP};
use crate::geom::Point;
use crate::selection::Selection;
use crate::view::HtmlViewEvent;

/// What handling an event asks of the host: the view applies these to its
/// node after the widget's state has been updated.
#[derive(Default)]
pub(crate) struct Effects {
    pub(crate) invalidate: bool,
    pub(crate) focus: bool,
    pub(crate) capture: bool,
    pub(crate) release_capture: bool,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) emit: Option<HtmlViewEvent>,
}

impl Effects {
    fn invalidate(&mut self) {
        self.invalidate = true;
    }

    fn focus(&mut self) {
        self.focus = true;
    }

    fn capture(&mut self) {
        self.capture = true;
    }

    fn release_capture(&mut self) {
        self.release_capture = true;
    }

    fn cursor(&mut self, cursor: Cursor) {
        self.cursor = Some(cursor);
    }

    fn emit(&mut self, event: HtmlViewEvent) {
        self.emit = Some(event);
    }
}

impl HtmlWidget {
    pub(crate) fn handle_input(&self, event: &Event) -> Effects {
        let mut cx = Effects::default();
        self.dispatch(event, &mut cx);
        cx
    }

    fn dispatch(&self, event: &Event, cx: &mut Effects) {
        match *event {
            Event::MouseWheel {
                delta,
                horizontal: false,
                ..
            } => {
                self.scroll_by(-delta as f32 / 120.0 * WHEEL_LINE_DIP);
                cx.invalidate();
            }
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let p = self.scale_point(x, y);
                self.register_click(p);
                self.press.set(Some(p));
                self.dragging.set(true);
                self.moved.set(false);
                cx.focus();
                cx.capture();
                let doc = self.doc_point(p);
                if let Some(at) = self.caret_at(doc) {
                    let sel = if self.shift_held.get() {
                        match self.selection.get() {
                            Some(sel) => Selection {
                                anchor: sel.anchor,
                                head: at,
                            },
                            None => Selection::caret(at),
                        }
                    } else {
                        Selection::caret(at)
                    };
                    self.selection.set(Some(sel));
                }
                cx.invalidate();
            }
            Event::MouseMove { x, y, .. } => {
                let p = self.scale_point(x, y);
                if self.dragging.get() {
                    if let Some(press) = self.press.get()
                        && ((press.x - p.x).abs() > DRAG_SLOP_DIP
                            || (press.y - p.y).abs() > DRAG_SLOP_DIP)
                    {
                        self.moved.set(true);
                    }
                    if self.moved.get()
                        && let (Some(sel), Some(at)) =
                            (self.selection.get(), self.caret_at(self.doc_point(p)))
                    {
                        self.selection.set(Some(Selection {
                            anchor: sel.anchor,
                            head: at,
                        }));
                    }
                    // Dragging past the top/bottom edge keeps scrolling.
                    let viewport = self.viewport_height.get();
                    if p.y < 0.0 {
                        self.scroll_by(p.y);
                    } else if p.y > viewport {
                        self.scroll_by(p.y - viewport);
                    }
                    cx.cursor(Cursor::Text);
                    cx.invalidate();
                } else {
                    let doc = self.doc_point(p);
                    let frame = self.frame.borrow();
                    let cursor = match frame.as_ref() {
                        Some(frame) if frame.links.href_at(doc).is_some() => Cursor::Hand,
                        Some(frame) if frame.runs.is_text_at(doc) => Cursor::Text,
                        _ => Cursor::Default,
                    };
                    cx.cursor(cursor);
                }
            }
            Event::MouseDoubleClick {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let p = self.scale_point(x, y);
                let count = self.register_click(p);
                self.dragging.set(false);
                let doc = self.doc_point(p);
                if let Some(at) = self.caret_at(doc) {
                    let frame = self.frame.borrow();
                    if let Some(frame) = frame.as_ref() {
                        let sel = if count >= 3 {
                            frame.runs.block_at_pos(at)
                        } else {
                            frame.runs.word_at_pos(at)
                        };
                        if let Some(sel) = sel {
                            self.selection.set(Some(sel));
                        }
                    }
                }
                cx.invalidate();
            }
            Event::MouseUp {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging.set(false);
                cx.release_capture();
                let p = self.scale_point(x, y);
                let doc = self.doc_point(p);
                // A clean single click (not a drag, not a multi-click) that
                // lands on a link reports it.
                if !self.moved.get() && self.click_count.get() == 1 {
                    let href = self
                        .frame
                        .borrow()
                        .as_ref()
                        .and_then(|f| f.links.href_at(doc))
                        .map(str::to_string);
                    if let Some(href) = href {
                        self.selection.set(None);
                        cx.emit(HtmlViewEvent::LinkClicked(href));
                    }
                }
                self.moved.set(false);
            }
            Event::MouseLeave => {
                if !self.dragging.get() {
                    cx.cursor(Cursor::Default);
                }
            }
            Event::CaptureChanged => {
                self.dragging.set(false);
            }
            Event::KeyDown {
                key,
                modifiers,
                repeat: _,
                system: _,
            } => {
                if key == Key::SHIFT {
                    self.shift_held.set(true);
                } else if modifiers.ctrl && key == Key::C {
                    self.copy_selection();
                } else if modifiers.ctrl && key == Key::A {
                    self.select_all();
                    cx.invalidate();
                } else {
                    self.scroll_key(key, cx);
                }
            }
            Event::KeyUp { key, .. } if key == Key::SHIFT => {
                self.shift_held.set(false);
            }
            _ => {}
        }
    }

    /// Converts client device pixels to device-independent pixels.
    fn scale_point(&self, x: i32, y: i32) -> Point {
        let s = self.scale.get();
        Point::new(x as f32 / s, y as f32 / s)
    }

    fn scroll_key(&self, key: Key, cx: &mut Effects) {
        let page = self.viewport_height.get();
        let delta = if key == Key::DOWN {
            KEY_LINE_DIP
        } else if key == Key::UP {
            -KEY_LINE_DIP
        } else if key == Key::PAGE_DOWN {
            page
        } else if key == Key::PAGE_UP {
            -page
        } else if key == Key::HOME {
            -self.scroll.get()
        } else if key == Key::END {
            self.max_scroll() - self.scroll.get()
        } else {
            0.0
        };
        if delta != 0.0 {
            self.scroll_by(delta);
            cx.invalidate();
        }
    }
}
