#![forbid(unsafe_code)]

//! Painting tables: the grid around and between the cells, and the header row
//! drawn again at the top of each later page a table runs onto.

use xui_core::backend::Canvas;
use xui_core::geometry::Rect;

use super::paint::Painter;
use crate::layout::{Pages, TableLayout};

impl Painter<'_> {
    /// Paints the grids and repeated header rows of the tables that reach
    /// into the layout's vertical span `top..bottom`.
    pub(super) fn tables(&self, canvas: &mut dyn Canvas, top: f32, bottom: f32) {
        let layout = &self.state.layout;
        for t in layout.tables() {
            if t.bottom() < top || t.top() > bottom {
                continue;
            }
            for &copy in &t.repeats {
                self.header_copy(canvas, t, copy);
            }
            if t.border {
                for row in &t.rows {
                    self.grid(canvas, t, row.top, row.bottom());
                }
            }
        }
    }

    /// Paints the first row's cells again with its top at `copy`.
    fn header_copy(&self, canvas: &mut dyn Canvas, t: &TableLayout, copy: f32) {
        let Some(header) = t.rows.first() else {
            return;
        };
        let dy = (copy - header.top).round() as i32;
        let moved = Painter {
            shift: self.shift - dy,
            ..*self
        };
        let paras = self.state.layout.paragraphs();
        for cells in &header.cells {
            for para in &paras[cells.clone()] {
                moved.paragraph(canvas, para);
            }
        }
        if t.border {
            self.grid(canvas, t, copy, copy + header.height);
        }
    }

    /// Paints the grid of one row spanning `top..bottom`, cut at the page
    /// gaps it crosses so no line runs over the desk between sheets.
    fn grid(&self, canvas: &mut dyn Canvas, t: &TableLayout, top: f32, bottom: f32) {
        let width = self.scale.round().max(1.0) as i32;
        let (Some(&left), Some(&right)) = (t.edges.first(), t.edges.last()) else {
            return;
        };
        let (left, right) = (left.round() as i32, right.round() as i32);
        for (from, to) in segments(self.state.layout.pages(), top, bottom) {
            let (from, to) = (from.round() as i32, to.round() as i32);
            let color = self.theme.text;
            canvas.fill_rect(
                self.rect(Rect::new(left, from, right + width, from + width)),
                color,
            );
            canvas.fill_rect(
                self.rect(Rect::new(left, to, right + width, to + width)),
                color,
            );
            for edge in &t.edges {
                let x = edge.round() as i32;
                canvas.fill_rect(self.rect(Rect::new(x, from, x + width, to + width)), color);
            }
        }
    }
}

/// `top..bottom` cut into the parts that lie on pages (the whole span off
/// pages).
fn segments(pages: Option<Pages>, top: f32, bottom: f32) -> impl Iterator<Item = (f32, f32)> {
    let mut at = top;
    std::iter::from_fn(move || {
        if at >= bottom {
            return None;
        }
        let Some(pages) = pages else {
            at = bottom;
            return Some((top, bottom));
        };
        let page = pages.index_at(at);
        let start = at.max(pages.top(page));
        let end = bottom.min(pages.bottom(page));
        at = pages.top(page + 1);
        Some((start, end.max(start)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_is_cut_at_page_gaps() {
        let pages = Pages::new(100.0, 150.0);
        let parts: Vec<_> = segments(pages, 60.0, 190.0).collect();
        assert_eq!(parts, vec![(60.0, 100.0), (150.0, 190.0)]);
        let whole: Vec<_> = segments(None, 60.0, 190.0).collect();
        assert_eq!(whole, vec![(60.0, 190.0)]);
    }
}
