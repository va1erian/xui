#![forbid(unsafe_code)]

//! The contract a backend implements, and the portable vocabulary the front
//! layer uses to drive it.
//!
//! The front layer never names a platform handle. It refers to a window by a
//! [`WindowId`] and a widget by a [`WidgetId`], both assigned by the backend,
//! and describes what to create with a [`NodeSpec`]. Events arrive as a
//! portable [`Event`] and are routed by a [`Router`](crate::router::Router).
//!
//! This contract grows as controls are ported; it deliberately covers the
//! operations the widget layer needs today rather than a speculative full API.

mod canvas;
mod cursor;
mod event;
mod ids;
mod node;
mod paint;
mod text;

#[cfg(test)]
pub(crate) mod headless;

pub use canvas::{Canvas, TextAlign, TextMetrics, TextStyle, TextVAlign, TextWeight};
pub use cursor::Cursor;
pub use event::{Event, TimerId};
pub use ids::{WidgetId, WindowId};
pub use node::{ImplKind, NodeKind, NodeOptions, NodeSpec, ParentRef};
pub use paint::{Cap, Corner, Dash, GradientStop, LinearGradient, RadialGradient, Rgba, Stroke};
pub use text::{FontSpec, TextHit, TextLayout, TextShaper};

use std::fmt;
use std::rc::Rc;

use crate::geometry::Rect;
use crate::router::WidgetHost;
use crate::theme::Theme;
use crate::units::Dip;

/// A backend operation's result.
pub type Result<T> = std::result::Result<T, BackendError>;

/// A painted widget's draw routine. The front layer registers one per node;
/// the backend invokes it whenever the node needs painting, handing it a
/// short-lived [`Canvas`].
pub type Painter = Rc<dyn Fn(&mut dyn Canvas)>;

/// A thread-safe wake for one window: calling it makes the backend deliver
/// [`Event::Wake`], as a worker thread needs. `Send + Sync`, unlike the
/// backend itself.
pub type Waker = Box<dyn Fn() + Send + Sync>;

/// Why a backend operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    /// The backend cannot provide the requested kind or operation.
    Unsupported(&'static str),
    /// The backend could not create the window or node.
    CreateFailed(&'static str),
    /// Anything else, described by the backend.
    Other(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::Unsupported(what) => write!(f, "unsupported: {what}"),
            BackendError::CreateFailed(what) => write!(f, "could not create {what}"),
            BackendError::Other(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for BackendError {}

/// Whether a window shows the platform's system title bar and border.
///
/// Set with [`PlatformSpec::decorations`]. [`Decorations::None`] removes the
/// system title bar so the application can draw its own. A backend that
/// supports an extended title bar (Win32) keeps the native window buttons and
/// resize borders, while one that does not leaves the whole frame to the
/// application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Decorations {
    /// The system title bar, border and window buttons (the default).
    #[default]
    System,
    /// No system title bar: the application draws its own.
    None,
}

/// The material a backend draws behind a window's client area.
///
/// Set with [`PlatformSpec::backdrop`]. A backend without the material, or one
/// where the platform rejects it, falls back to the opaque theme background —
/// exactly as on Windows when DWM declines the request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backdrop {
    /// An opaque theme background (the default).
    #[default]
    Opaque,
    /// A translucent, blurred material (Windows Acrylic).
    Acrylic,
    /// A desktop-tinted material (Windows Mica).
    Mica,
}

/// The portable part of a top-level window's spec.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformSpec {
    /// The window title.
    pub title: String,
    /// The initial client width as a design value.
    pub width: Dip,
    /// The initial client height as a design value.
    pub height: Dip,
    /// Whether the user may resize the window.
    pub resizable: bool,
    /// Whether the window shows the platform's system title bar.
    pub decorations: Decorations,
    /// The height of a custom caption band reserved at the top of the client
    /// area, as a design value. Zero (the default) when the application draws
    /// no caption. Read the band a backend actually reserved with
    /// [`Backend::caption_inset`].
    pub caption_inset: Dip,
    /// The material behind the client area.
    pub backdrop: Backdrop,
}

impl PlatformSpec {
    /// A resizable window of a default size.
    pub fn new(title: impl Into<String>) -> PlatformSpec {
        PlatformSpec {
            title: title.into(),
            width: Dip(640.0),
            height: Dip(480.0),
            resizable: true,
            decorations: Decorations::System,
            caption_inset: Dip(0.0),
            backdrop: Backdrop::Opaque,
        }
    }

    /// Sets the initial client size.
    pub fn size(mut self, width: Dip, height: Dip) -> PlatformSpec {
        self.width = width;
        self.height = height;
        self
    }

    /// Hides the system title bar so the application draws its own.
    pub fn decorations(mut self, decorations: Decorations) -> PlatformSpec {
        self.decorations = decorations;
        self
    }

    /// Reserves a custom caption band of `height` at the top of the client
    /// area, so a title bar can sit in the title area.
    pub fn caption_inset(mut self, height: Dip) -> PlatformSpec {
        self.caption_inset = height;
        self
    }

    /// Selects the material behind the client area.
    pub fn backdrop(mut self, backdrop: Backdrop) -> PlatformSpec {
        self.backdrop = backdrop;
        self
    }
}

/// A platform backend: the event loop, window and node operations, painting
/// and text.
///
/// One backend value drives one event loop on the thread that owns the UI.
/// Object-safe, so the front layer can hold it behind a `dyn Backend`.
pub trait Backend {
    /// One-time process initialisation (DPI awareness, common-control classes).
    /// Idempotent; the default does nothing.
    fn init(&self) {}

    /// Runs the event loop until [`Backend::quit`] is called, returning the
    /// exit code.
    fn run(&self) -> i32;

    /// Ends the event loop with `code`.
    fn quit(&self, code: i32);

    /// Wakes the loop for `window` after a worker posted data.
    fn wake(&self, window: WindowId);

    /// A thread-safe handle that wakes `window` from another thread. A worker
    /// uses one to hand messages back (see the message proxy).
    fn waker(&self, window: WindowId) -> Waker;

    /// Installs the sink the backend delivers decoded [`Event`]s to for
    /// `window`. Replaces any previous sink.
    ///
    /// Window-level events that belong to no node (close, timers, DPI and
    /// display changes, activation, wake) target [`WidgetId::NONE`].
    fn set_event_sink(&self, window: WindowId, sink: Rc<dyn WidgetHost>);

    /// Creates a top-level window.
    fn open_window(&self, spec: &PlatformSpec) -> Result<WindowId>;

    /// Destroys a top-level window and every node in it.
    fn close_window(&self, window: WindowId);

    /// Minimizes `window` to the taskbar. A backend that cannot minimize a
    /// window does nothing.
    fn minimize(&self, window: WindowId) {
        let _ = window;
    }

    /// Toggles `window` between maximized and its restored bounds. A backend
    /// that cannot maximize a window does nothing.
    fn toggle_maximize(&self, window: WindowId) {
        let _ = window;
    }

    /// Whether `window` is currently maximized.
    fn is_maximized(&self, window: WindowId) -> bool {
        let _ = window;
        false
    }

    /// The height a backend reserves at the top of `window`'s client area for a
    /// custom caption, in design units. Zero for a window with a system title
    /// bar, or one whose backend reserves no caption band.
    fn caption_inset(&self, window: WindowId) -> Dip {
        let _ = window;
        Dip(0.0)
    }

    /// Creates a node inside `parent`, which is the window or a container node.
    fn create(&self, parent: ParentRef, spec: &NodeSpec) -> Result<WidgetId>;

    /// Destroys a node and every node created inside it. The front layer
    /// destroys a container before its children, but a backend must cascade.
    fn destroy(&self, id: WidgetId);

    /// Moves several nodes as one batch, so a relayout does not flicker. The
    /// nodes may belong to different parents; the backend batches per parent.
    fn apply_moves(&self, window: WindowId, moves: &[(WidgetId, Rect)]);

    /// Shows or hides a node.
    fn set_visible(&self, id: WidgetId, visible: bool);

    /// Enables or disables a node.
    fn set_enabled(&self, id: WidgetId, enabled: bool);

    /// Raises a node above its siblings in the z-order. A backend with no
    /// overlapping children may ignore it.
    fn raise(&self, id: WidgetId) {
        let _ = id;
    }

    /// Marks a node's area as a window-drag region: a left-button press that
    /// begins there moves the whole window instead of reaching the node. A
    /// custom title bar sets this on its empty area. A backend that cannot move
    /// a window by region, or a window with a system title bar, ignores it.
    fn set_drag_region(&self, id: WidgetId, drag: bool) {
        let _ = (id, drag);
    }

    /// Requests the pointer shape shown over a node. A backend that cannot
    /// change cursors may ignore it.
    fn set_cursor(&self, id: WidgetId, cursor: Cursor) {
        let _ = (id, cursor);
    }

    /// Gives a node the keyboard focus.
    fn focus(&self, id: WidgetId);

    /// Replaces a node's text.
    fn set_text(&self, id: WidgetId, text: &str);

    /// A node's current text; empty for a node that has none. A native control
    /// answers from its own state.
    fn text(&self, id: WidgetId) -> String {
        let _ = id;
        String::new()
    }

    /// A node's current bounds, in device pixels.
    fn bounds(&self, id: WidgetId) -> Rect {
        let _ = id;
        Rect::default()
    }

    /// Schedules a repaint of a node's whole area.
    fn invalidate(&self, id: WidgetId);

    /// Schedules a repaint of `rect` within a node.
    fn invalidate_rect(&self, id: WidgetId, rect: Rect);

    /// Registers the draw routine for a painted node, replacing any previous
    /// one. The backend invokes it on paint; a native node ignores it.
    fn set_painter(&self, id: WidgetId, painter: Painter);

    /// Measures a run of text in device pixels.
    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics;

    /// A `Send + Sync` handle that shapes text for this backend.
    ///
    /// Obtain one on the UI thread and use it from a worker: the handle shapes,
    /// hit-tests and selects, and the layout it returns is `Send + Sync`, so a
    /// shaped document can be sent back and drawn with
    /// [`Canvas::draw_layout`](canvas::Canvas::draw_layout). A backend that
    /// cannot shape returns a handle that produces empty layouts.
    fn text_shaper(&self) -> Box<dyn TextShaper> {
        Box::new(text::UnsupportedShaper)
    }

    /// Shapes `text` at `max_width` device pixels (or `f32::INFINITY` for one
    /// unwrapped line) at `dpi`.
    fn layout_text(
        &self,
        text: &str,
        spec: &FontSpec,
        max_width: f32,
        dpi: u32,
    ) -> Box<dyn TextLayout> {
        self.text_shaper().layout(text, spec, max_width, dpi)
    }

    /// A window's dots-per-inch.
    fn dpi(&self, window: WindowId) -> u32;

    /// A window's client area, in device pixels.
    fn client_rect(&self, window: WindowId) -> Rect;

    /// Applies a theme to a window and its nodes.
    fn set_theme(&self, window: WindowId, theme: &Theme);

    /// Starts a repeating timer on `window` and returns its id, or
    /// `TimerId(0)` when the backend could not start one.
    fn set_timer(&self, window: WindowId, millis: u32) -> TimerId;

    /// Stops a timer started with [`Backend::set_timer`].
    fn kill_timer(&self, window: WindowId, id: TimerId);

    /// Whether the backend provides a native widget for `kind`, or expects the
    /// front layer to paint it.
    fn supports(&self, kind: NodeKind) -> ImplKind;
}

#[cfg(test)]
mod tests {
    use super::{Backdrop, Decorations, PlatformSpec};
    use crate::units::dip;

    #[test]
    fn a_new_window_is_decorated_and_opaque_by_default() {
        let spec = PlatformSpec::new("t");
        assert_eq!(spec.decorations, Decorations::System);
        assert_eq!(spec.caption_inset, dip(0.0));
        assert_eq!(spec.backdrop, Backdrop::Opaque);
    }

    #[test]
    fn builder_sets_the_portable_chrome_options() {
        let spec = PlatformSpec::new("t")
            .decorations(Decorations::None)
            .caption_inset(dip(36.0))
            .backdrop(Backdrop::Mica);
        assert_eq!(spec.decorations, Decorations::None);
        assert_eq!(spec.caption_inset, dip(36.0));
        assert_eq!(spec.backdrop, Backdrop::Mica);
    }
}
