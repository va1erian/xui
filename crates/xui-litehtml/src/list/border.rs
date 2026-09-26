//! Border decomposition: turning a box's four litehtml borders into paint
//! operations on the neutral display list. Pure and testable; split out of
//! `list.rs` so each file stays small.

use crate::geom::{Point, Radius, Rect, Rgba};

use super::{Dash, Stroke};

/// How a border edge is styled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderKind {
    /// Not drawn.
    None,
    /// Drawn as a solid fill (and treated as such for a rounded outline).
    Solid,
    /// Drawn as a solid fill.
    Double,
    /// Dashed.
    Dashed,
    /// Dotted.
    Dotted,
    /// Drawn as a solid fill (bevelled looks are flattened).
    Groove,
    /// Drawn as a solid fill.
    Ridge,
    /// Drawn as a solid fill.
    Inset,
    /// Drawn as a solid fill.
    Outset,
}

impl BorderKind {
    /// Whether the edge is drawn at all.
    pub fn is_drawn(self) -> bool {
        !matches!(self, BorderKind::None)
    }

    /// Whether the edge is drawn as a solid fill (not a dashed/dotted line).
    pub fn is_solid(self) -> bool {
        matches!(
            self,
            BorderKind::Solid
                | BorderKind::Double
                | BorderKind::Groove
                | BorderKind::Ridge
                | BorderKind::Inset
                | BorderKind::Outset
        )
    }
}

/// One border edge, as recorded from litehtml.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderEdge {
    /// The edge width.
    pub width: f32,
    /// The edge colour.
    pub color: Rgba,
    /// The edge style.
    pub kind: BorderKind,
}

/// One painted edge after decomposition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EdgePaint {
    /// A solid fill of the edge's rectangle.
    Solid {
        /// The edge's rectangle.
        rect: Rect,
        /// The fill colour.
        color: Rgba,
    },
    /// A dashed/dotted line down the middle of the edge.
    Line {
        /// The start point.
        a: Point,
        /// The end point.
        b: Point,
        /// The stroke width.
        width: f32,
        /// The line colour.
        color: Rgba,
        /// How the line is broken up.
        dash: Dash,
    },
}

/// What `decompose_borders` produces: either a single rounded outline, or the
/// four edges painted individually.
#[derive(Clone, Debug, PartialEq)]
pub enum BorderPaint {
    /// One colour and width all round, with rounded corners.
    Outline {
        /// The box.
        rect: Rect,
        /// One radius per corner.
        radii: [Radius; 4],
        /// The pen.
        stroke: Stroke,
    },
    /// Each drawn edge on its own.
    Edges(Vec<EdgePaint>),
}

/// Turns a box's four borders into paint operations. A box whose borders are
/// all drawn, all solid, one colour and one width, with rounded corners,
/// collapses to a single rounded outline; anything else is decomposed into
/// four edges so dashed/dotted edges are stroked.
pub fn decompose_borders(
    rect: Rect,
    radii: [Radius; 4],
    top: BorderEdge,
    right: BorderEdge,
    bottom: BorderEdge,
    left: BorderEdge,
) -> BorderPaint {
    let drawn = |edge: BorderEdge| edge.width > 0.0 && edge.kind.is_drawn();
    let rounded = radii.iter().any(|r| r.x > 0.0 || r.y > 0.0);
    let edges = [top, right, bottom, left];
    if rounded
        && edges.iter().all(|e| drawn(*e) && e.kind.is_solid())
        && edges
            .iter()
            .all(|e| e.width == top.width && e.color == top.color)
    {
        return BorderPaint::Outline {
            rect,
            radii,
            stroke: Stroke::solid(top.width, top.color),
        };
    }

    let mut paints = Vec::new();
    // Each edge's rectangle inside the box, in top, bottom, left, right order.
    let edges = [
        (
            top,
            Rect::from_min_size(rect.left, rect.top, rect.width(), top.width),
            true,
        ),
        (
            bottom,
            Rect::from_min_size(
                rect.left,
                rect.bottom - bottom.width,
                rect.width(),
                bottom.width,
            ),
            true,
        ),
        (
            left,
            Rect::from_min_size(rect.left, rect.top, left.width, rect.height()),
            false,
        ),
        (
            right,
            Rect::from_min_size(
                rect.right - right.width,
                rect.top,
                right.width,
                rect.height(),
            ),
            false,
        ),
    ];
    for (edge, edge_rect, horizontal) in edges {
        if !drawn(edge) {
            continue;
        }
        if edge.kind.is_solid() {
            paints.push(EdgePaint::Solid {
                rect: edge_rect,
                color: edge.color,
            });
            continue;
        }
        let (a, b) = if horizontal {
            (
                Point::new(edge_rect.left, edge_rect.top + edge_rect.height() / 2.0),
                Point::new(edge_rect.right, edge_rect.top + edge_rect.height() / 2.0),
            )
        } else {
            (
                Point::new(edge_rect.left + edge_rect.width() / 2.0, edge_rect.top),
                Point::new(edge_rect.left + edge_rect.width() / 2.0, edge_rect.bottom),
            )
        };
        let dash = match edge.kind {
            BorderKind::Dashed => Dash::Dashed,
            _ => Dash::Dotted,
        };
        paints.push(EdgePaint::Line {
            a,
            b,
            width: edge.width,
            color: edge.color,
            dash,
        });
    }
    BorderPaint::Edges(paints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uniform_rounded_border_is_a_single_outline() {
        let rect = Rect::new(0.0, 0.0, 40.0, 40.0);
        let radii = [Radius::uniform(8.0); 4];
        let solid = BorderEdge {
            width: 2.0,
            color: Rgba::rgb(0, 0, 255),
            kind: BorderKind::Solid,
        };
        let paint = decompose_borders(rect, radii, solid, solid, solid, solid);
        assert!(
            matches!(paint, BorderPaint::Outline { stroke, .. } if stroke.width == 2.0 && stroke.color == Rgba::rgb(0, 0, 255))
        );
    }

    #[test]
    fn a_square_border_decomposes_into_four_edges() {
        let rect = Rect::new(0.0, 0.0, 40.0, 40.0);
        let solid = BorderEdge {
            width: 2.0,
            color: Rgba::rgb(0, 0, 255),
            kind: BorderKind::Solid,
        };
        let BorderPaint::Edges(edges) =
            decompose_borders(rect, [Radius::default(); 4], solid, solid, solid, solid)
        else {
            panic!("expected edges");
        };
        assert_eq!(edges.len(), 4);
        assert!(edges.iter().all(|e| matches!(e, EdgePaint::Solid { .. })));
    }

    #[test]
    fn a_dashed_edge_is_a_line_and_undrawn_edges_are_dropped() {
        let rect = Rect::new(0.0, 0.0, 40.0, 40.0);
        let none = BorderEdge {
            width: 0.0,
            color: Rgba::BLACK,
            kind: BorderKind::None,
        };
        let dashed = BorderEdge {
            width: 2.0,
            color: Rgba::rgb(255, 0, 0),
            kind: BorderKind::Dashed,
        };
        let BorderPaint::Edges(edges) =
            decompose_borders(rect, [Radius::default(); 4], dashed, none, dashed, none)
        else {
            panic!("expected edges");
        };
        assert_eq!(edges.len(), 2);
        assert!(edges.iter().all(|e| matches!(
            e,
            EdgePaint::Line {
                dash: Dash::Dashed,
                ..
            }
        )));
    }
}
