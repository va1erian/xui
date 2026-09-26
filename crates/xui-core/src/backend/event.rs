#![forbid(unsafe_code)]

//! The portable event vocabulary a backend decodes native input into.
//!
//! A backend delivers an [`Event`] to the [`WidgetId`](super::WidgetId) it
//! targets. A native backend usually knows the target; a single-surface backend
//! that paints itself leaves the target to the front layer, which hit-tests.

use crate::geometry::Rect;
use crate::message::{Key, Modifiers, MouseButton};

/// A `SetTimer` identifier. `TimerId(0)` means a backend could not start the
/// timer; every valid id is non-zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimerId(pub usize);

/// One decoded event a backend produces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A mouse button went down.
    MouseDown {
        /// Cursor x in client pixels.
        x: i32,
        /// Cursor y in client pixels.
        y: i32,
        /// Which button.
        button: MouseButton,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// A mouse button was released.
    MouseUp {
        /// Cursor x in client pixels.
        x: i32,
        /// Cursor y in client pixels.
        y: i32,
        /// Which button.
        button: MouseButton,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// The cursor moved.
    MouseMove {
        /// Cursor x in client pixels.
        x: i32,
        /// Cursor y in client pixels.
        y: i32,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// A mouse button was double-clicked.
    MouseDoubleClick {
        /// Cursor x in client pixels.
        x: i32,
        /// Cursor y in client pixels.
        y: i32,
        /// Which button.
        button: MouseButton,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// The wheel was rolled.
    MouseWheel {
        /// Rotation in multiples of the platform notch.
        delta: i16,
        /// Whether this is a horizontal wheel.
        horizontal: bool,
        /// Cursor x in client pixels.
        x: i32,
        /// Cursor y in client pixels.
        y: i32,
        /// Modifiers held.
        modifiers: Modifiers,
    },
    /// The cursor left the widget.
    MouseLeave,
    /// Another window took the mouse capture.
    CaptureChanged,
    /// A key went down.
    KeyDown {
        /// The virtual key.
        key: Key,
        /// Modifiers held.
        modifiers: Modifiers,
        /// Auto-repeat count (`1` on the first press).
        repeat: u16,
        /// Whether this is a system (Alt) combination.
        system: bool,
    },
    /// A key was released.
    KeyUp {
        /// The virtual key.
        key: Key,
        /// Modifiers held.
        modifiers: Modifiers,
        /// Whether this is a system (Alt) combination.
        system: bool,
    },
    /// A translated character.
    Char(char),
    /// A text field's contents changed (a native control's `EN_CHANGE`). The
    /// new text is read back with `Backend::text`.
    TextChanged,
    /// The widget gained the keyboard focus.
    SetFocus,
    /// The widget lost the keyboard focus.
    KillFocus,
    /// The window was activated or deactivated.
    Activate {
        /// Whether the window is now active.
        active: bool,
        /// Whether it is minimised.
        minimized: bool,
    },
    /// The client area changed size, in device pixels.
    Resize {
        /// New client width.
        width: i32,
        /// New client height.
        height: i32,
    },
    /// A repaint is requested; `dirty` is the region to repaint.
    Paint {
        /// The dirty region, in the widget's client pixels.
        dirty: Rect,
    },
    /// A timer started with `set_timer` fired.
    Timer {
        /// The timer that fired.
        id: TimerId,
    },
    /// The window moved to a monitor with a different DPI.
    ///
    /// `dpi` is the new dots-per-inch (96 = 100%). A native backend that can
    /// recommend new bounds — Windows passes them with `WM_DPICHANGED` — puts
    /// them in `suggested`, a device-pixel rectangle in screen coordinates; a
    /// backend with no suggestion (winit reports only the scale factor) passes
    /// [`Rect::default()`].
    ///
    /// The front layer never moves the window itself: it hands the event to the
    /// mapper installed with
    /// [`Ui::on_dpi_changed`](crate::Ui::on_dpi_changed), and the application
    /// decides whether to adopt the suggestion and re-lay-out. A window whose
    /// DPI changed must repaint at the new value; read it back with
    /// [`Backend::dpi`](super::Backend::dpi).
    DpiChanged {
        /// The new dots-per-inch.
        dpi: u32,
        /// The window bounds the backend suggests, in device pixels in screen
        /// coordinates, or [`Rect::default()`] when it has none.
        suggested: Rect,
    },
    /// The desktop resolution or monitor layout changed.
    DisplayChange {
        /// The new screen width in pixels.
        width: u32,
        /// The new screen height in pixels.
        height: u32,
        /// The new colour depth in bits per pixel.
        bits_per_pixel: u32,
    },
    /// The user or code asked to close the window.
    Close,
    /// A menu item with this command id was chosen.
    MenuCommand(u16),
    /// A registered accelerator with this command id fired.
    Accelerator(u16),
    /// A worker has new data (see the message proxy).
    Wake,
}

impl Event {
    /// Whether this event is pointer or keyboard input a widget handles.
    pub const fn is_input(&self) -> bool {
        matches!(
            self,
            Event::MouseDown { .. }
                | Event::MouseUp { .. }
                | Event::MouseMove { .. }
                | Event::MouseDoubleClick { .. }
                | Event::MouseWheel { .. }
                | Event::MouseLeave
                | Event::CaptureChanged
                | Event::KeyDown { .. }
                | Event::KeyUp { .. }
                | Event::Char(_)
                | Event::SetFocus
                | Event::KillFocus
        )
    }

    /// The event's cursor position in client pixels, when it has one.
    pub const fn position(&self) -> Option<(i32, i32)> {
        match *self {
            Event::MouseDown { x, y, .. }
            | Event::MouseUp { x, y, .. }
            | Event::MouseMove { x, y, .. }
            | Event::MouseDoubleClick { x, y, .. }
            | Event::MouseWheel { x, y, .. } => Some((x, y)),
            _ => None,
        }
    }
}
