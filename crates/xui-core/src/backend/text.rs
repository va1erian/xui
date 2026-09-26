#![forbid(unsafe_code)]

//! Portable text layout: a font request, a shaped layout, and the hit-testing
//! and selection geometry a selectable document needs.
//!
//! The same shaped value measures, hit-tests and draws, so a document that
//! lays text out on a worker thread sees the widths and glyph positions that
//! get painted. Backends implement [`TextShaper`] on a `Send + Sync` handle
//! ([`Backend::text_shaper`]) that can be used off the UI thread; a shaped
//! [`TextLayout`] is `Send + Sync` too, so it can be sent back to the UI thread
//! and drawn there through [`Canvas::draw_layout`].
//!
//! A trait cannot name a backend's concrete layout type, so a layout exposes
//! [`TextLayout::as_any`]; only the backend's own canvas needs it, to recover
//! the type it shaped with.
//!
//! [`Backend::text_shaper`]: super::Backend::text_shaper
//! [`Canvas::draw_layout`]: super::Canvas::draw_layout

use std::any::Any;

use super::canvas::TextWeight;
use crate::geometry::Rect;
use crate::units::Dip;

/// A font request: the family, size, weight and slant to shape and draw with.
///
/// `family` is `None` for the backend's default UI font, matching
/// [`TextStyle`](super::TextStyle).
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    /// The font family, or `None` for the backend's default UI font.
    pub family: Option<String>,
    /// The font size as a design value.
    pub size: Dip,
    /// The face weight.
    pub weight: TextWeight,
    /// Whether the face is slanted.
    pub italic: bool,
}

impl FontSpec {
    /// A regular-weight, upright run of `size` in the default family.
    pub fn new(size: Dip) -> FontSpec {
        FontSpec {
            family: None,
            size,
            weight: TextWeight::REGULAR,
            italic: false,
        }
    }

    /// Sets the family, or `None` for the backend's default UI font.
    pub fn family(mut self, family: impl Into<String>) -> FontSpec {
        self.family = Some(family.into());
        self
    }

    /// Sets the weight (100-900).
    pub fn weight(mut self, weight: u16) -> FontSpec {
        self.weight = TextWeight::new(weight);
        self
    }

    /// Sets whether the face is slanted.
    pub fn italic(mut self, italic: bool) -> FontSpec {
        self.italic = italic;
        self
    }
}

/// Where a point falls in a shaped layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextHit {
    /// The byte offset of the nearest caret position, always on a `char`
    /// boundary. Before the character hit when the point is on its leading
    /// half, after it otherwise.
    pub byte_index: usize,
    /// Whether the point is inside the text rather than beside or past it.
    pub inside: bool,
}

/// A shaped, wrapped run of text, in device pixels relative to its top-left
/// corner.
///
/// Byte offsets are UTF-8 offsets into the laid-out `&str`; offsets past the
/// end clamp to it and offsets inside a character snap to its start. A layout
/// is `Send + Sync`, so it can be shaped off-thread and drawn on the UI thread.
pub trait TextLayout: Send + Sync {
    /// The width of the widest line, in device pixels.
    fn width(&self) -> f32;

    /// The height of all lines, in device pixels.
    fn height(&self) -> f32;

    /// The caret position nearest `(x, y)`, using the shaper's own hit-testing
    /// so right-to-left and complex scripts are accurate.
    fn hit_test_point(&self, x: f32, y: f32) -> TextHit;

    /// The boxes covering the text from byte `byte_start` up to `byte_end`, one
    /// per line and per direction change, for painting a selection behind the
    /// text.
    fn selection_rects(&self, byte_start: usize, byte_end: usize) -> Vec<Rect>;

    /// The backend's concrete layout type, so its own canvas can draw it.
    fn as_any(&self) -> &dyn Any;
}

/// A `Send + Sync` handle that shapes text for a backend.
///
/// It can be obtained once from [`Backend::text_shaper`](super::Backend::text_shaper)
/// and used from a worker thread, where the UI-thread `Backend` is unavailable.
pub trait TextShaper: Send + Sync {
    /// Shapes `text` wrapped at `max_width` device pixels (use
    /// `f32::INFINITY` for a single unwrapped line) at `dpi`.
    fn layout(&self, text: &str, spec: &FontSpec, max_width: f32, dpi: u32) -> Box<dyn TextLayout>;
}

/// An empty layout, for a backend that predates the text API.
pub(crate) struct EmptyLayout;

impl TextLayout for EmptyLayout {
    fn width(&self) -> f32 {
        0.0
    }

    fn height(&self) -> f32 {
        0.0
    }

    fn hit_test_point(&self, _x: f32, _y: f32) -> TextHit {
        TextHit {
            byte_index: 0,
            inside: false,
        }
    }

    fn selection_rects(&self, _byte_start: usize, _byte_end: usize) -> Vec<Rect> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The default shaper: shapes nothing. A backend overrides
/// [`Backend::text_shaper`](super::Backend::text_shaper) to provide one.
pub(crate) struct UnsupportedShaper;

impl TextShaper for UnsupportedShaper {
    fn layout(
        &self,
        _text: &str,
        _spec: &FontSpec,
        _max_width: f32,
        _dpi: u32,
    ) -> Box<dyn TextLayout> {
        Box::new(EmptyLayout)
    }
}
