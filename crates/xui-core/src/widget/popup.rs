#![forbid(unsafe_code)]

//! The shared elevation treatment for transient surfaces.
//!
//! Menus, combo popups and tooltips all read as raised above the surface they
//! cover, so they share one painter: a rounded face, a hairline border and a
//! soft drop shadow. The portable [`Canvas`] has no blur, so the shadow is
//! approximated by a few concentric translucent rounded rectangles offset
//! downward, drawn *outside* the surface's own bounds. The face stays exactly
//! at the caller's bounds, so content placement is unchanged.
//!
//! The composited canvas backend shows the overhang; a native per-window popup
//! clips it to its own client area, where the OS (DWM) draws the drop shadow.

use crate::backend::{Canvas, Corner, Rgba};
use crate::color::Color;
use crate::geometry::Rect;
use crate::theme::Theme;
use crate::units::Dip;

/// The corner radius of a popup face.
pub(crate) const RADIUS: Dip = Dip(4.0);
/// The depth of the shadow's falloff around the face.
const SHADOW_BLUR: Dip = Dip(8.0);
/// How far the shadow drops below the face.
const SHADOW_OFFSET: Dip = Dip(2.0);
/// The number of translucent rectangles approximating the blur.
const SHADOW_RINGS: i32 = 4;
/// The alpha of each ring (`0..=255`). Overlapping rings darken towards the
/// face, so the shadow fades out from the edge.
const SHADOW_ALPHA: u8 = 24;

/// Paints an elevated popup face over the canvas's bounds: a soft shadow, the
/// rounded `face` colour and a hairline border. `face` is the surface colour
/// the caller's content is drawn on.
pub(crate) fn paint(canvas: &mut dyn Canvas, theme: Theme, face: Color) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    paint_shadow(canvas, bounds, theme.shadow, dpi);
    let radius = RADIUS.to_px(dpi).value().max(1) as f32;
    canvas.fill_rounded_rect(bounds, radius, face);
    canvas.stroke_rounded_rect(bounds, radius, theme.border, 1.0);
}

/// Draws the soft shadow around `bounds`, outermost ring first so the
/// overlapping translucent fills darken towards the face.
fn paint_shadow(canvas: &mut dyn Canvas, bounds: Rect, color: Color, dpi: u32) {
    let blur = SHADOW_BLUR.to_px(dpi).value().max(1);
    let offset = SHADOW_OFFSET.to_px(dpi).value();
    let radius = RADIUS.to_px(dpi).value().max(1) as f32;
    for ring in (1..=SHADOW_RINGS).rev() {
        let grow = blur * ring / SHADOW_RINGS;
        let rect = Rect::new(
            bounds.left - grow,
            bounds.top - grow + offset,
            bounds.right + grow,
            bounds.bottom + grow + offset,
        );
        let corners = [Corner::uniform(radius + grow as f32); 4];
        canvas.fill_rounded_rect_corners(
            rect,
            corners,
            Rgba::with_alpha(color.r, color.g, color.b, SHADOW_ALPHA),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::backend::headless::{DrawOp, HeadlessBackend};
    use crate::backend::{Backend, NodeKind, NodeSpec, ParentRef, PlatformSpec};

    /// Renders one node whose painter is [`paint`] and returns its draw ops.
    fn ops(theme: Theme) -> Vec<DrawOp> {
        let bounds = Rect::new(10, 10, 110, 60);
        let backend = HeadlessBackend::new();
        let window = backend.open_window(&PlatformSpec::new("test")).unwrap();
        let node = backend
            .create(
                ParentRef::Window(window),
                &NodeSpec::new(NodeKind::Custom, bounds),
            )
            .unwrap();
        backend.set_painter(
            node,
            Rc::new(move |canvas| paint(canvas, theme, theme.raised)),
        );
        backend.render(node);
        backend.ops(node)
    }

    #[test]
    fn a_popup_paints_shadow_face_and_border_in_that_order() {
        let ops = ops(Theme::light());
        let face = Rect::new(10, 10, 110, 60);
        let rings: Vec<&DrawOp> = ops
            .iter()
            .filter(|op| matches!(op, DrawOp::RoundedCorners(..)))
            .collect();
        assert_eq!(
            rings.len(),
            SHADOW_RINGS as usize,
            "one translucent fill per shadow ring: {ops:?}"
        );
        assert!(
            rings
                .iter()
                .all(|op| matches!(op, DrawOp::RoundedCorners(_, _, color) if color.a < 255)),
            "the shadow rings are translucent: {ops:?}"
        );
        assert!(
            matches!(&ops[SHADOW_RINGS as usize], DrawOp::Rounded(rect, _, color) if *rect == face && *color == Theme::light().raised),
            "the face stays at its bounds after the shadow: {ops:?}"
        );
        assert!(
            matches!(ops.last(), Some(DrawOp::StrokeRounded(..))),
            "a hairline border closes the face: {ops:?}"
        );
    }

    #[test]
    fn the_shadow_drops_below_the_face_without_moving_it() {
        let ops = ops(Theme::dark());
        let face = Rect::new(10, 10, 110, 60);
        assert!(
            ops.iter().any(
                |op| matches!(op, DrawOp::RoundedCorners(rect, _, _) if rect.bottom > face.bottom)
            ),
            "the shadow extends below the face: {ops:?}"
        );
    }
}
