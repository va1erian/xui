//! The [`DocumentContainer`] implementation for [`D2dContainer`]: litehtml's
//! draw callbacks translated into the neutral [`Cmd`] display list. Kept in its
//! own file so `mod.rs` stays small; behaviour is unchanged.

use litehtml::{
    BackgroundLayer, Border, BorderRadiuses, BorderStyle, Borders, Color, ConicGradient,
    DocumentContainer, DrawContext, FontDescription, FontHandle, FontMetrics, FontStyle,
    LinearGradient, ListMarker, ListStyleType, MediaFeatures, MediaType, Position, RadialGradient,
    Size, TextTransform,
};
use xui_win32::d2d::FontSpec;

use super::{D2dContainer, Slot, c32, corner_radii, rect_of};
use crate::geom::{Point, Radius, Rgba};
use crate::list::{
    BorderEdge, BorderKind, BorderPaint, Cmd, EdgePaint, FontDesc, FontKey, Stroke,
    decompose_borders,
};

fn border_kind(style: BorderStyle) -> BorderKind {
    match style {
        BorderStyle::None | BorderStyle::Hidden => BorderKind::None,
        BorderStyle::Solid => BorderKind::Solid,
        BorderStyle::Double => BorderKind::Double,
        BorderStyle::Dashed => BorderKind::Dashed,
        BorderStyle::Dotted => BorderKind::Dotted,
        BorderStyle::Groove => BorderKind::Groove,
        BorderStyle::Ridge => BorderKind::Ridge,
        BorderStyle::Inset => BorderKind::Inset,
        BorderStyle::Outset => BorderKind::Outset,
    }
}

fn border_edge(border: &Border) -> BorderEdge {
    BorderEdge {
        width: border.width,
        color: c32(border.color),
        kind: border_kind(border.style),
    }
}

impl DocumentContainer for D2dContainer {
    fn create_font(&mut self, d: &FontDescription) -> (FontHandle, FontMetrics) {
        let size = d.size().max(1.0);
        let spec = FontSpec::new(d.family(), size)
            .weight(d.weight().clamp(1, 1000) as u16)
            .italic(matches!(d.style(), FontStyle::Italic));
        let font = self.text.font(&spec).unwrap_or_else(|_| {
            self.text
                .font(&FontSpec::new("Segoe UI", size))
                .expect("Segoe UI resolves")
        });
        let m = font.metrics();
        let height = m.line_height();
        let ascent = m.ascent;
        let ch_width = font.width("0");

        let key = self.fonts.len() as FontKey;
        self.fonts.push(FontDesc {
            family: spec.family.clone(),
            size,
            weight: spec.weight,
            italic: spec.italic,
        });
        let handle = self.next_font;
        self.next_font += 1;
        self.slots.borrow_mut().insert(
            handle,
            Slot {
                key,
                font,
                ascent,
                height,
                font_size: size,
                decoration: d.decoration_line(),
                decoration_color: d.decoration_color(),
            },
        );

        let metrics = FontMetrics {
            font_size: size,
            height,
            ascent,
            descent: height - ascent,
            x_height: m.x_height,
            ch_width,
            draw_spaces: true,
            sub_shift: size * 0.3,
            super_shift: size * 0.4,
        };
        (FontHandle(handle), metrics)
    }

    fn delete_font(&mut self, _font: FontHandle) {
        // Slots are keyed into the display list and cheap; keep them.
    }

    fn text_width(&self, text: &str, font: FontHandle) -> f32 {
        let slots = self.slots.borrow();
        let Some(slot) = slots.get(&font.0) else {
            return text.len() as f32 * 8.0;
        };
        let before = slot.font.cached_widths();
        let width = slot.font.width(text);
        let hit = (slot.font.cached_widths() == before) as u64;
        self.measure_calls.set(self.measure_calls.get() + 1);
        self.measure_hits.set(self.measure_hits.get() + hit);
        width
    }

    fn draw_text(
        &mut self,
        _hdc: DrawContext,
        text: &str,
        font: FontHandle,
        color: Color,
        pos: Position,
    ) {
        self.push_text(text, font, color, pos, true);
    }

    fn draw_list_marker(&mut self, _hdc: DrawContext, marker: &ListMarker) {
        let pos = marker.pos();
        let color = c32(marker.color());
        let center = Point::new(pos.x + pos.width / 2.0, pos.y + pos.height / 2.0);
        let radius = pos.width.min(pos.height) / 2.0;
        match marker.marker_type() {
            ListStyleType::None => {}
            ListStyleType::Disc => {
                if radius > 0.0 {
                    self.cmds.push(Cmd::Circle {
                        center,
                        radius,
                        fill: color,
                        stroke: Stroke::solid(0.0, Rgba::TRANSPARENT),
                    });
                }
            }
            ListStyleType::Circle => {
                if radius > 0.0 {
                    self.cmds.push(Cmd::Circle {
                        center,
                        radius,
                        fill: Rgba::TRANSPARENT,
                        stroke: Stroke::solid(1.0, color),
                    });
                }
            }
            ListStyleType::Square => {
                self.cmds.push(Cmd::Rect {
                    rect: rect_of(&pos),
                    radii: [Radius::default(); 4],
                    fill: color,
                });
            }
            _ => self.push_text(
                &format!("{}.", marker.index()),
                marker.font(),
                marker.color(),
                pos,
                false,
            ),
        }
    }

    fn load_image(&mut self, src: &str, _baseurl: &str, redraw_on_ready: bool) {
        if src.is_empty() || self.images.contains_key(src) || self.requested_images.contains(src) {
            return;
        }
        self.requested_images.insert(src.to_string());
        self.pending_images.push((src.to_string(), redraw_on_ready));
    }

    fn get_image_size(&self, src: &str, _baseurl: &str) -> Size {
        let Some(&key) = self.images.get(src) else {
            return Size::default();
        };
        self.image_data
            .get(key as usize)
            .map_or_else(Size::default, |i| Size {
                width: i.width as f32,
                height: i.height as f32,
            })
    }

    fn draw_image(
        &mut self,
        _hdc: DrawContext,
        layer: &BackgroundLayer,
        url: &str,
        _base_url: &str,
    ) {
        self.record_image(layer, url);
    }

    fn draw_solid_fill(&mut self, _hdc: DrawContext, layer: &BackgroundLayer, color: Color) {
        self.fill(layer, color);
    }

    fn draw_linear_gradient(
        &mut self,
        _hdc: DrawContext,
        layer: &BackgroundLayer,
        gradient: &LinearGradient,
    ) {
        self.record_linear_gradient(layer, gradient);
    }

    fn draw_radial_gradient(
        &mut self,
        _hdc: DrawContext,
        layer: &BackgroundLayer,
        gradient: &RadialGradient,
    ) {
        self.record_radial_gradient(layer, gradient);
    }

    fn draw_conic_gradient(
        &mut self,
        _hdc: DrawContext,
        layer: &BackgroundLayer,
        gradient: &ConicGradient,
    ) {
        self.record_conic_gradient(layer, gradient);
    }

    fn draw_borders(&mut self, _hdc: DrawContext, borders: &Borders, pos: Position, _root: bool) {
        let rect = rect_of(&pos);
        let radii = corner_radii(&borders.radius);
        let top = border_edge(&borders.top);
        let right = border_edge(&borders.right);
        let bottom = border_edge(&borders.bottom);
        let left = border_edge(&borders.left);
        match decompose_borders(rect, radii, top, right, bottom, left) {
            BorderPaint::Outline {
                rect,
                radii,
                stroke,
            } => {
                self.cmds.push(Cmd::Outline {
                    rect,
                    radii,
                    stroke,
                });
            }
            BorderPaint::Edges(edges) => {
                for edge in edges {
                    match edge {
                        EdgePaint::Solid { rect, color } => {
                            self.cmds.push(Cmd::Rect {
                                rect,
                                radii: [Radius::default(); 4],
                                fill: color,
                            });
                        }
                        EdgePaint::Line {
                            a,
                            b,
                            width,
                            color,
                            dash,
                        } => {
                            self.cmds.push(Cmd::Line {
                                a,
                                b,
                                stroke: Stroke::solid(width, color),
                                dash,
                            });
                        }
                    }
                }
            }
        }
    }

    fn set_caption(&mut self, _caption: &str) {}

    fn on_anchor_click(&mut self, _url: &str) {}

    fn set_clip(&mut self, pos: Position, radius: BorderRadiuses) {
        self.cmds.push(Cmd::PushClip {
            rect: rect_of(&pos),
            radii: corner_radii(&radius),
        });
    }

    fn del_clip(&mut self) {
        self.cmds.push(Cmd::PopClip);
    }

    fn get_viewport(&self) -> Position {
        self.viewport
    }

    fn get_media_features(&self) -> MediaFeatures {
        MediaFeatures {
            media_type: MediaType::Screen,
            width: self.viewport.width,
            height: self.viewport.height,
            device_width: self.viewport.width,
            device_height: self.viewport.height,
            color: 8,
            color_index: 0,
            monochrome: 0,
            resolution: 96.0,
        }
    }

    fn transform_text(&self, text: &str, tt: TextTransform) -> String {
        match tt {
            TextTransform::Uppercase => text.to_uppercase(),
            TextTransform::Lowercase => text.to_lowercase(),
            TextTransform::Capitalize => {
                let mut result = String::with_capacity(text.len());
                let mut capitalize_next = true;
                for ch in text.chars() {
                    if capitalize_next && ch.is_alphabetic() {
                        result.extend(ch.to_uppercase());
                        capitalize_next = false;
                    } else {
                        result.push(ch);
                        if ch.is_whitespace() {
                            capitalize_next = true;
                        }
                    }
                }
                result
            }
            TextTransform::None => text.to_string(),
        }
    }
}
