//! The backend-neutral display list: the commands a laid-out document becomes
//! and the types they refer to. Nothing here depends on egui or xui-win32, so
//! the list could be replayed by any painter. Border decomposition lives in
//! `border.rs` and is re-exported here.

use std::sync::Arc;

use crate::geom::{Point, Radius, Rect, Rgba};
use crate::links::LinkTable;
use crate::text_runs::TextRunTable;

mod border;

pub use border::{BorderEdge, BorderKind, BorderPaint, EdgePaint, decompose_borders};

/// Identifies one font of a document. Indexes into [`DisplayList::fonts`].
pub type FontKey = u32;
/// Identifies one decoded image of a document. Indexes into
/// [`DisplayList::images`].
pub type ImageKey = u32;

/// Everything the painter needs to rebuild one font: DirectWrite (and the
/// Direct2D painter) resolves this through its own text system, so the widths
/// it paints match the widths litehtml measured.
#[derive(Clone, Debug, PartialEq)]
pub struct FontDesc {
    /// The CSS `font-family` list, as written.
    pub family: String,
    /// The em size in device-independent pixels.
    pub size: f32,
    /// Weight from 100 to 900.
    pub weight: u16,
    /// Whether the face is italic.
    pub italic: bool,
}

/// A decoded RGBA image, ready to upload.
#[derive(Clone)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA pixel data, four bytes per pixel.
    pub rgba: Vec<u8>,
}

/// One colour in a gradient, at `offset` in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// The position along the gradient, in `0.0..=1.0`.
    pub offset: f32,
    /// The colour at that position.
    pub color: Rgba,
}

/// A linear gradient from `start` to `end`.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    /// Where the gradient starts.
    pub start: Point,
    /// Where the gradient ends.
    pub end: Point,
    /// The colours at their positions; at least two.
    pub stops: Vec<GradientStop>,
}

/// A radial gradient centred at `center` with elliptical radii.
#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradient {
    /// The centre of the gradient.
    pub center: Point,
    /// The horizontal radius.
    pub radius_x: f32,
    /// The vertical radius.
    pub radius_y: f32,
    /// The colours at their positions; at least two.
    pub stops: Vec<GradientStop>,
}

/// A solid pen: a width and an RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// The line width.
    pub width: f32,
    /// The line colour.
    pub color: Rgba,
}

impl Stroke {
    /// A solid stroke `width` wide.
    pub const fn solid(width: f32, color: Rgba) -> Stroke {
        Stroke { width, color }
    }
}

/// How a line or border edge is broken up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dash {
    /// An unbroken line.
    Solid,
    /// Dashes (segment and gap are three times the width).
    Dashed,
    /// Dots (segment and gap are the width, round caps).
    Dotted,
}

/// One paint operation, in document coordinates (device-independent pixels,
/// origin at the document's top-left).
#[derive(Clone, Debug)]
pub enum Cmd {
    /// A filled rectangle (rounded when any corner radius is non-zero).
    Rect {
        /// The rectangle.
        rect: Rect,
        /// One radius per corner.
        radii: [Radius; 4],
        /// The fill colour.
        fill: Rgba,
    },
    /// A rounded outline of one colour and width all round.
    Outline {
        /// The rectangle.
        rect: Rect,
        /// One radius per corner.
        radii: [Radius; 4],
        /// The pen.
        stroke: Stroke,
    },
    /// A line.
    Line {
        /// The start point.
        a: Point,
        /// The end point.
        b: Point,
        /// The pen.
        stroke: Stroke,
        /// How the line is broken up.
        dash: Dash,
    },
    /// A filled/outlined circle (list markers).
    Circle {
        /// The centre.
        center: Point,
        /// The radius.
        radius: f32,
        /// The fill colour.
        fill: Rgba,
        /// The outline pen.
        stroke: Stroke,
    },
    /// A filled polygon (at least three points); engines that draw borders
    /// as trapezoids use it.
    Polygon {
        /// The corners, in order.
        points: Vec<Point>,
        /// The fill colour.
        fill: Rgba,
    },
    /// A text run. `width` and `height` are the run's box, for culling.
    Text {
        /// The run's top-left corner.
        origin: Point,
        /// The run's laid-out width.
        width: f32,
        /// The run's line height.
        height: f32,
        /// The text.
        text: Arc<str>,
        /// The font, into [`DisplayList::fonts`].
        font: FontKey,
        /// The text colour.
        color: Rgba,
    },
    /// One tile of an image, drawn into `rect`.
    Image {
        /// The image, into [`DisplayList::images`].
        image: ImageKey,
        /// Where to draw it.
        rect: Rect,
    },
    /// A linear-gradient fill over `rect`.
    LinearGradient {
        /// The filled rectangle.
        rect: Rect,
        /// The gradient.
        gradient: LinearGradient,
    },
    /// A radial-gradient fill over `rect`.
    RadialGradient {
        /// The filled rectangle.
        rect: Rect,
        /// The gradient.
        gradient: RadialGradient,
    },
    /// Restricts drawing to `rect` (rounded when any radius is non-zero) until
    /// the matching [`Cmd::PopClip`].
    PushClip {
        /// The clip rectangle.
        rect: Rect,
        /// One radius per corner.
        radii: [Radius; 4],
    },
    /// Ends the innermost clip.
    PopClip,
}

impl Cmd {
    /// Where this paints, for culling; `None` for clip bookkeeping.
    pub fn bounds(&self) -> Option<Rect> {
        Some(match self {
            Cmd::Rect { rect, .. }
            | Cmd::Outline { rect, .. }
            | Cmd::Image { rect, .. }
            | Cmd::LinearGradient { rect, .. }
            | Cmd::RadialGradient { rect, .. } => rect.expand(1.0),
            Cmd::Line { a, b, stroke, .. } => {
                Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
                    .expand(stroke.width + 1.0)
            }
            Cmd::Circle {
                center,
                radius,
                stroke,
                ..
            } => {
                let r = radius + stroke.width / 2.0 + 1.0;
                Rect::new(center.x - r, center.y - r, center.x + r, center.y + r)
            }
            Cmd::Text {
                origin,
                width,
                height,
                ..
            } => Rect::from_min_size(origin.x, origin.y, *width, *height).expand(2.0),
            Cmd::Polygon { points, .. } => {
                let first = points.first().copied().unwrap_or_default();
                points
                    .iter()
                    .fold(Rect::new(first.x, first.y, first.x, first.y), |r, p| {
                        r.union(Rect::new(p.x, p.y, p.x, p.y))
                    })
                    .expand(1.0)
            }
            Cmd::PushClip { .. } | Cmd::PopClip => return None,
        })
    }
}

/// A laid-out document, ready to paint any number of times.
#[derive(Clone)]
pub struct DisplayList {
    /// The paint operations, in order.
    pub cmds: Vec<Cmd>,
    /// The content size (width, height) in device-independent pixels.
    pub size: (f32, f32),
    /// The document's fonts, indexed by [`FontKey`].
    pub fonts: Vec<FontDesc>,
    /// The document's decoded images, indexed by [`ImageKey`].
    pub images: Vec<Arc<Image>>,
}

/// A finished render of one page, as the worker sends it to the UI thread.
pub struct Frame {
    /// The render job this frame answers.
    pub id: u64,
    /// The display list.
    pub list: Arc<DisplayList>,
    /// Where the page's text is, for selection (see [`TextRunTable`]).
    pub runs: Arc<TextRunTable>,
    /// Where the page's links are, for clicks and the hover cursor.
    pub links: Arc<LinkTable>,
}

// ─── Gradient stop mapping (pure, testable) ────────────────────────────────

/// Sorts gradient stops by offset, deduplicating coincident positions (keeping
/// the last colour, as CSS does). Stops carry straight RGBA; Direct2D
/// interpolates them itself.
pub fn normalize_stops(stops: &[(f32, Rgba)]) -> Vec<GradientStop> {
    let mut stops: Vec<GradientStop> = stops
        .iter()
        .map(|(offset, color)| GradientStop {
            offset: *offset,
            color: *color,
        })
        .collect();
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    let mut out: Vec<GradientStop> = Vec::with_capacity(stops.len());
    for stop in stops {
        if let Some(last) = out.last_mut()
            && (last.offset - stop.offset).abs() < 1e-4
        {
            last.color = stop.color;
        } else {
            out.push(stop);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_stops_are_sorted_and_coincident_ones_deduplicated() {
        let stops = normalize_stops(&[
            (1.0, Rgba::BLACK),
            (0.0, Rgba::WHITE),
            (0.5, Rgba::rgb(1, 2, 3)),
            (0.5, Rgba::rgb(9, 9, 9)),
        ]);
        assert_eq!(stops.len(), 3);
        assert_eq!(stops[0].offset, 0.0);
        assert_eq!(
            stops[1].color,
            Rgba::rgb(9, 9, 9),
            "the last colour at a stop wins"
        );
        assert_eq!(stops[2].offset, 1.0);
    }

    #[test]
    fn culling_bounds_cover_every_visible_cmd_kind() {
        let text = Cmd::Text {
            origin: Point::new(10.0, 20.0),
            width: 50.0,
            height: 16.0,
            text: Arc::from("hello"),
            font: 0,
            color: Rgba::BLACK,
        };
        assert_eq!(text.bounds(), Some(Rect::new(8.0, 18.0, 62.0, 38.0)));
        let clip = Cmd::PushClip {
            rect: Rect::default(),
            radii: [Radius::default(); 4],
        };
        assert_eq!(clip.bounds(), None);
    }
}
