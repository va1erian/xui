#![forbid(unsafe_code)]

//! Opaque, backend-assigned handles.
//!
//! The front layer never sees a platform handle (`HWND`, a `winit` window, …);
//! it names a window with a [`WindowId`] and a widget (a child node) with a
//! [`WidgetId`]. A backend assigns the values and owns their meaning.

/// Identifies a top-level window created by a [`Backend`](super::Backend).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(u64);

/// Identifies a widget node created by a [`Backend`](super::Backend) inside a
/// window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(u64);

macro_rules! id {
    ($name:ident) => {
        impl $name {
            /// The null id, held before a backend assigns one.
            pub const NONE: $name = $name(0);

            /// Wraps a backend-assigned raw value.
            pub const fn from_raw(value: u64) -> $name {
                $name(value)
            }

            /// The raw value a backend assigned.
            pub const fn raw(self) -> u64 {
                self.0
            }

            /// Whether this is the null id.
            pub const fn is_none(self) -> bool {
                self.0 == 0
            }
        }
    };
}

id!(WindowId);
id!(WidgetId);
