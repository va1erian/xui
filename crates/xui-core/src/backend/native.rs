#![forbid(unsafe_code)]

//! A portable, opaque view of a backend's native window handle.

/// An opaque native window handle.
///
/// A backend that can expose one returns it from
/// [`Backend::native_window`](super::Backend::native_window), so an application
/// can hand it to an OS integration (COM, the taskbar, a raw-message hook)
/// without naming a platform type in its portable code. The meaning of
/// [`NativeWindowHandle::raw`] is a platform contract: on Windows it is the
/// `HWND` value, on another backend it may be a display-server window id, and a
/// backend with no stable handle returns `None` instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeWindowHandle(usize);

impl NativeWindowHandle {
    /// Wraps a raw platform handle value.
    pub const fn from_raw(value: usize) -> NativeWindowHandle {
        NativeWindowHandle(value)
    }

    /// The raw platform handle value.
    pub const fn raw(self) -> usize {
        self.0
    }

    /// Whether this is the null handle.
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}
