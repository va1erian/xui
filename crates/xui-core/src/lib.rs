#![forbid(unsafe_code)]

//! The backend-agnostic core of xui.
//!
//! Everything a widget or a backend needs that is not tied to a platform lives
//! here: pixel geometry, typed length units, colour, the pure layout
//! arithmetic, the semantic theme tokens, the input vocabulary and the
//! accessibility tree model. The crate has no platform dependency and no
//! `unsafe`, so every backend can share these types.

pub mod accessibility;
pub mod color;
pub mod geometry;
pub mod layout;
pub mod message;
pub mod theme;
pub mod units;

pub use color::Color;
pub use geometry::{Point, Rect, Size};
pub use layout::{Dock, DockLayout, Insets, Stack, StackDirection, StackSlot};
pub use message::{HitTest, Key, Modifiers, MouseButton};
pub use theme::{Theme, Themed};
pub use units::{Dip, Px, dip};

/// The core types a frontend or a backend usually needs, in one `use`.
pub mod prelude {
    pub use crate::color::prelude::*;
    pub use crate::geometry::prelude::*;
    pub use crate::layout::prelude::*;
    pub use crate::message::{HitTest, Key, Modifiers, MouseButton};
    pub use crate::theme::{Theme, Themed};
    pub use crate::units::prelude::*;
}
