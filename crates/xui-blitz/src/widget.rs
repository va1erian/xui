//! The custom-painted widget behind [`BlitzView`](crate::BlitzView): it
//! keeps the newest frame the engine drew, paints it, and turns the node's
//! input into the engine's.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use blitz_traits::events::MouseEventButtons;
use xui_core::Color;
use xui_core::backend::{Canvas, Cursor, Event};
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::message::MouseButton;
use xui_core::theme::Theme;

use xui_core::widget::scrollbar::{self, Scroll};

use crate::engine::{Command, Input, Output, PointerAction, ScrollMetrics};

/// What handling an event asks of the host node.
#[derive(Default)]
pub(crate) struct Effects {
    pub focus: bool,
    pub capture: bool,
    pub release_capture: bool,
}

/// The view's UI-thread state. It sits in cells because the painter and the
/// event mapper share it through an `Rc`.
pub(crate) struct Widget {
    tx: Sender<Command>,
    rx: Receiver<Output>,
    /// Set by the engine when it posts a wake; cleared here before draining.
    pending: Arc<AtomicBool>,
    frame: RefCell<Option<Image>>,
    ready: Cell<bool>,
    failed: Cell<bool>,
    background: Cell<Color>,
    /// The size (device pixels) and scale last sent to the engine.
    sent: Cell<((u32, u32), f32)>,
    dark: Cell<Option<bool>>,
    /// Device pixels per CSS pixel, as of the last paint.
    scale: Cell<f32>,
    held: Cell<MouseEventButtons>,
    cursor: Cell<Option<Cursor>>,
    /// How far the page scrolls, as the engine last said (CSS pixels).
    scroll: Cell<ScrollMetrics>,
}

impl Widget {
    pub(crate) fn new(
        tx: Sender<Command>,
        rx: Receiver<Output>,
        pending: Arc<AtomicBool>,
        size: (u32, u32),
        scale: f32,
    ) -> Widget {
        Widget {
            tx,
            rx,
            pending,
            frame: RefCell::new(None),
            ready: Cell::new(false),
            failed: Cell::new(false),
            background: Cell::new(Color::rgb(255, 255, 255)),
            sent: Cell::new((size, scale)),
            dark: Cell::new(None),
            scale: Cell::new(scale),
            held: Cell::new(MouseEventButtons::None),
            cursor: Cell::new(None),
            scroll: Cell::new(ScrollMetrics::default()),
        }
    }

    pub(crate) fn send(&self, command: Command) {
        if self.tx.send(command).is_err() {
            self.failed.set(true);
        }
    }

    pub(crate) fn set_background(&self, color: Color) {
        self.background.set(color);
        self.send(Command::Background(color));
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.get() && self.frame.borrow().is_some()
    }

    pub(crate) fn has_failed(&self) -> bool {
        self.failed.get()
    }

    /// A new page is on its way: what is on show is no longer it.
    pub(crate) fn loading(&self) {
        self.ready.set(false);
        self.failed.set(false);
    }

    /// Takes what the engine sent; the cursor it asked for, if that changed.
    pub(crate) fn drain(&self) -> Option<Cursor> {
        self.pending.store(false, Ordering::Release);
        loop {
            match self.rx.try_recv() {
                Ok(Output::Frame(frame)) => {
                    self.ready.set(frame.ready);
                    *self.frame.borrow_mut() = Some(frame.image);
                }
                Ok(Output::Scroll(metrics)) => self.scroll.set(metrics),
                Ok(Output::Cursor(cursor)) => self.cursor.set(Some(cursor)),
                Ok(Output::Failed(failed)) => self.failed.set(failed),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.failed.set(true);
                    break;
                }
            }
        }
        self.cursor.take()
    }

    /// Paints the newest frame, first telling the engine the node's size,
    /// scale or theme if they changed.
    pub(crate) fn paint(&self, canvas: &mut dyn Canvas, theme: Theme) {
        let bounds = canvas.bounds();
        let scale = canvas.dpi() as f32 / 96.0;
        self.scale.set(scale);
        // The scrollbar's gutter is the view's right edge: the page is laid
        // out in what is left, so the bar never covers it.
        let gutter = scrollbar::THICKNESS.to_px(canvas.dpi()).value();
        let size = (
            (bounds.width() - gutter).max(1) as u32,
            bounds.height().max(1) as u32,
        );
        if self.sent.get() != (size, scale) {
            self.sent.set((size, scale));
            self.send(Command::Resize { size, scale });
        }
        if self.dark.get() != Some(theme.is_dark) {
            self.dark.set(Some(theme.is_dark));
            self.send(Command::Dark(theme.is_dark));
        }
        let frame = self.frame.borrow();
        let Some(image) = frame.as_ref() else {
            return canvas.clear(self.background.get());
        };
        // A frame drawn for the old size shows at its own size until the
        // engine catches up, with the background around it.
        if image.size() != size {
            canvas.clear(self.background.get());
        }
        let (w, h) = (image.width() as i32, image.height() as i32);
        canvas.draw_image(
            image,
            Rect::new(bounds.left, bounds.top, bounds.left + w, bounds.top + h),
        );
    }

    /// The scrollbar's metrics in device pixels.
    pub(crate) fn scroll_metrics(&self) -> Scroll {
        let (m, s) = (self.scroll.get(), self.scale.get());
        Scroll {
            viewport: (m.viewport * s).round() as i32,
            content: (m.content * s).round() as i32,
            offset: (m.offset * s).round() as i32,
        }
    }

    /// The scrollbar asked for the device-pixel offset `target`.
    pub(crate) fn scroll_to_px(&self, target: i32) {
        let s = self.scale.get();
        let mut m = self.scroll.get();
        m.offset = target.max(0) as f32 / s;
        self.scroll.set(m);
        self.send(Command::ScrollTo(m.offset as f64));
    }

    /// Node-local device pixels as client CSS pixels.
    fn css(&self, x: i32, y: i32) -> (f32, f32) {
        let s = self.scale.get();
        (x as f32 / s, y as f32 / s)
    }

    fn pointer(
        &self,
        action: PointerAction,
        x: i32,
        y: i32,
        button: MouseButton,
        mods: xui_core::message::Modifiers,
    ) {
        let (x, y) = self.css(x, y);
        self.send(Command::Input(Input::Pointer {
            action,
            x,
            y,
            button,
            held: self.held.get(),
            mods,
        }));
    }

    pub(crate) fn handle_input(&self, event: &Event) -> Effects {
        let mut fx = Effects::default();
        match *event {
            Event::MouseDown {
                x,
                y,
                button,
                modifiers,
            }
            | Event::MouseDoubleClick {
                x,
                y,
                button,
                modifiers,
            } => {
                self.held.set(self.held.get() | buttons(button));
                self.pointer(PointerAction::Down, x, y, button, modifiers);
                fx.focus = true;
                fx.capture = true;
            }
            Event::MouseUp {
                x,
                y,
                button,
                modifiers,
            } => {
                self.held.set(self.held.get() - buttons(button));
                self.pointer(PointerAction::Up, x, y, button, modifiers);
                fx.release_capture = self.held.get().is_empty();
            }
            Event::MouseMove { x, y, modifiers } => {
                self.pointer(PointerAction::Move, x, y, MouseButton::Left, modifiers);
            }
            Event::MouseWheel {
                delta,
                horizontal,
                x,
                y,
                modifiers,
            } => {
                let (x, y) = self.css(x, y);
                self.send(Command::Input(Input::Wheel {
                    notches: delta as f64 / 120.0,
                    horizontal,
                    x,
                    y,
                    mods: modifiers,
                }));
            }
            Event::CaptureChanged => self.held.set(MouseEventButtons::None),
            Event::KeyDown {
                key,
                modifiers,
                repeat,
                ..
            } => self.send(Command::Input(Input::Key {
                key,
                mods: modifiers,
                repeat: repeat > 1,
            })),
            Event::Char(c) if !c.is_control() => self.send(Command::Input(Input::Char(c))),
            _ => {}
        }
        fx
    }
}

fn buttons(button: MouseButton) -> MouseEventButtons {
    match button {
        MouseButton::Right => MouseEventButtons::Secondary,
        MouseButton::Middle => MouseEventButtons::Auxiliary,
        _ => MouseEventButtons::Primary,
    }
}
