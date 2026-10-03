#![forbid(unsafe_code)]

//! The flow engine: breaks a [`Document`](crate::Document)'s paragraphs into
//! lines around floating images, through a backend's `TextShaper`.
//!
//! Everything here is in device pixels at the layout's DPI; the model's `Dip`
//! sizes are converted once, when a paragraph is laid out. The pipeline per
//! paragraph is: `segment` (UAX #14 break opportunities and style
//! boundaries become pieces), `items` (each piece is shaped, through the
//! bounded `shape_cache`), `line` (greedy line breaking in the intervals the
//! `floats` leave free) and `flow` (paragraphs stacked into a continuous
//! area, with floats carried across paragraph boundaries and relayout limited
//! to what changed). `hit` maps points to positions and back.

mod floats;
mod flow;
mod hit;
mod items;
mod line;
mod nav;
mod partial;
mod resolve;
mod segment;
mod shape_cache;
#[cfg(test)]
mod tests;

use std::ops::Range;
use std::sync::Arc;

use xui_core::backend::TextLayout;

use crate::model::{CharStyleId, ObjectId};

pub use floats::{Excl, ExclKind, FRect};
pub use flow::Layout;

/// What a [`PlacedItem`] draws.
#[derive(Clone)]
pub enum PlacedKind {
    /// A word, shaped once; paint calls `draw_layout` with it.
    Text(Arc<dyn TextLayout>),
    /// A run of spaces or tabs, `unit` pixels per space (stretched when the
    /// line is justified).
    Space {
        /// The width of one space.
        unit: f32,
    },
    /// An inline image sitting on the baseline.
    Object {
        /// The object.
        id: ObjectId,
        /// The displayed height.
        height: f32,
    },
    /// The anchor of a floating image (zero width); the image itself is in
    /// [`ParaLayout::floats`].
    Float(ObjectId),
    /// A forced line break (zero width).
    Break,
}

/// One laid-out piece of a line.
#[derive(Clone)]
pub struct PlacedItem {
    /// Left edge, in area pixels.
    pub x: f32,
    /// Width of the piece.
    pub width: f32,
    /// The paragraph bytes the piece covers.
    pub range: Range<usize>,
    /// What to draw.
    pub kind: PlacedKind,
    /// The character style of the piece.
    pub style: CharStyleId,
    /// The baseline offset (positive is down) of super- and subscripts.
    pub dy: f32,
    /// The font size in pixels, for decoration lines.
    pub size: f32,
}

/// One line of a paragraph.
#[derive(Clone)]
pub struct Line {
    /// The paragraph bytes the line covers.
    pub range: Range<usize>,
    /// Top, relative to the paragraph's top.
    pub y: f32,
    /// Height.
    pub height: f32,
    /// Baseline, relative to the paragraph's top.
    pub baseline: f32,
    /// The left edge of the first piece (where an empty line's caret sits).
    pub x: f32,
    /// The pieces, left to right.
    pub items: Vec<PlacedItem>,
}

/// A floating image placed by the flow.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedFloat {
    /// The object.
    pub id: ObjectId,
    /// The byte of its anchor in the paragraph.
    pub byte: usize,
    /// The image rectangle, relative to the paragraph's top.
    pub rect: FRect,
}

/// A list marker, hung to the left of the first line.
#[derive(Clone)]
pub struct Marker {
    /// The shaped marker text.
    pub layout: Arc<dyn TextLayout>,
    /// Left edge in area pixels.
    pub x: f32,
    /// Top, relative to the paragraph's top.
    pub y: f32,
    /// The character style the marker is set in.
    pub style: CharStyleId,
}

/// A paragraph's lines and the floats it placed.
#[derive(Clone)]
pub struct ParaLayout {
    /// Top of the paragraph (its space before included), in area pixels.
    pub y: f32,
    /// Height, space before and after included.
    pub height: f32,
    /// The lines; their `y` is relative to [`ParaLayout::y`].
    pub lines: Vec<Line>,
    /// The floating images anchored in this paragraph.
    pub floats: Vec<PlacedFloat>,
    /// The list marker, for a list item.
    pub marker: Option<Marker>,
    /// The quote rule `(x, top, bottom)`, relative to the paragraph's top.
    pub rule: Option<(f32, f32, f32)>,
    /// Exclusions active when the paragraph started, relative to `y`.
    pub(crate) entering: Vec<Excl>,
    /// Exclusions still active at its bottom, relative to `y + height`.
    pub(crate) exit: Vec<Excl>,
    pub(crate) dirty: bool,
    /// Laid out without knowing the floats entering it (the paragraphs before
    /// it were still estimates); checked when the flow reaches it in order.
    pub(crate) speculative: bool,
    /// The paragraph text this layout was made from.
    pub(crate) text: Arc<str>,
    /// The list number its marker was laid out with; an edit elsewhere in the
    /// list can change it without touching this paragraph.
    pub(crate) number: Option<usize>,
}

impl ParaLayout {
    pub(crate) fn dirty() -> ParaLayout {
        ParaLayout {
            y: 0.0,
            height: 0.0,
            lines: Vec::new(),
            floats: Vec::new(),
            marker: None,
            rule: None,
            entering: Vec::new(),
            exit: Vec::new(),
            dirty: true,
            speculative: false,
            text: Arc::from(""),
            number: None,
        }
    }

    /// The bottom edge, in area pixels.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}
