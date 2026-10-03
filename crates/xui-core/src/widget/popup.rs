#![forbid(unsafe_code)]

//! The shared elevation treatment for transient surfaces.
//!
//! Menus, combo popups and tooltips all read as raised above the surface they
//! cover, so they share one painter: an opaque face and a hairline border. A
//! native popup is a top-level window that DWM already gives a drop shadow, so
//! the painter draws no shadow of its own. It fills the whole surface with the
//! face colour rather than only a rounded rectangle: the corners of a native
//! popup window are square, and the rounded face left them to the suppressed
//! erase (the off-screen buffer), where they showed as black pixels.

use crate::backend::Canvas;
use crate::color::Color;
use crate::geometry::Rect;
use crate::theme::Theme;

/// How much of the text colour is mixed into the border, so a transient
/// surface's edge reads clearly against the surface behind it (the plain card
/// stroke is too faint to outline a floating surface).
const BORDER_MIX: f32 = 0.25;

/// Paints an elevated popup face over the canvas's bounds: the `face` colour
/// and a hairline border more contrasted than a card's. `face` is the surface
/// colour the caller's content is drawn on.
pub(crate) fn paint(canvas: &mut dyn Canvas, theme: Theme, face: Color) {
    let bounds = canvas.bounds();
    canvas.clear(face);
    if theme.bevel.a > 0 {
        let top = Rect::new(
            bounds.left + 1,
            bounds.top + 1,
            bounds.right - 1,
            bounds.top + 2,
        );
        canvas.fill_rect_rgba(top, theme.bevel);
    }
    canvas.stroke_rect(bounds, border(theme), 1.0);
}

/// The popup outline colour: the card stroke mixed towards the text colour.
fn border(theme: Theme) -> Color {
    theme.border.lerp(theme.text, BORDER_MIX)
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
    fn a_popup_fills_its_whole_face_then_draws_a_border() {
        let theme = Theme::light();
        let ops = ops(theme);
        let face = Rect::new(10, 10, 110, 60);
        assert!(
            matches!(ops.first(), Some(DrawOp::Clear(color)) if *color == theme.raised),
            "the whole face is filled, so the square corners are never left blank: {ops:?}"
        );
        assert!(
            matches!(ops.last(), Some(DrawOp::Stroke(rect, _, _)) if *rect == face),
            "a border closes the surface: {ops:?}"
        );
    }

    #[test]
    fn the_popup_border_is_more_contrasted_than_a_card_stroke() {
        for theme in [Theme::light(), Theme::dark()] {
            let border = border(theme);
            assert!(
                border.contrast_ratio(theme.raised) > theme.border.contrast_ratio(theme.raised),
                "the popup border must stand out more than the card stroke"
            );
        }
    }
}
