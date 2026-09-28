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
    /// A horizontal (west-east) resize arrow, for dragging a vertical divider.
    SizeHorizontal,
    /// A vertical (north-south) resize arrow, for dragging a horizontal
    /// divider.
    SizeVertical,
    /// A diagonal (north-west to south-east) resize arrow, for a top-left or
    /// bottom-right corner handle.
    SizeNwSe,
    /// A diagonal (north-east to south-west) resize arrow, for a top-right or
    /// bottom-left corner handle.
    SizeNeSw,
}
