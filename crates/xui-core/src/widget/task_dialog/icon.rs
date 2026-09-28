#![forbid(unsafe_code)]

//! [`TaskDialogIcon`] and its portable vector painting.

use crate::backend::Canvas;
use crate::geometry::{Point, Rect};
use crate::theme::Theme;

/// The icon drawn beside a [`TaskDialog`](super::TaskDialog)'s title.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskDialogIcon {
    /// No icon (the default).
    #[default]
    None,
    /// An information circle, tinted with [`Theme::accent`].
    Info,
    /// A warning triangle, tinted with [`Theme::warning`].
    Warning,
    /// An error circle, tinted with [`Theme::danger`].
    Error,
    /// A shield, tinted with [`Theme::accent`] (a UAC-style prompt).
    Shield,
}

/// Draws `icon` in `rect`: a filled glyph in the icon's tinted colour with the
/// mark cut out in the card's background, so it needs no image dependency.
pub(super) fn draw_icon(
    canvas: &mut dyn Canvas,
    icon: TaskDialogIcon,
    rect: Rect,
    theme: Theme,
    dpi: u32,
) {
    let color = match icon {
        TaskDialogIcon::None => return,
        TaskDialogIcon::Info | TaskDialogIcon::Shield => theme.accent,
        TaskDialogIcon::Warning => theme.warning,
        TaskDialogIcon::Error => theme.danger,
    };
    let size = rect.width().min(rect.height());
    if size < 6 {
        return;
    }
    let inner = rect.shrink(size / 8);
    let cx = inner.left + inner.width() / 2;
    let cy = inner.top + inner.height() / 2;
    let radius = (inner.width().min(inner.height()) / 2).max(2) as f32;
    let r = radius as i32;
    let mark = theme.raised;
    let stroke = (dpi as f32 / 96.0 * 2.0).max(1.0);
    match icon {
        TaskDialogIcon::None => {}
        TaskDialogIcon::Info => {
            canvas.fill_ellipse(Point::new(cx, cy), radius, radius, color);
            let dot = (radius / 4.0).max(1.0);
            canvas.fill_ellipse(Point::new(cx, cy - r * 2 / 5), dot, dot, mark);
            canvas.draw_line(
                Point::new(cx, cy - r / 8),
                Point::new(cx, cy + r / 2),
                mark,
                stroke,
            );
        }
        TaskDialogIcon::Warning => {
            let triangle = [
                Point::new(cx, cy - r),
                Point::new(cx - r, cy + r),
                Point::new(cx + r, cy + r),
            ];
            canvas.fill_polygon(&triangle, color);
            canvas.draw_line(
                Point::new(cx, cy - r / 3),
                Point::new(cx, cy + r / 4),
                mark,
                stroke,
            );
            let dot = (radius / 5.0).max(1.0);
            canvas.fill_ellipse(Point::new(cx, cy + r * 3 / 5), dot, dot, mark);
        }
        TaskDialogIcon::Error => {
            canvas.fill_ellipse(Point::new(cx, cy), radius, radius, color);
            let arm = r * 3 / 5;
            canvas.draw_line(
                Point::new(cx - arm, cy - arm),
                Point::new(cx + arm, cy + arm),
                mark,
                stroke,
            );
            canvas.draw_line(
                Point::new(cx - arm, cy + arm),
                Point::new(cx + arm, cy - arm),
                mark,
                stroke,
            );
        }
        TaskDialogIcon::Shield => {
            let shield = [
                Point::new(cx - r, cy - r),
                Point::new(cx + r, cy - r),
                Point::new(cx + r, cy),
                Point::new(cx, cy + r),
                Point::new(cx - r, cy),
            ];
            canvas.fill_polygon(&shield, color);
            canvas.draw_line(
                Point::new(cx - r / 2, cy),
                Point::new(cx - r / 6, cy + r / 3),
                mark,
                stroke,
            );
            canvas.draw_line(
                Point::new(cx - r / 6, cy + r / 3),
                Point::new(cx + r / 2, cy - r / 3),
                mark,
                stroke,
            );
        }
    }
}
