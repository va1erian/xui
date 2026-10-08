#![forbid(unsafe_code)]

//! Where the completion popup sits and how it is drawn.
//!
//! The popup is painted inside the editor's own surface, just below the caret
//! line (above it when it does not fit), so the layout is plain arithmetic over
//! the grid metrics. The painter and the mouse handling share [`Layout`], which
//! keeps what is drawn and what is clickable the same.

use xui_core::backend::{Canvas, TextAlign};
use xui_core::geometry::Rect;
use xui_core::units::Dip;

use crate::completion::Popup;
use crate::metrics::Viewport;
use crate::state::EditorState;
use crate::text::display_col;
use crate::theme::EditorTheme;

/// The widest the popup grows, in character cells.
const MAX_COLS: usize = 56;
/// The narrowest the popup shrinks, in character cells.
const MIN_COLS: usize = 16;
/// The cells the kind marker and its gap take before the label.
const MARKER_COLS: usize = 2;
/// The cells between the label and the detail.
const DETAIL_GAP_COLS: usize = 2;
/// The border width in pixels.
const BORDER: i32 = 1;
/// The design padding either side of a row's content.
const PADDING: Dip = Dip(4.0);

/// The popup's rectangle and how its rows map onto the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    /// The whole popup, border included, in node-local pixels.
    pub(crate) rect: Rect,
    /// The list row shown first.
    pub(crate) first_row: usize,
    /// How many rows are shown.
    pub(crate) rows: usize,
    /// One row's height.
    pub(crate) row_height: i32,
    /// The padding either side of a row's content.
    pub(crate) pad: i32,
    /// One character cell's width.
    pub(crate) advance: i32,
}

impl Layout {
    /// Places `popup` for the caret in `state`, or `None` when the caret's line
    /// is scrolled out of view (there is nothing to hang the popup from).
    pub(crate) fn compute(
        state: &EditorState,
        popup: &Popup,
        viewport: &Viewport,
        dpi: u32,
    ) -> Option<Layout> {
        let metrics = viewport.metrics;
        let text = viewport.text;
        let buffer = &state.buffer;
        let caret = state.view.caret.min(buffer.len_chars());
        let line = buffer.line_of_char(caret);
        let first_line = state
            .view
            .first_line
            .min(buffer.line_count().saturating_sub(1));
        if line < first_line || line >= first_line + viewport.visible_lines {
            return None;
        }
        let line_top = metrics.y_of_line(text, line, first_line);
        let line_start = buffer.line_start(line);
        let anchor = popup.start().clamp(line_start, caret.max(line_start));
        let col = display_col(
            &buffer.line_string(line),
            anchor - line_start,
            state.options.tab_width,
        );
        let anchor_x = metrics.x_of_col(text, col, state.view.first_col);

        let pad = PADDING.to_px(dpi).value();
        let cols = content_cols(popup);
        let bounds = viewport.bounds;
        let width = (cols as i32 * metrics.advance + 2 * (pad + BORDER)).min(bounds.width());
        let left = anchor_x.min(bounds.right - width).max(bounds.left);

        let wanted = popup.visible_rows();
        let chrome = 2 * BORDER;
        let below = text.bottom - (line_top + metrics.line_height);
        let above = line_top - text.top;
        let fits = |space: i32| space >= wanted as i32 * metrics.line_height + chrome;
        let (place_below, space) = if fits(below) || (!fits(above) && below >= above) {
            (true, below)
        } else {
            (false, above)
        };
        let rows = (((space - chrome) / metrics.line_height).max(1) as usize).min(wanted);
        let height = rows as i32 * metrics.line_height + chrome;
        let top = if place_below {
            line_top + metrics.line_height
        } else {
            line_top - height
        };
        let first_row = popup
            .first()
            .max((popup.selected() + 1).saturating_sub(rows))
            .min(popup.selected());
        Some(Layout {
            rect: Rect::new(left, top, left + width, top + height),
            first_row,
            rows,
            row_height: metrics.line_height,
            pad,
            advance: metrics.advance,
        })
    }

    /// The list row under node-local `(x, y)`, or `None` outside the rows.
    pub(crate) fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        let rows_top = self.rect.top + BORDER;
        let inside_x = x > self.rect.left && x < self.rect.right - BORDER;
        if !inside_x || y < rows_top {
            return None;
        }
        let offset = ((y - rows_top) / self.row_height) as usize;
        (offset < self.rows).then_some(self.first_row + offset)
    }

    /// Whether node-local `(x, y)` is anywhere on the popup.
    pub(crate) fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x < self.rect.right && y >= self.rect.top && y < self.rect.bottom
    }

    /// The rectangle of the row at `offset` rows below the first shown one.
    fn row_rect(&self, offset: usize) -> Rect {
        let top = self.rect.top + BORDER + offset as i32 * self.row_height;
        Rect::new(
            self.rect.left + BORDER,
            top,
            self.rect.right - BORDER,
            top + self.row_height,
        )
    }
}

/// The popup's content width in character cells: the marker, the widest label
/// and the widest detail, kept between [`MIN_COLS`] and [`MAX_COLS`].
fn content_cols(popup: &Popup) -> usize {
    let mut label = 0;
    let mut detail = 0;
    for row in 0..popup.len() {
        if let Some(item) = popup.item(row) {
            label = label.max(item.label.chars().count());
            detail = detail.max(item.detail.as_ref().map_or(0, |d| d.chars().count()));
        }
    }
    let gap = if detail > 0 { DETAIL_GAP_COLS } else { 0 };
    (MARKER_COLS + label + gap + detail).clamp(MIN_COLS, MAX_COLS)
}

/// Draws the completion popup over the editor, if one is open.
pub(crate) fn paint(
    canvas: &mut dyn Canvas,
    state: &EditorState,
    theme: &EditorTheme,
    viewport: &Viewport,
) {
    let Some(popup) = &state.completion else {
        return;
    };
    let Some(layout) = Layout::compute(state, popup, viewport, canvas.dpi()) else {
        return;
    };
    canvas.fill_rect(layout.rect, theme.popup_background);
    canvas.stroke_rect(layout.rect, theme.popup_border, 1.0);
    canvas.push_clip(layout.rect);
    for offset in 0..layout.rows {
        let row = layout.first_row + offset;
        let Some(item) = popup.item(row) else {
            break;
        };
        let rect = layout.row_rect(offset);
        let selected = row == popup.selected();
        if selected {
            canvas.fill_rect(rect, theme.popup_selection);
        }
        let ink = if selected {
            theme.popup_selection_text
        } else {
            theme.popup_text
        };
        let marker = Rect::new(
            rect.left + layout.pad,
            rect.top,
            rect.left + layout.pad + layout.advance * 2,
            rect.bottom,
        );
        let kind_style = state.options.font.style(theme.kind_color(item.kind));
        canvas.draw_text(&item.kind.letter().to_string(), marker, &kind_style);

        let label_left = marker.left + layout.advance * MARKER_COLS as i32;
        let label_cells = item.label.chars().count() as i32;
        let label = Rect::new(label_left, rect.top, rect.right - layout.pad, rect.bottom);
        canvas.draw_text(&item.label, label, &state.options.font.style(ink));

        if let Some(detail) = &item.detail {
            let from = label_left + (label_cells + DETAIL_GAP_COLS as i32) * layout.advance;
            let area = Rect::new(from, rect.top, rect.right - layout.pad, rect.bottom);
            if area.width() > 0 {
                let mut style = state.options.font.style(theme.popup_detail);
                style.align = TextAlign::End;
                canvas.push_clip(area);
                canvas.draw_text(detail, area, &style);
                canvas.pop_clip();
            }
        }
    }
    canvas.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::{Completion, CompletionItem, CompletionKind, MAX_ROWS};
    use crate::metrics::Metrics;
    use crate::options::Options;
    use crate::platform::InProcessClipboard;
    use crate::theme::EditorTheme;
    use xui_core::backend::TextMetrics;
    use xui_core::theme::Theme;

    fn state(text: &str, caret: usize) -> EditorState {
        let mut state = EditorState::new(text, Options::default(), Box::new(InProcessClipboard));
        state.view.caret = caret;
        state.view.anchor = caret;
        state
    }

    /// A 10x20 cell grid in a `width` x `height` editor with no gutter.
    fn viewport(width: i32, height: i32) -> Viewport {
        let metrics = Metrics::new(
            TextMetrics {
                width: 100,
                height: 20,
                ascent: 16,
                descent: 4,
            },
            10,
            false,
            96,
        );
        Viewport::split(Rect::new(0, 0, width, height), metrics, 10, 10, 96)
    }

    fn popup(count: usize, start: usize) -> Popup {
        let items = (0..count)
            .map(|n| CompletionItem::new(format!("item{n}"), CompletionKind::Other))
            .collect();
        Popup::open(Completion { start, items }, "").expect("matches")
    }

    #[test]
    fn the_popup_hangs_below_the_caret_line_at_the_words_column() {
        let state = state("abc de\nxyz", 6);
        let layout = Layout::compute(&state, &popup(3, 4), &viewport(400, 300), 96).expect("fits");
        let text = viewport(400, 300).text;
        assert_eq!(
            layout.rect.left,
            text.left + 4 * 10,
            "starts under the word"
        );
        assert_eq!(layout.rect.top, text.top + 20, "just under line 0");
        assert_eq!(layout.rows, 3);
        assert_eq!(layout.rect.height(), 3 * 20 + 2);
    }

    #[test]
    fn it_flips_above_the_caret_line_when_there_is_no_room_below() {
        let text = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let mut state = state(text, 16);
        state.view.first_line = 0;
        let view = viewport(400, 200);
        // Line 8 is at y=160; 4 rows (82px) do not fit in the 20px below it.
        let layout = Layout::compute(&state, &popup(4, 16), &view, 96).expect("fits above");
        assert_eq!(layout.rect.bottom, 160, "its bottom meets the caret line");
        assert_eq!(layout.rows, 4);
    }

    #[test]
    fn it_takes_the_roomier_side_and_shows_fewer_rows_when_neither_fits() {
        let mut state = state("a\nb\nc\nd\ne", 4);
        state.view.first_line = 0;
        let view = viewport(400, 100);
        // Line 2 at y=40: 40px above, 40px below the line (60-100 => 40).
        let layout = Layout::compute(&state, &popup(8, 4), &view, 96).expect("laid out");
        assert_eq!(layout.rows, 1, "38px of room holds one 20px row");
        assert!(layout.rect.top >= 0 && layout.rect.bottom <= 100);
    }

    #[test]
    fn it_is_clamped_inside_the_editor_horizontally() {
        let state = state("abcdefghijklmnopqrstuvwxyz0123456789", 36);
        let view = viewport(300, 300);
        let layout = Layout::compute(&state, &popup(2, 30), &view, 96).expect("fits");
        assert!(layout.rect.right <= 300, "kept inside the right edge");
        assert!(layout.rect.left >= 0);
        let narrow = Layout::compute(&state, &popup(2, 30), &viewport(80, 300), 96).expect("fits");
        assert_eq!(narrow.rect.left, 0);
        assert_eq!(narrow.rect.width(), 80, "shrunk to the editor");
    }

    #[test]
    fn the_caret_line_must_be_on_screen() {
        let mut state = state("a\nb\nc\nd\ne\nf\ng\nh\ni\nj", 0);
        state.view.first_line = 5;
        assert!(Layout::compute(&state, &popup(2, 0), &viewport(400, 100), 96).is_none());
    }

    #[test]
    fn rows_map_to_list_positions_and_clicks_hit_test() {
        let state = state("ab", 2);
        let mut popup = popup(20, 0);
        popup.select(12);
        let layout = Layout::compute(&state, &popup, &viewport(400, 300), 96).expect("fits");
        assert_eq!(layout.rows, MAX_ROWS);
        assert_eq!(layout.first_row, popup.first());
        let x = layout.rect.left + 5;
        let y = layout.rect.top + BORDER + 3 * 20 + 2;
        assert_eq!(layout.row_at(x, y), Some(layout.first_row + 3));
        assert_eq!(layout.row_at(x, layout.rect.top), None, "the border");
        assert_eq!(layout.row_at(x, layout.rect.bottom), None, "below it");
        assert_eq!(layout.row_at(layout.rect.left - 1, y), None);
        assert!(layout.contains(x, layout.rect.top));
        assert!(!layout.contains(x, layout.rect.bottom));
    }

    #[test]
    fn the_selection_stays_visible_when_fewer_rows_fit() {
        let mut state = state("a\nb\nc\nd\ne", 4);
        state.view.first_line = 0;
        let mut popup = popup(8, 4);
        popup.select(7);
        let layout = Layout::compute(&state, &popup, &viewport(400, 100), 96).expect("laid out");
        assert_eq!(layout.rows, 1);
        assert_eq!(layout.first_row, 7);
    }

    #[test]
    fn the_width_follows_the_longest_label_and_detail() {
        let items = vec![
            CompletionItem::new("a", CompletionKind::Other),
            CompletionItem::new("a_longer_label", CompletionKind::Other).with_detail("detail"),
        ];
        let popup = Popup::open(Completion { start: 0, items }, "").expect("matches");
        // 2 + 14 + 2 + 6 = 24 cells.
        assert_eq!(content_cols(&popup), 24);
        let short = Popup::open(
            Completion {
                start: 0,
                items: vec![CompletionItem::new("a", CompletionKind::Other)],
            },
            "",
        )
        .expect("matches");
        assert_eq!(content_cols(&short), MIN_COLS);
        let wide = Popup::open(
            Completion {
                start: 0,
                items: vec![CompletionItem::new("x".repeat(200), CompletionKind::Other)],
            },
            "",
        )
        .expect("matches");
        assert_eq!(content_cols(&wide), MAX_COLS);
    }

    fn render(state: &EditorState, xui_theme: Theme) -> xui_canvas::RgbaImage {
        let theme = EditorTheme::from_theme(xui_theme);
        let mut surface = xui_canvas::Surface::new(300, 200);
        surface.with_canvas_at(Rect::new(0, 0, 300, 200), 96, |canvas| {
            crate::paint::paint(canvas, state, &theme, &xui_theme);
        });
        surface.to_image()
    }

    fn count(image: &xui_canvas::RgbaImage, color: xui_core::Color) -> usize {
        let mut found = 0;
        for y in 0..image.height {
            for x in 0..image.width {
                if image.pixel(x, y) == Some([color.r, color.g, color.b, 0xFF]) {
                    found += 1;
                }
            }
        }
        found
    }

    #[test]
    fn an_open_popup_is_painted_in_light_and_dark() {
        for xui_theme in [Theme::light(), Theme::dark()] {
            let theme = EditorTheme::from_theme(xui_theme);
            let mut state = state("pr", 2);
            state.focused = true;
            state.blink_on = false;
            let closed = render(&state, xui_theme);
            assert_eq!(count(&closed, theme.popup_selection), 0);
            state.completion = Some(popup(3, 0));
            let open = render(&state, xui_theme);
            assert_ne!(open.pixels, closed.pixels, "the popup is drawn");
            assert!(count(&open, theme.popup_selection) > 0, "the selected row");
        }
    }

    #[test]
    fn the_popup_follows_the_selection_and_the_scroll() {
        let xui_theme = Theme::light();
        let mut state = state("pr", 2);
        state.focused = true;
        state.blink_on = false;
        let mut open = popup(20, 0);
        state.completion = Some(popup(20, 0));
        let first = render(&state, xui_theme);
        open.move_by(1, false);
        state.completion = Some(open);
        let second = render(&state, xui_theme);
        assert_ne!(first.pixels, second.pixels, "the highlight moved");
    }
}
