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
mod event;
mod ids;
mod node;

pub use canvas::{Canvas, TextAlign, TextMetrics, TextStyle, TextWeight};
pub use event::{Event, TimerId};
pub use ids::{WidgetId, WindowId};
pub use node::{ImplKind, NodeKind, NodeOptions, NodeSpec, ParentRef};

use std::fmt;

use crate::geometry::Rect;
use crate::theme::Theme;
use crate::units::Dip;

/// A backend operation's result.
pub type Result<T> = std::result::Result<T, BackendError>;

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

/// The portable part of a top-level window's spec.
///
/// Backend-specific extras (backdrop materials, an extended title bar, …) are
/// not here; a backend exposes them through its own extension trait.
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
}

impl PlatformSpec {
    /// A resizable window of a default size.
    pub fn new(title: impl Into<String>) -> PlatformSpec {
        PlatformSpec {
            title: title.into(),
            width: Dip(640.0),
            height: Dip(480.0),
            resizable: true,
        }
    }

    /// Sets the initial client size.
    pub fn size(mut self, width: Dip, height: Dip) -> PlatformSpec {
        self.width = width;
        self.height = height;
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

    /// Creates a top-level window.
    fn open_window(&self, spec: &PlatformSpec) -> Result<WindowId>;

    /// Destroys a top-level window and every node in it.
    fn close_window(&self, window: WindowId);

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

    /// Gives a node the keyboard focus.
    fn focus(&self, id: WidgetId);

    /// Replaces a node's text.
    fn set_text(&self, id: WidgetId, text: &str);

    /// Schedules a repaint of a node's whole area.
    fn invalidate(&self, id: WidgetId);

    /// Schedules a repaint of `rect` within a node.
    fn invalidate_rect(&self, id: WidgetId, rect: Rect);

    /// Paints a node by handing the front layer a short-lived canvas.
    fn paint(&self, id: WidgetId, dirty: Rect, paint: &mut dyn FnMut(&mut dyn Canvas));

    /// Measures a run of text in device pixels.
    fn measure_text(&self, text: &str, style: &TextStyle, dpi: u32) -> TextMetrics;

    /// A window's dots-per-inch.
    fn dpi(&self, window: WindowId) -> u32;

    /// A window's client area, in device pixels.
    fn client_rect(&self, window: WindowId) -> Rect;

    /// Applies a theme to a window and its nodes.
    fn set_theme(&self, window: WindowId, theme: &Theme);

    /// Starts a repeating timer on `window` and returns its id.
    fn set_timer(&self, window: WindowId, millis: u32) -> TimerId;

    /// Stops a timer started with [`Backend::set_timer`].
    fn kill_timer(&self, window: WindowId, id: TimerId);

    /// Whether the backend provides a native widget for `kind`, or expects the
    /// front layer to paint it.
    fn supports(&self, kind: NodeKind) -> ImplKind;
}
