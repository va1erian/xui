#![forbid(unsafe_code)]

//! The grid view's painter: the viewport background, then only the visible
//! tiles, each delegated to its custom painter or the default one.

use super::TilePainter;
use super::layout;
use super::model::TilePaint;
use super::state::{Metrics, State};
use crate::backend::{Canvas, TextStyle};
use crate::geometry::Rect;
use crate::theme::Theme;
use crate::theme::look::{self, backdrop};
use crate::units::Dip;

/// The inset of a default tile's content from its edge.
const INNER: Dip = Dip(6.0);
/// The height of a default tile's caption strip.
const CAPTION: Dip = Dip(18.0);
/// The design size of a default tile's caption.
const CAPTION_SIZE: Dip = Dip(12.0);
/// The corner radius of the selection/hover fill.
const RADIUS: f32 = 5.0;

/// Draws `state` into `canvas`. Only the visible tiles are touched, so a large
/// model costs the same as a small one.
pub(crate) fn paint(
    canvas: &mut dyn Canvas,
    state: &State,
    theme: &Theme,
    painter: &Option<TilePainter>,
    outline: bool,
) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    backdrop(canvas, theme.background);

    let metrics = Metrics::of(state.size, dpi);
    let len = state.len();
    let columns = state.columns(bounds.width(), dpi).max(1);
    let rows = layout::row_count(len, columns);
    let visible = layout::visible_row_range(
        state.offset,
        bounds.height(),
        metrics.height,
        metrics.gap,
        rows,
    );

    canvas.push_clip(bounds);
    for row in visible {
        let top = bounds.top + row as i32 * metrics.row_stride() - state.offset;
        for col in 0..columns {
            let index = row * columns + col;
            if index >= len {
                break;
            }
            let Some(tile) = state.tile(index) else {
                continue;
            };
            let left = bounds.left + col as i32 * metrics.col_stride();
            let rect = Rect::new(left, top, left + metrics.width, top + metrics.height);
            let selected = state.selected == Some(index);
            let hovered = state.hover == Some(index);
            if selected {
                look::face(canvas, rect, RADIUS, theme.selection, theme);
            } else if hovered {
                canvas.fill_rounded_rect(rect, RADIUS, theme.hover);
            }
            let context = TilePaint {
                tile,
                rect,
                index,
                selected,
                hovered,
                theme,
                dpi,
            };
            match painter {
                Some(painter) => painter(canvas, &context),
                None => default_paint(canvas, &context),
            }
        }
    }
    canvas.pop_clip();

    if outline {
        canvas.stroke_rect(bounds, theme.accent, 2.0);
    }
}

/// The default tile painter: art in the upper part, the caption below it.
pub(crate) fn default_paint(canvas: &mut dyn Canvas, paint: &TilePaint<'_>) {
    let rect = paint.rect;
    let inner = INNER.to_px(paint.dpi).value();
    let caption = CAPTION.to_px(paint.dpi).value();

    if let Some(image) = paint.tile.image {
        let art = Rect::new(
            rect.left + inner,
            rect.top + inner,
            rect.right - inner,
            rect.bottom - inner - caption,
        );
        if art.width() > 0 && art.height() > 0 {
            canvas.draw_image(image, art);
        }
    }

    if !paint.tile.label.is_empty() {
        let text = Rect::new(
            rect.left + inner,
            rect.bottom - inner - caption,
            rect.right - inner,
            rect.bottom - inner,
        );
        let style = TextStyle::new(paint.theme.text, CAPTION_SIZE).middle();
        canvas.draw_text(paint.tile.label, text, &style);
    }
}
