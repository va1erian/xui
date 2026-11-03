#![forbid(unsafe_code)]

//! The handler of one node window: it paints the registered painter and
//! forwards input to the front layer.

use std::cell::Cell;
use std::rc::Rc;

use xui_core::Rect;
use xui_core::backend::{Event, Painter, WidgetId};

use crate::backend::canvas::Win32Canvas;
use crate::gdi::Paint;
use crate::message::{LResult, Message, MouseButton};
use crate::sys;
use crate::window::{Window, WindowHandler};

use super::handler::{WindowShared, to_event};

/// The handler of one node window: it paints the registered painter and
/// forwards input to the front layer.
pub(crate) struct NodeHandler {
    widget: WidgetId,
    shared: Rc<WindowShared>,
    bounds: Rc<Cell<Rect>>,
    painter: Rc<std::cell::RefCell<Option<Painter>>>,
    /// Whether a press that starts on this node drags the window.
    drag_region: Rc<Cell<bool>>,
    tracking_mouse: Cell<bool>,
}

impl NodeHandler {
    /// A handler for `widget`.
    pub(crate) fn new(
        widget: WidgetId,
        shared: Rc<WindowShared>,
        bounds: Rc<Cell<Rect>>,
        painter: Rc<std::cell::RefCell<Option<Painter>>>,
        drag_region: Rc<Cell<bool>>,
    ) -> NodeHandler {
        NodeHandler {
            widget,
            shared,
            bounds,
            painter,
            drag_region,
            tracking_mouse: Cell::new(false),
        }
    }

    /// Runs the registered painter on `window` and returns the region that was
    /// dirty (the paint's `rcPaint`), or an empty rectangle when nothing drew.
    fn paint(&self, window: &Window) -> Rect {
        let Some(painter) = self.painter.borrow().clone() else {
            return Rect::default();
        };
        let hwnd = window.hwnd();
        let Some(paint) = Paint::begin(hwnd) else {
            // A window that cannot begin a paint (a null DC, a lost back
            // buffer) would otherwise keep its update region dirty and loop on
            // `WM_PAINT`; validating leaves it for the next frame.
            sys::window::validate(hwnd);
            return Rect::default();
        };
        let dirty = paint.paint_rect();
        let dpi = sys::dpi::window_dpi(hwnd);
        let bounds = self.bounds.get();
        // The double buffer starts with the stock `System` font; select the
        // shared UI font so text matches the native controls and is
        // anti-aliased rather than the Windows 3.1 bitmap face.
        match crate::gdi::Font::shared_ui(dpi) {
            Ok(font) => paint.canvas().with_font(&font, |canvas| {
                let mut canvas = Win32Canvas::new(canvas, bounds, dpi);
                painter(&mut canvas);
            }),
            Err(_) => {
                let mut canvas = Win32Canvas::new(paint.canvas(), bounds, dpi);
                painter(&mut canvas);
            }
        }
        dirty
    }
}

impl WindowHandler for NodeHandler {
    fn message(&self, window: &Window, message: Message) -> Option<LResult> {
        // The class brush would flash the (light) background before every
        // double-buffered paint, which reads as flicker on hover and while a
        // slider or progress bar drags. The whole dirty rectangle is repainted
        // on `WM_PAINT`, so claim the erase and skip the default fill.
        if matches!(&message, Message::Other { code, .. } if *code == sys::d2d::WM_ERASEBKGND) {
            return Some(1);
        }
        // A left press on a drag region starts a window move on the top-level
        // window, as the system caption would, so the node's own handler never
        // sees the click.
        if self.drag_region.get()
            && matches!(
                &message,
                Message::MouseDown {
                    button: MouseButton::Left,
                    ..
                }
            )
        {
            sys::drag::begin_move(sys::window::root(window.hwnd()));
            return Some(0);
        }
        match &message {
            Message::Paint => {
                // A node without a painter draws nothing, but it still has to
                // validate its update region: `paint` returns before
                // `BeginPaint` when the painter is absent, so an unvalidated
                // region makes Windows re-deliver `WM_PAINT` forever (a busy
                // loop that starves timers and freezes the window). Validate
                // and leave the node transparent.
                if self.painter.borrow().is_none() {
                    sys::window::validate(window.hwnd());
                    return Some(0);
                }
                let dirty = self.paint(window);
                self.shared.deliver(self.widget, Event::Paint { dirty });
                return Some(0);
            }
            Message::Size { width, height } => {
                self.bounds.set(Rect::new(0, 0, *width, *height));
            }
            Message::MouseMove { .. } => {
                if !self.tracking_mouse.replace(true) {
                    let _ = window.track_mouse_leave();
                }
            }
            Message::MouseLeave => self.tracking_mouse.set(false),
            _ => {}
        }
        let event = to_event(&message)?;
        self.shared.deliver(self.widget, event);
        Some(0)
    }
}
