#![forbid(unsafe_code)]

//! Semantic theme tokens ([`Theme`]) and the [`Themed`] trait every widget
//! implements. Both are pure data, so a backend owns only the platform side of
//! applying a theme.

pub mod look;
mod midnight;
mod themed;
mod tokens;

pub use themed::Themed;
pub use tokens::Theme;

/// The theming types a frontend usually needs.
pub mod prelude {
    pub use super::{Theme, Themed};
}
