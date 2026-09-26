#![forbid(unsafe_code)]

//! The window handlers the Win32 backend installs: one for a top-level window,
//! one for each node. They decode the typed [`Message`] into a portable
//! [`Event`] and deliver it to the window's [`WidgetHost`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Graphics::Gdi::{HBRUSH, HDC, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{EN_CHANGE, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC};

use xui_core::backend::{Event, Painter, WidgetId, WindowId};
use xui_core::router::WidgetHost;
use xui_core::{Color, Rect, Theme};

use crate::backend::canvas::Win32Canvas;
use crate::gdi::Paint;
use crate::hwnd::Hwnd;

use crate::message::{CommandNotification, LResult, Message};
use crate::sys;
use crate::window::{Window, WindowHandler};

/// State a window shares with every node handler in it.
pub(crate) struct WindowShared {
    sink: RefCell<Option<Rc<dyn WidgetHost>>>,
    /// Child handles mapped to their node, so a native control's `WM_COMMAND`
    /// (sent to the parent) reaches the widget that owns it.
    nodes: RefCell<HashMap<usize, WidgetId>>,
    /// The window's theme, so a native control's `WM_CTLCOLOR*` is answered
    /// with the right text and background.
    theme: Cell<Theme>,
    /// The brush last returned for `WM_CTLCOLOR*`, kept alive for the control
    /// to paint with.
    brush: RefCell<Option<(Color, HBRUSH)>>,
}

impl WindowShared {
    /// A shared state with no sink installed yet.
    pub(crate) fn new() -> Rc<WindowShared> {
        Rc::new(WindowShared {
            sink: RefCell::new(None),
            nodes: RefCell::new(HashMap::new()),
            theme: Cell::new(Theme::light()),
            brush: RefCell::new(None),
        })
    }

    /// Records the window's theme.
    pub(crate) fn set_theme(&self, theme: Theme) {
        self.theme.set(theme);
    }

    /// The window's theme.
    pub(crate) fn theme(&self) -> Theme {
        self.theme.get()
    }

    /// The brush for a `background` colour, created once and reused.
    fn control_brush(&self, background: Color) -> isize {
        let mut brush = self.brush.borrow_mut();
        if let Some((color, handle)) = brush.as_ref()
            && *color == background
        {
            return handle.0 as isize;
        }
        if let Some((_, old)) = brush.take() {
            sys::gdi::delete_object(HGDIOBJ(old.0));
        }
        match sys::gdi::solid_brush(background) {
            Ok(handle) => {
                let raw = handle.0 as isize;
                *brush = Some((background, handle));
                raw
            }
            Err(_) => 0,
        }
    }

    /// Replaces the event sink.
    pub(crate) fn set_sink(&self, sink: Rc<dyn WidgetHost>) {
        self.sink.replace(Some(sink));
    }

    /// Records that `hwnd` belongs to `id`.
    pub(crate) fn register_node(&self, hwnd: Hwnd, id: WidgetId) {
        self.nodes.borrow_mut().insert(hwnd.raw(), id);
    }

    /// Forgets a child handle.
    pub(crate) fn unregister_node(&self, hwnd: Hwnd) {
        self.nodes.borrow_mut().remove(&hwnd.raw());
    }

    /// Offers `event` to the sink, targeted at `target`.
    pub(crate) fn deliver(&self, target: WidgetId, event: Event) -> bool {
        let sink = self.sink.borrow().clone();
        sink.is_some_and(|sink| sink.deliver(target, &event))
    }
}

/// Maps a decoded [`Message`] to a portable [`Event`], when it is one the
/// front layer models.
fn to_event(message: &Message) -> Option<Event> {
    Some(match message {
        Message::Close => Event::Close,
        Message::Paint => Event::Paint {
            dirty: Rect::default(),
        },
        Message::Size { width, height } => Event::Resize {
            width: *width,
            height: *height,
        },
        Message::DpiChanged { dpi, suggested } => Event::DpiChanged {
            dpi: *dpi,
            suggested: *suggested,
        },
        Message::DisplayChange {
            width,
            height,
            bits_per_pixel,
        } => Event::DisplayChange {
            width: *width,
            height: *height,
            bits_per_pixel: *bits_per_pixel,
        },
        Message::Timer { id } => Event::Timer { id: *id },
        Message::Wake => Event::Wake,
        Message::Activate { active, minimized } => Event::Activate {
            active: *active,
            minimized: *minimized,
        },
        Message::KeyDown {
            key,
            modifiers,
            repeat,
            system,
        } => Event::KeyDown {
            key: *key,
            modifiers: *modifiers,
            repeat: *repeat,
            system: *system,
        },
        Message::KeyUp {
            key,
            modifiers,
            system,
        } => Event::KeyUp {
            key: *key,
            modifiers: *modifiers,
            system: *system,
        },
        Message::Char(character) => Event::Char(*character),
        Message::MouseDown {
            x,
            y,
            button,
            modifiers,
        } => Event::MouseDown {
            x: *x,
            y: *y,
            button: *button,
            modifiers: *modifiers,
        },
        Message::MouseUp {
            x,
            y,
            button,
            modifiers,
        } => Event::MouseUp {
            x: *x,
            y: *y,
            button: *button,
            modifiers: *modifiers,
        },
        Message::MouseMove { x, y, modifiers } => Event::MouseMove {
            x: *x,
            y: *y,
            modifiers: *modifiers,
        },
        Message::MouseDoubleClick {
            x,
            y,
            button,
            modifiers,
        } => Event::MouseDoubleClick {
            x: *x,
            y: *y,
            button: *button,
            modifiers: *modifiers,
        },
        Message::MouseWheel {
            delta,
            horizontal,
            x,
            y,
            modifiers,
        } => Event::MouseWheel {
            delta: *delta,
            horizontal: *horizontal,
            x: *x,
            y: *y,
            modifiers: *modifiers,
        },
        Message::MouseLeave => Event::MouseLeave,
        Message::CaptureChanged => Event::CaptureChanged,
        Message::SetFocus => Event::SetFocus,
        Message::KillFocus => Event::KillFocus,
        _ => return None,
    })
}

/// The handler of a top-level backend window. Window-level events target
/// [`WidgetId::NONE`].
pub(crate) struct TopHandler {
    window: WindowId,
    shared: Rc<WindowShared>,
}

impl TopHandler {
    /// A handler for `window` sharing `shared`.
    pub(crate) fn new(window: WindowId, shared: Rc<WindowShared>) -> TopHandler {
        TopHandler { window, shared }
    }
}

impl WindowHandler for TopHandler {
    fn message(&self, _window: &Window, message: Message) -> Option<LResult> {
        let _ = self.window;
        // A native control asks its parent how to paint its background; answer
        // with the window's theme so a native `EDIT` is dark in dark mode.
        if let Message::Other {
            code,
            wparam,
            lparam,
        } = &message
            && matches!(*code, WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC)
            && *lparam != 0
        {
            let hwnd = Hwnd::from_raw(*lparam as usize);
            if self.shared.nodes.borrow().contains_key(&hwnd.raw()) {
                let theme = self.shared.theme();
                let background = theme.input_background;
                // SAFETY-free: `set_*_color` are safe `sys` wrappers; the DC is
                // only valid for this message.
                let hdc = HDC(*wparam as *mut c_void);
                sys::gdi::set_text_color(hdc, theme.text);
                sys::gdi::set_bk_color(hdc, background);
                return Some(self.shared.control_brush(background));
            }
        }
        // A native control sends its notifications to the parent, so route a
        // child's `WM_COMMAND` to the node that owns it.
        if let Message::Command(command) = &message
            && matches!(
                command.notification,
                CommandNotification::Other(code) if code == EN_CHANGE as u16
            )
            && let Some(control) = command.control
            && let Some(target) = self.shared.nodes.borrow().get(&control.raw()).copied()
        {
            self.shared.deliver(target, Event::TextChanged);
            return Some(0);
        }
        // The class brush erases the top-level window; painting it here would
        // skip that.
        if matches!(&message, Message::Paint) {
            return None;
        }
        let is_close = matches!(&message, Message::Close);
        let event = to_event(&message)?;
        let handled = self.shared.deliver(WidgetId::NONE, event);
        // With no sink to handle it, let the default close the window.
        if is_close && !handled {
            return None;
        }
        Some(0)
    }
}

/// The handler of one node window: it paints the registered painter and
/// forwards input to the front layer.
pub(crate) struct NodeHandler {
    widget: WidgetId,
    shared: Rc<WindowShared>,
    bounds: Rc<Cell<Rect>>,
    painter: Rc<RefCell<Option<Painter>>>,
    tracking_mouse: Cell<bool>,
}

impl NodeHandler {
    /// A handler for `widget`.
    pub(crate) fn new(
        widget: WidgetId,
        shared: Rc<WindowShared>,
        bounds: Rc<Cell<Rect>>,
        painter: Rc<RefCell<Option<Painter>>>,
    ) -> NodeHandler {
        NodeHandler {
            widget,
            shared,
            bounds,
            painter,
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
        if let Some(paint) = Paint::begin(hwnd) {
            let dirty = paint.paint_rect();
            let dpi = sys::dpi::window_dpi(hwnd);
            let mut canvas = Win32Canvas::new(paint.canvas(), self.bounds.get(), dpi);
            painter(&mut canvas);
            dirty
        } else {
            Rect::default()
        }
    }
}

impl WindowHandler for NodeHandler {
    fn message(&self, window: &Window, message: Message) -> Option<LResult> {
        match &message {
            Message::Paint => {
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
