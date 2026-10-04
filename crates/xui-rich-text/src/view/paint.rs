#![forbid(unsafe_code)]

//! Painting: visible paragraphs only, from the cached layout. Nothing here
//! shapes text or allocates per item.

use xui_core::backend::{Canvas, Dash, Rgba, Stroke};
use xui_core::geometry::{Point, Rect};
use xui_core::widget::scrollbar::{self, Orientation, ThumbState};
use xui_core::{Color, Theme};

use super::overlay;
use super::sheets;
use super::state::State;
use crate::layout::{FRect, Layout, Line, ParaLayout, PlacedItem, PlacedKind};
use crate::model::{CharStyle, DocPos, Document, Selection, TextColor};

/// The contrast (WCAG ratio) automatic text must keep against a highlight.
const MIN_CONTRAST: f32 = 4.5;

/// Paints the document, its selection and the scrollbar.
///
/// The software compositor reports [`Canvas::bounds`] as the node's window
/// rectangle with no translation applied, while pointer events arrive
/// node-local; so the origin moves to the node's corner and everything below
/// works in node-local coordinates. On a canvas whose bounds already sit at the
/// origin this is a no-op.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &mut State, theme: &Theme) {
    let origin = canvas.bounds();
    let bounds = Rect::from_size(origin.size());
    canvas.save();
    canvas.set_translation(origin.left as f32, origin.top as f32);
    paint_local(canvas, state, theme, bounds);
    canvas.restore();
}

/// Paints into `bounds`, the node's area at the canvas origin.
fn paint_local(canvas: &mut dyn Canvas, state: &mut State, theme: &Theme, bounds: Rect) {
    let dpi = canvas.dpi();
    let text_area = state.prepare(bounds, dpi);
    let track = state.track;
    // On paper the text is what gets printed: dark on white in either theme.
    let paper = Theme::light();
    let ink = match state.sheets {
        Some(sheets) => {
            canvas.fill_rect(bounds, sheets::desk(theme));
            canvas.push_clip(text_area);
            sheets::paint(canvas, state, sheets, theme);
            canvas.pop_clip();
            &paper
        }
        None => {
            canvas.fill_rect(bounds, theme.input_background);
            theme
        }
    };

    let layout = &state.layout;
    let shift = state.shift();
    let pad = state.origin.x;
    let range = layout.visible(shift as f32, (shift + bounds.height()) as f32);
    let painter = Painter {
        theme: ink,
        doc: &state.ed.doc,
        layout,
        selection: Some(state.ed.selection),
        focused: state.focused,
        viewport: state.viewport,
        paged: state.sheets.is_some(),
        shift,
        pad,
        scale: layout.dpi() as f32 / 96.0,
    };
    canvas.push_clip(text_area);
    painter.selection(canvas, range.clone());
    for para in &layout.paragraphs()[range.clone()] {
        painter.paragraph(canvas, para);
    }
    painter.tables(canvas, shift as f32, (shift + bounds.height()) as f32);
    overlay::paint(canvas, state, ink, pad, shift);
    canvas.pop_clip();

    let thumb = if state.bar_drag.is_some() {
        ThumbState::Pressed
    } else {
        ThumbState::Normal
    };
    scrollbar::paint_state(
        canvas,
        track,
        state.bar_scroll(),
        Orientation::Vertical,
        *theme,
        thumb,
    );
}

/// Paints laid-out paragraphs and tables: the editor's view, or an area of a
/// printed sheet.
#[derive(Clone, Copy)]
pub(super) struct Painter<'a> {
    pub(super) theme: &'a Theme,
    pub(super) doc: &'a Document,
    pub(super) layout: &'a Layout,
    /// The selection to paint behind the text (`None` on paper).
    pub(super) selection: Option<Selection>,
    /// Whether the editor has the focus (the selection's colour).
    pub(super) focused: bool,
    /// The painted area's height: lines below it are skipped.
    pub(super) viewport: f32,
    /// Whether the flow is cut into pages (no page-break markers, rules cut
    /// at page edges).
    pub(super) paged: bool,
    /// The scroll offset in whole pixels.
    pub(super) shift: i32,
    /// The left margin in whole pixels.
    pub(super) pad: i32,
    pub(super) scale: f32,
}

impl Painter<'_> {
    pub(super) fn rect(&self, r: Rect) -> Rect {
        r.offset(self.pad, -self.shift)
    }

    fn frect(&self, r: &FRect, para_y: f32) -> Rect {
        self.rect(r.shifted(para_y).to_rect())
    }

    fn color(&self, style: &CharStyle) -> Rgba {
        let auto = if style.link.is_some() {
            self.theme.accent
        } else {
            self.theme.text
        };
        match (style.color, style.highlight) {
            (TextColor::Fixed(c), _) => c.into(),
            (TextColor::Auto, None) => auto.into(),
            // Automatic text on a highlight must stay readable in either
            // theme: keep the theme's colour when it contrasts enough with
            // the highlight, else take it or its inverse, whichever contrasts
            // more.
            (TextColor::Auto, Some(back)) if auto.contrast_ratio(back) >= MIN_CONTRAST => {
                auto.into()
            }
            (TextColor::Auto, Some(back)) => {
                let text = self.theme.text;
                let inverse = Color::rgb(255 - text.r, 255 - text.g, 255 - text.b);
                if text.contrast_ratio(back) >= inverse.contrast_ratio(back) {
                    text.into()
                } else {
                    inverse.into()
                }
            }
        }
    }

    /// Paints the selection of the visible paragraphs behind the text.
    fn selection(&self, canvas: &mut dyn Canvas, visible: std::ops::Range<usize>) {
        let (Some(Selection::Text { anchor: a, head: b }), Some(last)) =
            (self.selection, visible.end.checked_sub(1))
        else {
            return;
        };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let doc = &self.doc;
        let from = a.max(DocPos::new(visible.start, 0));
        let to = b.min(DocPos::new(last, doc.paragraphs()[last].text().len()));
        if from >= to {
            return;
        }
        for rect in self.layout.selection_rects(doc, from, to) {
            let color = if self.focused {
                self.theme.selection
            } else {
                self.theme.selection_unfocused
            };
            canvas.fill_rect(self.rect(rect), color);
        }
    }

    pub(super) fn paragraph(&self, canvas: &mut dyn Canvas, para: &ParaLayout) {
        let height = self.viewport as i32;
        if let Some((x, top, bottom)) = para.rule {
            let w = (2.0 * self.scale).round().max(1.0) as i32;
            let x = x.round() as i32;
            let bar = |canvas: &mut dyn Canvas, top: f32, bottom: f32| {
                let rect = Rect::new(
                    x,
                    (para.y + top).round() as i32,
                    x + w,
                    (para.y + bottom).round() as i32,
                );
                canvas.fill_rect(self.rect(rect), self.theme.text_secondary);
            };
            if self.paged {
                // Line by line, so the rule stops at a page's edge.
                for line in &para.lines {
                    bar(canvas, line.y, line.y + line.height);
                }
            } else {
                bar(canvas, top, bottom);
            }
        }
        if !self.paged {
            self.page_break_marker(canvas, para);
        }
        if let Some(marker) = &para.marker {
            let origin = Point::new(
                marker.x.round() as i32 + self.pad,
                (para.y + marker.y).round() as i32 - self.shift,
            );
            let style = self.doc.styles().char(marker.style);
            canvas.draw_layout(marker.layout.as_ref(), origin, self.color(style));
        }
        for float in &para.floats {
            if let Some(object) = self.doc.objects().get(float.id) {
                canvas.draw_image(&object.image, self.frect(&float.rect, para.y));
            }
        }
        for line in &para.lines {
            let top = (para.y + line.y).round() as i32 - self.shift;
            if top > height || top + line.height.ceil() as i32 + 1 < 0 {
                continue;
            }
            for item in &line.items {
                self.item(canvas, para.y, line, item);
            }
        }
    }

    /// In draft view, a dashed rule across the top of a paragraph that starts
    /// a new page.
    fn page_break_marker(&self, canvas: &mut dyn Canvas, para: &ParaLayout) {
        if !para.page_break {
            return;
        }
        let y = para.y.round() as i32 - self.shift;
        let right = self.layout.width().round() as i32 + self.pad;
        let stroke = Stroke::new(1.0).dash(Dash::Dashed);
        canvas.draw_line_stroked(
            Point::new(self.pad, y),
            Point::new(right, y),
            self.theme.text_secondary.into(),
            &stroke,
        );
    }

    fn item(&self, canvas: &mut dyn Canvas, para_y: f32, line: &Line, item: &PlacedItem) {
        let style = self.doc.styles().char(item.style);
        let top = para_y + line.y;
        let baseline = para_y + line.baseline + item.dy;
        let (left, right) = (item.x.round() as i32, (item.x + item.width).round() as i32);
        if let Some(Color { r, g, b }) = style.highlight {
            let band = Rect::new(
                left,
                top.round() as i32,
                right,
                (top + line.height).round() as i32,
            );
            canvas.fill_rect(self.rect(band), Color::rgb(r, g, b));
        }
        let color = self.color(style);
        match &item.kind {
            PlacedKind::Text(layout) => {
                let origin = Point::new(
                    left + self.pad,
                    (baseline - layout.baseline()).round() as i32 - self.shift,
                );
                canvas.draw_layout(layout.as_ref(), origin, color);
            }
            PlacedKind::Object { id, height } => {
                if let Some(object) = self.doc.objects().get(*id) {
                    let bottom = para_y + line.baseline;
                    let rect = Rect::new(
                        left,
                        (bottom - height).round() as i32,
                        right,
                        bottom.round() as i32,
                    );
                    canvas.draw_image(&object.image, self.rect(rect));
                }
            }
            _ => {}
        }
        if matches!(item.kind, PlacedKind::Text(_) | PlacedKind::Space { .. }) {
            let thickness = (item.size / 14.0).round().max(1.0) as i32;
            let mut line_at = |y: f32| {
                let y = y.round() as i32;
                let band = Rect::new(left, y, right, y + thickness);
                canvas.fill_rect(self.rect(band), Color::rgb(color.r, color.g, color.b));
            };
            if style.underline || style.link.is_some() {
                line_at(baseline + item.size * 0.12);
            }
            if style.strike {
                line_at(baseline - item.size * 0.3);
            }
        }
    }
}
