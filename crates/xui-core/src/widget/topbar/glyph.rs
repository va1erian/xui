#![forbid(unsafe_code)]

//! The portable [`Glyph`] icon vocabulary, shared by the [`TopBar`] and the
//! [`TreeView`](crate::widget::TreeView).
//!
//! [`TopBar`]: super::TopBar

/// A dependency-free icon.
///
/// The set is deliberately small: it carries the bar without an image
/// dependency (see issue #35) and the tree without one either, drawing each
/// shape with the portable [`Canvas`](crate::backend::Canvas) and any other
/// short mark as a text glyph. The transport shapes
/// ([`Play`](Glyph::Play), [`Pause`](Glyph::Pause), [`Stop`](Glyph::Stop),
/// [`Previous`](Glyph::Previous), [`Next`](Glyph::Next),
/// [`Repeat`](Glyph::Repeat), [`Shuffle`](Glyph::Shuffle)) are drawn as
/// vectors, so a media bar needs no icon font and renders the same on every
/// backend. A caller that needs a richer set can pass text, e.g.
/// `Glyph::Text("+")`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// Three stacked bars (a menu).
    Menu,
    /// A magnifier (search).
    Search,
    /// An X (close).
    Close,
    /// Three dots (more).
    More,
    /// An outlined five-pointed star (a favourite).
    Star,
    /// A right-pointing triangle (play).
    Play,
    /// Two vertical bars (pause).
    Pause,
    /// A filled square (stop).
    Stop,
    /// A bar and a left-pointing triangle (previous track).
    Previous,
    /// A right-pointing triangle and a bar (next track).
    Next,
    /// A loop with two arrow heads (repeat).
    Repeat,
    /// Two crossing arrows (shuffle).
    Shuffle,
    /// A speaker (audio / a music library).
    Audio,
    /// A disc with a spindle hole (an album).
    Album,
    /// Two people (artists).
    People,
    /// A tag (genres).
    Tag,
    /// A folder (a file browser).
    Folder,
    /// A filled five-pointed star (a favourite, filled).
    StarFilled,
    /// A clock face with hands (history).
    History,
    /// A screen on a stand (a visualisation).
    Monitor,
    /// A gear (settings).
    Settings,
    /// A short text run drawn as the icon, e.g. `"+"` or `"A"`.
    Text(&'static str),
}
