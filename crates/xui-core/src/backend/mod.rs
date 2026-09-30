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
mod clipboard;
mod cursor;
mod dialog;
mod event;
mod ids;
mod native;
mod node;
mod paint;
mod path;
mod spec;
mod text;

#[cfg(test)]
pub(crate) mod headless;

pub use canvas::{Canvas, TextAlign, TextMetrics, TextStyle, TextVAlign, TextWeight};
pub use cursor::Cursor;
pub use dialog::{FileDialogMode, FileDialogOutcome, FileDialogRequest, FileFilter};
pub use event::{Event, TimerId};
pub use ids::{WidgetId, WindowId};
pub use native::NativeWindowHandle;
pub use node::{ImplKind, NodeKind, NodeOptions, NodeSpec, ParentRef};
pub use paint::{
    Cap, Corner, Dash, GradientStop, Join, LinearGradient, PathGradient, RadialGradient, Rgba,
    Stroke,
};
pub use path::{PathPlacement, PathSeg, Polyline, flatten};
pub use spec::{Backdrop, Decorations, PlatformSpec};
pub use text::{FontSpec, TextHit, TextLayout, TextShaper};

use std::fmt;
use std::rc::Rc;

use crate::geometry::Rect;
use crate::image::Image;
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

    /// Runs `on_ready` once `window`'s platform window exists and its DPI is
    /// known, then pumps events until quit, returning the exit code.
    ///
    /// A backend that creates its window synchronously (Win32) leaves the
    /// default, which calls `on_ready` and then [`Backend::run`]. A backend
    /// whose window is created lazily by the event loop — `winit`, where
    /// `open_window` only records the request — overrides this so the app is
    /// built after the window is live and lays out at the real DPI.
    fn run_with(&self, window: WindowId, on_ready: &mut dyn FnMut()) -> i32 {
        let _ = window;
        on_ready();
        self.run()
    }

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

    /// Replaces a top-level window's title. A backend that cannot retitle a
    /// window does nothing.
    fn set_window_title(&self, window: WindowId, title: &str) {
        let _ = (window, title);
    }

    /// Sets a top-level window's icon (title bar, task bar, window switcher)
    /// from an RGBA image. A backend without window icons does nothing; one
    /// that opens its window lazily keeps the icon until the window exists.
    fn set_window_icon(&self, window: WindowId, icon: &Image) {
        let _ = (window, icon);
    }

    /// Enables or disables a whole top-level window, so a modal dialog can make
    /// its owner inert. A backend that cannot disable a window does nothing.
    fn set_window_enabled(&self, window: WindowId, enabled: bool) {
        let _ = (window, enabled);
    }

    /// The backend's native handle for `window`, for an application's OS
    /// integrations. `None` when the backend has no stable handle to expose.
    fn native_window(&self, window: WindowId) -> Option<NativeWindowHandle> {
        let _ = window;
        None
    }

    /// Renders `window`'s current content into a portable RGBA image. A backend
    /// that cannot capture reports [`BackendError::Unsupported`].
    fn capture(&self, window: WindowId) -> Result<Image> {
        let _ = window;
        Err(BackendError::Unsupported("window capture"))
    }

    /// Runs `window` modally: pumps the backend's events until `window` closes,
    /// blocking the caller. The opener should first be disabled with
    /// [`Backend::set_window_enabled`]. A backend that cannot run a nested loop
    /// reports [`BackendError::Unsupported`].
    fn run_modal(&self, window: WindowId) -> Result<()> {
        let _ = window;
        Err(BackendError::Unsupported("modal window"))
    }

    /// Shows the platform's own file picker for `window`, if the backend has
    /// one. The default declines, so the portable
    /// [`FileDialog`](crate::widget::FileDialog) covers the request instead;
    /// a backend with a native picker (the Win32 Common Item Dialog, a desktop
    /// portal) returns [`FileDialogOutcome::Chosen`] or
    /// [`FileDialogOutcome::Cancelled`]. The application never sees which
    /// picker answered.
    fn file_dialog(&self, window: WindowId, request: &FileDialogRequest) -> FileDialogOutcome {
        let _ = (window, request);
        FileDialogOutcome::Declined
    }

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

    /// Clips a node's descendants to `rect`, in the node's own coordinate
    /// space (relative to its top-left). `None` clears the clip. The node's own
    /// painting is not clipped. A backend whose painted nodes are native child
    /// windows already clips each child to its parent and may ignore it.
    fn set_clip(&self, id: WidgetId, rect: Option<Rect>) {
        let _ = (id, rect);
    }

    /// Routes subsequent pointer move and release events to `id`, even when the
    /// pointer leaves the node, so a drag survives the pointer leaving it. A
    /// backend that cannot capture may ignore it.
    fn set_capture(&self, id: WidgetId) {
        let _ = id;
    }

    /// Releases the pointer capture taken with [`Backend::set_capture`]. A
    /// backend that does not capture may ignore it.
    fn release_capture(&self) {}

    /// Gives a node the keyboard focus.
    fn focus(&self, id: WidgetId);

    /// Replaces a node's text.
    fn set_text(&self, id: WidgetId, text: &str);

    /// Sets a node's cue banner: the placeholder a text field shows while it
    /// is empty. A backend hosting a native control forwards it (the Win32
    /// `EDIT`'s `EM_SETCUEBANNER`); a painted field draws its own cue, so the
    /// default does nothing.
    fn set_cue(&self, id: WidgetId, cue: &str) {
        let _ = (id, cue);
    }

    /// A node's current text; empty for a node that has none. A native control
    /// answers from its own state.
    fn text(&self, id: WidgetId) -> String {
        let _ = id;
        String::new()
    }

    /// Reads the clipboard's text, or `None` when it holds none (or holds no
    /// text at all, such as an image only).
    ///
    /// The default is an in-process store, so a backend with no OS clipboard
    /// — the headless and offscreen backends, a minimal implementation — still
    /// supports copy and paste. Win32 and the canvas backend override it with
    /// the real clipboard.
    fn clipboard_text(&self) -> Option<String> {
        clipboard::get()
    }

    /// Replaces the clipboard's text. See [`Backend::clipboard_text`] on the
    /// default in-process store.
    fn set_clipboard_text(&self, text: &str) {
        clipboard::set(text);
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
