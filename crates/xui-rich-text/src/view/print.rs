#![forbid(unsafe_code)]

//! Printing: the document laid out on its pages at a printer's resolution,
//! painted one area of a sheet at a time.
//!
//! The layout is page view's, at the printer's DPI instead of the screen's: the
//! text area is the page's content box and the pitch from one page's text top
//! to the next is a whole sheet, so sheet `k`'s rows map onto flow
//! `k * pitch - margin_top ..` with no overlap between sheets. Painting an
//! area rather than a whole sheet keeps memory flat: a 300 dpi A4 sheet in
//! RGBA is 35 MB, a 256-row band of it 2.5 MB.

use xui_core::backend::{Canvas, TextShaper};
use xui_core::geometry::Rect;
use xui_core::{Dip, Theme};

use super::paint::Painter;
use crate::layout::{Layout, Pages};
use crate::model::{Document, PageSetup};

/// A document laid out on its pages at a printer's resolution.
pub struct Printout {
    doc: Document,
    layout: Layout,
    page: PageSetup,
    dpi: u32,
}

impl Printout {
    /// Lays the whole of `doc` out on its [`PageSetup`] at `dpi` dots per
    /// inch, shaping with `shaper`.
    pub fn new(doc: &Document, shaper: &dyn TextShaper, dpi: u32) -> Printout {
        let dpi = dpi.max(1);
        let page = *doc.page();
        let px = |d: Dip| d.0 * dpi as f32 / 96.0;
        let mut layout = Layout::new();
        layout.set_metrics(px(page.content_width()).max(1.0), dpi);
        layout.set_pages(Pages::new(px(page.content_height()), px(page.height)));
        layout.update(doc, shaper);
        Printout {
            doc: doc.clone(),
            layout,
            page,
            dpi,
        }
    }

    /// The resolution the document was laid out at.
    pub fn dpi(&self) -> u32 {
        self.dpi
    }

    /// How many sheets the document fills (at least one).
    pub fn page_count(&self) -> usize {
        self.layout.page_count()
    }

    /// A sheet's size in pixels, `(width, height)`, as the document is set up:
    /// wider than tall for a landscape page.
    pub fn sheet_size(&self) -> (u32, u32) {
        (self.px(self.page.width), self.px(self.page.height))
    }

    /// Whether the document's pages are landscape.
    pub fn is_landscape(&self) -> bool {
        self.page.is_landscape()
    }

    fn px(&self, d: Dip) -> u32 {
        (d.0 * self.dpi as f32 / 96.0).round().max(1.0) as u32
    }

    /// Paints `area` of sheet `page` (sheet pixels, from its top-left corner)
    /// with its top-left corner at the canvas's: white paper and the text,
    /// pictures and table grids on it, dark on white whatever the theme. Only
    /// `area` is touched. A page past the last one paints blank paper.
    pub fn paint(&self, canvas: &mut dyn Canvas, page: usize, area: Rect) {
        let origin = canvas.bounds();
        let size = Rect::from_size(area.size());
        canvas.save();
        canvas.set_translation(origin.left as f32, origin.top as f32);
        canvas.push_clip(size);
        let paper = Theme::light();
        canvas.fill_rect(size, paper.input_background);
        if page < self.page_count() {
            self.paint_text(canvas, &paper, page, area);
        }
        canvas.pop_clip();
        canvas.restore();
    }

    fn paint_text(&self, canvas: &mut dyn Canvas, paper: &Theme, page: usize, area: Rect) {
        let scale = self.dpi as f32 / 96.0;
        let pitch = self.layout.pages().map_or(0.0, |p| p.pitch);
        let margin_top = self.page.top.0 * scale;
        let margin_left = self.page.left.0 * scale;
        // Layout y of the area's top row; layout x of its left column.
        let shift = (page as f32 * pitch - margin_top).round() as i32 + area.top;
        let pad = margin_left.round() as i32 - area.left;
        let height = area.height();
        let painter = Painter {
            theme: paper,
            doc: &self.doc,
            layout: &self.layout,
            selection: None,
            focused: false,
            viewport: height as f32,
            paged: true,
            shift,
            pad,
            scale,
        };
        let (top, bottom) = (shift as f32, (shift + height) as f32);
        for para in &self.layout.paragraphs()[self.layout.visible(top, bottom)] {
            painter.paragraph(canvas, para);
        }
        painter.tables(canvas, top, bottom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xui_canvas::Surface;
    use xui_core::backend::Backend;

    use crate::model::mm;

    fn doc(text: &str, page: PageSetup) -> Document {
        Document::from_plain_text(text)
            .with_page(page)
            .expect("a valid page")
    }

    fn shaper() -> Box<dyn TextShaper> {
        xui_canvas::OffscreenBackend::new().text_shaper()
    }

    /// The darkest pixel's luminance in `pixels` (RGBA rows).
    fn darkest(pixels: &[u8]) -> u8 {
        pixels
            .chunks_exact(4)
            .map(|p| p[0].min(p[1]).min(p[2]))
            .min()
            .unwrap_or(255)
    }

    #[test]
    fn a_sheet_is_the_page_at_the_printer_resolution() {
        let shaper = shaper();
        let out = Printout::new(&doc("Hello", PageSetup::a4()), shaper.as_ref(), 300);
        assert_eq!(out.sheet_size(), (2480, 3508));
        assert_eq!(out.page_count(), 1);
        assert!(!out.is_landscape());
        let turned = Printout::new(
            &doc("Hello", PageSetup::a4().rotated()),
            shaper.as_ref(),
            300,
        );
        assert_eq!(turned.sheet_size(), (3508, 2480));
        assert!(turned.is_landscape());
    }

    #[test]
    fn long_text_runs_onto_more_sheets() {
        let shaper = shaper();
        let text = "line\n".repeat(400);
        let page = PageSetup::a4().with_margins(mm(25.0));
        let screen = Printout::new(&doc(&text, page), shaper.as_ref(), 96);
        let paper = Printout::new(&doc(&text, page), shaper.as_ref(), 300);
        assert!(screen.page_count() > 1);
        // The same pages at any resolution, give or take rounding at a break.
        assert!(screen.page_count().abs_diff(paper.page_count()) <= 1);
    }

    #[test]
    fn text_lands_inside_the_margins_and_nowhere_else() {
        let shaper = shaper();
        let page = PageSetup {
            width: Dip(200.0),
            height: Dip(200.0),
            left: Dip(50.0),
            top: Dip(50.0),
            right: Dip(50.0),
            bottom: Dip(50.0),
        };
        let out = Printout::new(&doc("Hello world", page), shaper.as_ref(), 96);
        let paint = |area: Rect| {
            let mut surface = Surface::new(area.width() as u32, area.height() as u32);
            surface.with_canvas_at(Rect::from_size(area.size()), 96, |canvas| {
                out.paint(canvas, 0, area);
            });
            surface.pixels().to_vec()
        };
        // The top margin is blank paper; the text area's first line is not.
        assert_eq!(darkest(&paint(Rect::new(0, 0, 200, 50))), 255);
        assert!(darkest(&paint(Rect::new(50, 50, 150, 80))) < 128);
        // A band of the text area painted on its own matches the same pixels
        // cut out of the whole sheet.
        let whole = paint(Rect::new(0, 0, 200, 200));
        let band = paint(Rect::new(40, 45, 160, 75));
        for row in 0..30 {
            let from = ((45 + row) * 200 + 40) * 4;
            assert_eq!(
                &band[row * 120 * 4..(row + 1) * 120 * 4],
                &whole[from..from + 120 * 4],
                "row {row}"
            );
        }
    }

    #[test]
    fn a_page_past_the_end_is_blank_paper() {
        let shaper = shaper();
        let out = Printout::new(&doc("Hello", PageSetup::a4()), shaper.as_ref(), 96);
        let mut surface = Surface::new(64, 64);
        surface.with_canvas_at(Rect::new(0, 0, 64, 64), 96, |canvas| {
            out.paint(canvas, 3, Rect::new(90, 90, 154, 154));
        });
        assert!(surface.pixels().iter().all(|&b| b == 255));
    }
}
