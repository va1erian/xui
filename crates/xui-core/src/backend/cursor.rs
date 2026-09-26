#![forbid(unsafe_code)]

//! The pointer shapes a widget can ask the backend to show over its area.

/// A pointer shape a widget requests while the pointer is over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    /// The platform's default arrow.
    #[default]
    Default,
    /// A hand, for a link.
    Hand,
    /// An I-beam, for editable text.
    Text,
}
