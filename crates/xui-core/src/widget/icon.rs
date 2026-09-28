#![forbid(unsafe_code)]

//! The legacy [`Icon`] vocabulary: a small button-oriented set that now maps
//! onto the generated [`Lucide`] icons, so old callers keep drawing the same
//! shapes. New code can name a Lucide icon directly; see
//! [`crate::icon`].

use super::lucide::Lucide;

/// A button-appropriate icon.
///
/// This is the original, deliberately small set; each variant resolves to a
/// generated [`Lucide`] outline of the same shape. Prefer the full
/// [`Lucide`] set in new code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A plus sign (add / new).
    Plus,
    /// A minus sign (remove).
    Minus,
    /// A cross (close / dismiss).
    Close,
    /// A tick (confirm / done).
    Check,
    /// A downward chevron (expand).
    ChevronDown,
    /// An upward chevron (collapse).
    ChevronUp,
    /// A magnifier (search).
    Search,
    /// Three dots (more actions).
    More,
}

impl Icon {
    /// The generated Lucide outline this icon draws.
    pub(crate) fn lucide(self) -> Lucide {
        match self {
            Icon::Plus => Lucide::Plus,
            Icon::Minus => Lucide::Minus,
            Icon::Close => Lucide::X,
            Icon::Check => Lucide::Check,
            Icon::ChevronDown => Lucide::ChevronDown,
            Icon::ChevronUp => Lucide::ChevronUp,
            Icon::Search => Lucide::Search,
            Icon::More => Lucide::Ellipsis,
        }
    }
}
