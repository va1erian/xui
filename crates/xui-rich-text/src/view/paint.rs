#![forbid(unsafe_code)]

//! Painting: visible paragraphs only, from the cached layout. Nothing here
//! shapes text or allocates per item.

use xui_core::backend::{Canvas, Rgba};
use xui_core::geometry::{Point, Rect};
use xui_core::widget::scrollbar::{self, Orientation, ThumbState};
use xui_core::{Color, Theme};

use super::state::{State, pad_px};
use crate::layout::{FRect, Line, ParaLayout, PlacedItem, PlacedKind};
use crate::model::{CharStyle, DocPos, TextColor};

/// The contrast (WCAG ratio) automatic text must keep against a highlight.
const MIN_CONTRAST: f32 = 4.5;

/// Paints the document, its selection and the scrollbar.
pub(crate) fn paint(canvas: &mut dyn Canvas, state: &mut State, theme: &Theme) {
    let bounds = canvas.bounds();
    let dpi = canvas.dpi();
    let text_area = state.prepare(bounds, dpi);
    let (track, pad) = (state.track, pad_px(dpi));
    canvas.fill_rect(bounds, theme.input_background);

    let scroll = state.scroll;
    let layout = &state.layout;
    let range = layout.visible(scroll, scroll + bounds.height() as f32);
    let painter = Painter {
        theme,
        state,
        shift: scroll.round() as i32,
        pad,
        scale: dpi as f32 / 96.0,
    };
    canvas.push_clip(text_area);
    painter.selection(canvas, range.clone());
    for para in &layout.paragraphs()[range.clone()] {
        painter.paragraph(canvas, para);
    }
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

struct Painter<'a> {
    theme: &'a Theme,
    state: &'a State,
    /// The scroll offset in whole pixels.
    shift: i32,
    /// The left margin in whole pixels.
    pad: i32,
    scale: f32,
}

impl Painter<'_> {
    fn rect(&self, r: Rect) -> Rect {
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
        let (Some((a, b)), Some(last)) = (self.state.selection, visible.end.checked_sub(1)) else {
            return;
        };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let doc = &self.state.doc;
        let from = a.max(DocPos::new(visible.start, 0));
        let to = b.min(DocPos::new(last, doc.paragraphs()[last].text().len()));
        if from >= to {
            return;
        }
        for rect in self.state.layout.selection_rects(doc, from, to) {
            canvas.fill_rect(self.rect(rect), self.theme.selection);
        }
    }

    fn paragraph(&self, canvas: &mut dyn Canvas, para: &ParaLayout) {
        let height = canvas.bounds().height();
        if let Some((x, top, bottom)) = para.rule {
            let w = (2.0 * self.scale).round().max(1.0) as i32;
            let x = x.round() as i32;
            let rect = Rect::new(
                x,
                (para.y + top).round() as i32,
                x + w,
                (para.y + bottom).round() as i32,
            );
            canvas.fill_rect(self.rect(rect), self.theme.text_secondary);
        }
        if let Some(marker) = &para.marker {
            let origin = Point::new(
                marker.x.round() as i32 + self.pad,
                (para.y + marker.y).round() as i32 - self.shift,
            );
            let style = self.state.doc.styles().char(marker.style);
            canvas.draw_layout(marker.layout.as_ref(), origin, self.color(style));
        }
        for float in &para.floats {
            if let Some(object) = self.state.doc.objects().get(float.id) {
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

    fn item(&self, canvas: &mut dyn Canvas, para_y: f32, line: &Line, item: &PlacedItem) {
        let style = self.state.doc.styles().char(item.style);
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
                if let Some(object) = self.state.doc.objects().get(*id) {
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
