#![forbid(unsafe_code)]

//! How a [`Theme`]'s decoration tokens are painted: the window background,
//! a widget's backdrop, cards, control faces and the accent glow. Widgets and
//! backends share these so every surface reads the tokens the same way, and a
//! flat theme (no gradient, bevel, radius or glow) paints exactly the plain
//! fills it always did.

use super::Theme;
use crate::Color;
use crate::backend::{Canvas, Corner, GradientStop, LinearGradient, Rgba};
use crate::geometry::{Point, Rect};

/// Fills `area` with the window background of a window covering `window`:
/// the vertical gradient from [`Theme::background`] to
/// [`Theme::background_end`], so a partial repaint matches the rest.
pub fn paint_background(canvas: &mut dyn Canvas, area: Rect, window: Rect, theme: &Theme) {
    if theme.background_end == theme.background {
        canvas.fill_rect(area, theme.background);
        return;
    }
    canvas.fill_rect_linear(
        area,
        &vertical(window, theme.background, theme.background_end),
    );
}

/// Prepares a widget's background: nothing where the backend has already
/// painted the widget's container under it
/// ([`Canvas::composites_parents`]), so the widget sits on the window or card
/// it is placed in; otherwise a fill with `color`, the container colour the
/// widget assumes.
pub fn backdrop(canvas: &mut dyn Canvas, color: Color) {
    if !canvas.composites_parents() {
        canvas.clear(color);
    }
}

/// Paints a card over `rect`: a section's own background (panel, group box,
/// dialog). Rounded by [`Theme::corner_radius`], filled with the
/// [`Theme::surface`] gradient, bevelled and framed with [`Theme::border`].
pub fn card(canvas: &mut dyn Canvas, rect: Rect, theme: &Theme) {
    card_with(canvas, rect, theme, theme.surface, theme.surface_end);
}

/// [`card`] filled from `top` to `bottom` instead of the surface colours (a
/// raised popup or dialog card).
pub fn card_with(canvas: &mut dyn Canvas, rect: Rect, theme: &Theme, top: Color, bottom: Color) {
    let radius = f32::from(theme.corner_radius);
    if radius == 0.0 && top == bottom && theme.bevel.a == 0 {
        canvas.fill_rect(rect, top);
        canvas.stroke_rect(rect, theme.border, 1.0);
        return;
    }
    gradient_face(canvas, rect, radius, top, bottom, theme.bevel);
    canvas.stroke_rounded_rect(rect, radius, theme.border, 1.0);
}

/// Fills the whole canvas as a bar (menu bar, tab strip, status bar): the
/// [`Theme::surface`] gradient with the bevel along its top.
pub fn band(canvas: &mut dyn Canvas, theme: &Theme) {
    let bounds = canvas.bounds();
    if theme.surface_end == theme.surface && theme.bevel.a == 0 {
        canvas.clear(theme.surface);
        return;
    }
    gradient_face(
        canvas,
        bounds,
        0.0,
        theme.surface,
        theme.surface_end,
        theme.bevel,
    );
}

/// Highlights a hovered row (list, tree): rounded on a decorated theme, a
/// plain fill otherwise.
pub fn row(canvas: &mut dyn Canvas, rect: Rect, fill: Color, theme: &Theme) {
    if decorated(theme) {
        canvas.fill_rounded_rect(rect.shrink(1), ROW_RADIUS, fill);
    } else {
        canvas.fill_rect(rect, fill);
    }
}

/// Highlights the selected row (list, tree) in the accent: on a decorated
/// theme the accent fades from left to right behind a glowing indicator bar,
/// so the row's text keeps [`Theme::text`] contrast; a plain fill otherwise.
pub fn selected_row(canvas: &mut dyn Canvas, rect: Rect, theme: &Theme) {
    if !decorated(theme) {
        canvas.fill_rect(rect, theme.accent);
        return;
    }
    let a = theme.accent;
    let inner = rect.shrink(1);
    canvas.push_clip_rounded(inner, [Corner::uniform(ROW_RADIUS); 4]);
    canvas.fill_rect_linear(
        inner,
        &LinearGradient::new(
            Point::new(inner.left, inner.top),
            Point::new(inner.right, inner.top),
            vec![
                GradientStop::new(0.0, Rgba::with_alpha(a.r, a.g, a.b, 0x80)),
                GradientStop::new(1.0, Rgba::with_alpha(a.r, a.g, a.b, 0x18)),
            ],
        ),
    );
    canvas.pop_clip();
    let bar = Rect::new(inner.left, inner.top + 3, inner.left + 3, inner.bottom - 3);
    canvas.fill_rounded_rect(bar, 1.5, a.lerp(Color::rgb(255, 255, 255), 0.3));
}

/// The text colour on a row painted by [`selected_row`].
pub fn selected_row_text(theme: &Theme) -> Color {
    if decorated(theme) {
        theme.text
    } else {
        theme.text_on_accent
    }
}

/// Corner radius of a decorated row highlight.
const ROW_RADIUS: f32 = 4.0;

/// Paints a control's face (button, check box, selected tab): `fill` at the
/// top darkening by [`Theme::shade`] toward the bottom, with the bevel.
pub fn face(canvas: &mut dyn Canvas, rect: Rect, radius: f32, fill: Color, theme: &Theme) {
    if theme.shade == 0 && theme.gloss == 0 && theme.bevel.a == 0 {
        canvas.fill_rounded_rect(rect, radius, fill);
        return;
    }
    gradient_face(
        canvas,
        rect,
        radius,
        glossed(fill, theme),
        shaded(fill, theme),
        theme.bevel,
    );
}

/// `fill` lightened by [`Theme::gloss`]: the top colour of a control face.
pub fn glossed(fill: Color, theme: &Theme) -> Color {
    fill.lerp(Color::rgb(255, 255, 255), f32::from(theme.gloss) / 255.0)
}

/// A glossy disc (radio dot, round swatch) of `fill` centred on `center`:
/// the [`face`] gradient clipped to a circle.
pub fn disc(canvas: &mut dyn Canvas, center: Point, radius: f32, fill: Color, theme: &Theme) {
    let r = radius.round() as i32;
    let rect = Rect::new(center.x - r, center.y - r, center.x + r, center.y + r);
    if !decorated(theme) {
        canvas.fill_ellipse(center, radius, radius, fill);
        return;
    }
    face(canvas, rect, radius, fill, theme);
}

/// Corner radius of a decorated input field.
const FIELD_RADIUS: f32 = 4.0;

/// Fills an input field's background (edit, combo box, number field): a
/// rounded well on a decorated theme, the whole node otherwise.
pub fn field(canvas: &mut dyn Canvas, theme: &Theme) {
    if decorated(theme) {
        backdrop(canvas, theme.background);
        let bounds = canvas.bounds();
        canvas.fill_rounded_rect(bounds, FIELD_RADIUS, theme.input_background);
    } else {
        canvas.clear(theme.input_background);
    }
}

/// Outlines an input field filled by [`field`] in `color`.
pub fn field_frame(canvas: &mut dyn Canvas, rect: Rect, color: Color, theme: &Theme) {
    if decorated(theme) {
        canvas.stroke_rounded_rect(rect, FIELD_RADIUS, color, 1.0);
    } else {
        canvas.stroke_rect(rect, color, 1.0);
    }
}

/// Whether `theme` decorates controls (gradient faces or rounded cards):
/// indicators then draw as filled, glowing controls instead of outlines.
pub fn decorated(theme: &Theme) -> bool {
    theme.shade > 0 || theme.corner_radius > 0
}

/// `fill` darkened by [`Theme::shade`]: the bottom colour of a control face.
pub fn shaded(fill: Color, theme: &Theme) -> Color {
    fill.lerp(Color::rgb(0, 0, 0), f32::from(theme.shade) / 255.0)
}

/// A soft accent glow around a checked or selected indicator centred on
/// `center` with `radius`, [`Theme::glow`] strong (nothing at 0).
pub fn glow(canvas: &mut dyn Canvas, center: Point, radius: f32, theme: &Theme) {
    if theme.glow == 0 {
        return;
    }
    let a = theme.accent;
    // Rings fading outward, each a third of the one inside it.
    for (spread, share) in [(1.0, 3u16), (2.5, 2), (4.0, 1)] {
        let alpha = (u16::from(theme.glow) * share / 6) as u8;
        canvas.stroke_ellipse_stroked(
            center,
            radius + spread,
            radius + spread,
            Rgba::with_alpha(a.r, a.g, a.b, alpha),
            &crate::backend::Stroke::new(1.5),
        );
    }
}

/// A soft accent halo just outside `rect` (a primary button), [`Theme::glow`]
/// strong (nothing at 0).
pub fn halo(canvas: &mut dyn Canvas, rect: Rect, radius: f32, theme: &Theme) {
    if theme.glow == 0 {
        return;
    }
    let a = theme.accent;
    for (grow, share) in [(1, 2u16), (2, 1)] {
        let ring = Rect::new(
            rect.left - grow,
            rect.top - grow,
            rect.right + grow,
            rect.bottom + grow,
        );
        let alpha = (u16::from(theme.glow) * share / 4) as u8;
        canvas.stroke_rounded_rect_corners(
            ring,
            [Corner::uniform(radius + grow as f32); 4],
            Rgba::with_alpha(a.r, a.g, a.b, alpha),
            &crate::backend::Stroke::new(1.0),
        );
    }
}

/// `rect` filled with a vertical gradient from `top` to `bottom`, rounded by
/// `radius`, with a 1px `bevel` highlight along its top edge.
fn gradient_face(
    canvas: &mut dyn Canvas,
    rect: Rect,
    radius: f32,
    top: Color,
    bottom: Color,
    bevel: Rgba,
) {
    canvas.push_clip_rounded(rect, [Corner::uniform(radius); 4]);
    canvas.fill_rect_linear(rect, &vertical(rect, top, bottom));
    if bevel.a > 0 {
        canvas.fill_rect_rgba(
            Rect::new(rect.left + 1, rect.top + 1, rect.right - 1, rect.top + 2),
            bevel,
        );
    }
    canvas.pop_clip();
}

/// A top-to-bottom gradient spanning `rect`.
fn vertical(rect: Rect, top: Color, bottom: Color) -> LinearGradient {
    LinearGradient::new(
        Point::new(rect.left, rect.top),
        Point::new(rect.left, rect.bottom),
        vec![
            GradientStop::new(0.0, Rgba::from(top)),
            GradientStop::new(1.0, Rgba::from(bottom)),
        ],
    )
}
