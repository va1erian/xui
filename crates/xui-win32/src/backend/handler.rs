#![forbid(unsafe_code)]

//! The window handlers the Win32 backend installs: one for a top-level window,
//! one for each node. They decode the typed [`Message`] into a portable
//! [`Event`] and deliver it to the window's [`WidgetHost`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Graphics::Gdi::{HBRUSH, HDC, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{
    EN_CHANGE, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_MOVE,
};

use xui_core::backend::{Event, Painter, WidgetId, WindowId};
use xui_core::router::WidgetHost;
use xui_core::{Color, Point, Rect, Theme};

use crate::backend::canvas::Win32Canvas;
use crate::gdi::Paint;
use crate::hwnd::Hwnd;

use crate::message::{CommandNotification, LResult, Message, MouseButton};
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
    /// The node that owns the host's keyboard input while the OS focus is on
    /// the host window itself: a transient popup, whose top-level window must
    /// not take activation. `None` leaves input to the host.
    key_focus: RefCell<Option<WidgetId>>,
    /// The host window's handle, so an open popup can be re-pinned to it.
    host: Cell<Hwnd>,
    /// Open popups by handle, with their host-client rectangles, so they
    /// follow the host window when it moves.
    popups: RefCell<HashMap<usize, Rect>>,
}

impl WindowShared {
    /// A shared state with no sink installed yet.
    pub(crate) fn new() -> Rc<WindowShared> {
        Rc::new(WindowShared {
            sink: RefCell::new(None),
            nodes: RefCell::new(HashMap::new()),
            theme: Cell::new(Theme::light()),
            brush: RefCell::new(None),
            key_focus: RefCell::new(None),
            host: Cell::new(Hwnd::NULL),
            popups: RefCell::new(HashMap::new()),
        })
    }

    /// Records the host window this shared state belongs to.
    pub(crate) fn set_host(&self, hwnd: Hwnd) {
        self.host.set(hwnd);
    }

    /// Records an open popup's host-client rectangle.
    pub(crate) fn track_popup(&self, hwnd: Hwnd, rect: Rect) {
        self.popups.borrow_mut().insert(hwnd.raw(), rect);
    }

    /// Forgets a popup, so a moved host no longer repositions it.
    pub(crate) fn forget_popup(&self, hwnd: Hwnd) {
        self.popups.borrow_mut().remove(&hwnd.raw());
    }

    /// Re-pins every open popup to the host after the host moved.
    pub(crate) fn reposition_popups(&self) {
        let host = self.host.get();
        if host.is_null() {
            return;
        }
        let moves: Vec<(Hwnd, Rect)> = self
            .popups
            .borrow()
            .iter()
            .map(|(raw, rect)| {
                let at =
                    crate::sys::window::client_to_screen(host, Point::new(rect.left, rect.top));
                (
                    Hwnd::from_raw(*raw),
                    Rect::new(at.x, at.y, at.x + rect.width(), at.y + rect.height()),
                )
            })
            .collect();
        crate::sys::layout::apply(&moves);
    }

    /// Sets the node that owns the host's keyboard input, if any.
    pub(crate) fn set_key_focus(&self, id: Option<WidgetId>) {
        *self.key_focus.borrow_mut() = id;
    }

    /// The node that owns the host's keyboard input while the OS focus is on
    /// the host window, if any.
    pub(crate) fn key_focus(&self) -> Option<WidgetId> {
        *self.key_focus.borrow()
    }

    /// Clears the host's keyboard focus when it is `id`.
    pub(crate) fn clear_key_focus(&self, id: WidgetId) {
        let mut focus = self.key_focus.borrow_mut();
        if *focus == Some(id) {
            *focus = None;
        }
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
        // A popup is pinned to the host's client area, so move the open popups
        // with the host when it is dragged. `WM_MOVE` reports the new origin.
        if let Message::Other { code, .. } = &message
            && *code == WM_MOVE
        {
            self.shared.reposition_popups();
        }
        let is_close = matches!(&message, Message::Close);
        let event = to_event(&message)?;
        // A transient popup never takes activation, so the OS focus stays on
        // the host; forward the host's keyboard input to the popup the app
        // focused, the way a native menu keeps its window active. Everything
        // else is window-level.
        let target = if is_keyboard(&event) {
            self.shared.key_focus().unwrap_or(WidgetId::NONE)
        } else {
            WidgetId::NONE
        };
        let handled = self.shared.deliver(target, event);
        // With no sink to handle it, let the default close the window.
        if is_close && !handled {
            return None;
        }
        Some(0)
    }
}

/// Whether `event` is keyboard input that must reach the app's logical focus
/// even while the OS focus is on the host window.
fn is_keyboard(event: &Event) -> bool {
    matches!(
        event,
        Event::KeyDown { .. } | Event::KeyUp { .. } | Event::Char(_)
    )
}

/// The handler of one node window: it paints the registered painter and
/// forwards input to the front layer.
pub(crate) struct NodeHandler {
    widget: WidgetId,
    shared: Rc<WindowShared>,
    bounds: Rc<Cell<Rect>>,
    painter: Rc<RefCell<Option<Painter>>>,
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
        painter: Rc<RefCell<Option<Painter>>>,
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
