#![forbid(unsafe_code)]

//! The tree's painter: background, indent guides, selection, the filled
//! chevron, per-row checkboxes and labels.

use super::flatten::{self, State};
use super::model::{CheckState, NodeId};
use crate::Color;
use crate::backend::{Canvas, TextStyle};
use crate::geometry::{Point, Rect};
use crate::theme::Theme;

/// The per-paint flags the app controls.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub(crate) current: Option<NodeId>,
    pub(crate) hover: Option<NodeId>,
    pub(crate) enabled: bool,
    pub(crate) checkboxes: bool,
    pub(crate) outline: bool,
}

/// Paints `state` into `canvas` from semantic theme tokens.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &State, theme: &Theme, options: Options) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    canvas.clear(theme.background);

    let row_height = flatten::ROW.to_px(dpi).value().max(1);
    let mut slot = 0;
    for (index, node) in state.rows.iter().enumerate() {
        if !flatten::is_visible(&state.rows, index) {
            continue;
        }
        let top = bounds.top + row_height * slot;
        slot += 1;
        let rect = Rect::new(bounds.left, top, bounds.right, top + row_height);
        let is_current = options.current == Some(node.id);
        let fill = if is_current {
            Some(theme.accent)
        } else if options.hover == Some(node.id) {
            Some(theme.hover)
        } else {
            None
        };
        if let Some(fill) = fill {
            canvas.fill_rect(rect, fill);
        }
        let color = match (options.enabled, is_current) {
            (false, _) => theme.text_disabled,
            (true, true) => theme.text_on_accent,
            (true, false) => theme.text,
        };

        let guide = guide_color(theme, fill);
        for level in 0..node.depth {
            if !flatten::guide_continues(&state.rows, index, level) {
                continue;
            }
            let x = flatten::guide_x(dpi, bounds.left, level);
            canvas.draw_line(
                Point::new(x, top),
                Point::new(x, top + row_height),
                guide,
                1.0,
            );
        }

        let level = flatten::level_x(dpi, bounds.left, node.depth);
        if node.expandable {
            draw_chevron(canvas, dpi, level, top, row_height, node.expanded, color);
        }
        if options.checkboxes {
            let square = flatten::checkbox_rect(dpi, bounds.left, top, node.depth);
            draw_check(canvas, square, node.checked, options.enabled, theme);
        }
        if let Some(icon) = &node.icon {
            let icon_box =
                flatten::icon_rect(dpi, bounds.left, top, node.depth, options.checkboxes);
            super::icon::draw(canvas, icon, icon_box, color, dpi);
        }

        let label = Rect::new(
            flatten::label_x(
                dpi,
                bounds.left,
                node.depth,
                options.checkboxes,
                node.icon.is_some(),
            ),
            top,
            bounds.right - flatten::pad(dpi),
            top + row_height,
        );
        let style = TextStyle::new(color, flatten::TEXT).middle();
        canvas.draw_text(&node.label, label, &style);
    }

    if options.outline {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// The indent-guide colour for a row: [`Theme::border`] on a plain row, but
/// blended toward the row's own fill on a selected or hovered row, so the
/// guide stays a subtle hint instead of a high-contrast line across the
/// highlight (issue #121).
pub(crate) fn guide_color(theme: &Theme, fill: Option<Color>) -> Color {
    match fill {
        Some(fill) => theme.border.lerp(fill, 0.6),
        None => theme.border,
    }
}

fn draw_chevron(
    canvas: &mut dyn Canvas,
    dpi: u32,
    level: i32,
    top: i32,
    row_height: i32,
    expanded: bool,
    color: crate::Color,
) {
    let chevron = flatten::chevron_px(dpi);
    let (cx, cy) = (level + chevron / 2, top + row_height / 2);
    let half = (chevron / 5).max(2);
    let triangle = if expanded {
        [
            Point::new(cx - half, cy - half),
            Point::new(cx + half, cy - half),
            Point::new(cx, cy + half),
        ]
    } else {
        [
            Point::new(cx - half, cy - half),
            Point::new(cx - half, cy + half),
            Point::new(cx + half, cy),
        ]
    };
    canvas.fill_polygon(&triangle, color);
}

fn draw_check(
    canvas: &mut dyn Canvas,
    square: Rect,
    state: CheckState,
    enabled: bool,
    theme: &Theme,
) {
    canvas.fill_rect(square, theme.input_background);
    canvas.stroke_rect(
        square,
        if enabled {
            theme.input_border
        } else {
            theme.text_disabled
        },
        1.0,
    );
    let mark = if enabled {
        theme.accent
    } else {
        theme.text_disabled
    };
    let (x, y) = (square.left as f32, square.top as f32);
    let s = square.width() as f32;
    match state {
        CheckState::Unchecked => {}
        CheckState::Checked => {
            canvas.draw_line(
                Point::new((x + s * 0.22) as i32, (y + s * 0.52) as i32),
                Point::new((x + s * 0.42) as i32, (y + s * 0.72) as i32),
                mark,
                2.0,
            );
            canvas.draw_line(
                Point::new((x + s * 0.42) as i32, (y + s * 0.72) as i32),
                Point::new((x + s * 0.78) as i32, (y + s * 0.28) as i32),
                mark,
                2.0,
            );
        }
        CheckState::Indeterminate => {
            canvas.draw_line(
                Point::new((x + s * 0.25) as i32, (y + s * 0.5) as i32),
                Point::new((x + s * 0.75) as i32, (y + s * 0.5) as i32),
                mark,
                2.0,
            );
        }
    }
}
