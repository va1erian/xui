//! The painting half of the renderer: replays a [`DisplayList`] into a portable
//! [`Canvas`], culling to the visible region and applying the scroll offset.
//!
//! The display list is in device-independent pixels; the painter scales it to
//! the canvas's own dpi, so text stays crisp at any scale.

use std::collections::HashMap;

use xui_core::Color;
use xui_core::backend::{
    Canvas, Cap, Corner, Dash as PDash, GradientStop as PGradientStop,
    LinearGradient as PLinearGradient, RadialGradient as PRadialGradient, Rgba as PRgba,
    Stroke as PStroke, TextLayout,
};
use xui_core::geometry::{Point as PxPoint, Rect as PxRect};
use xui_core::image::Image as PImage;

use crate::geom::{Point, Radius, Rect, Rgba};
use crate::list::{Cmd, Dash, DisplayList, FontKey, ImageKey};
use crate::text::{Font, LAYOUT_DPI, TextSystem};

mod text_select;

/// The most shaped text layouts kept between frames.
const MAX_LAYOUTS: usize = 4096;

fn to_rgba(color: Rgba) -> PRgba {
    PRgba::with_alpha(color.r, color.g, color.b, color.a)
}

fn is_rounded(radii: &[Radius; 4]) -> bool {
    radii.iter().any(|r| r.x > 0.0 || r.y > 0.0)
}

/// Maps document coordinates (DIPs) to canvas pixels: a scroll offset and a
/// scale.
#[derive(Clone, Copy)]
struct Space {
    scale: f32,
    scroll: f32,
}

impl Space {
    fn x(self, v: f32) -> i32 {
        (v * self.scale).round() as i32
    }

    fn y(self, v: f32) -> i32 {
        ((v - self.scroll) * self.scale).round() as i32
    }

    fn point(self, p: Point) -> PxPoint {
        PxPoint::new(self.x(p.x), self.y(p.y))
    }

    fn rect(self, r: Rect) -> PxRect {
        PxRect::new(
            self.x(r.left),
            self.y(r.top),
            self.x(r.right),
            self.y(r.bottom),
        )
    }

    fn corners(self, radii: [Radius; 4]) -> [Corner; 4] {
        radii.map(|r| Corner::new(r.x * self.scale, r.y * self.scale))
    }

    fn stroke(self, s: crate::list::Stroke, dash: Dash) -> PStroke {
        let stroke = PStroke::new((s.width * self.scale).max(1.0));
        match dash {
            Dash::Solid => stroke,
            Dash::Dashed => stroke.dash(PDash::Dashed),
            Dash::Dotted => stroke.dash(PDash::Dotted).cap(Cap::Round),
        }
    }
}

fn stops(gradient: &[crate::list::GradientStop]) -> Vec<PGradientStop> {
    gradient
        .iter()
        .map(|s| PGradientStop::new(s.offset, to_rgba(s.color)))
        .collect()
}

/// A painter that caches what a replay needs (resolved fonts, shaped text and
/// decoded images), so painting a frame allocates little per command.
pub struct Painter {
    text: TextSystem,
    fonts: HashMap<FontKey, Font>,
    layouts: HashMap<(FontKey, u32, String), Box<dyn TextLayout>>,
    images: HashMap<ImageKey, PImage>,
}

impl Painter {
    /// Creates a painter over `text`, shared with the worker that laid the
    /// document out (so painted widths match measured widths).
    pub fn new(text: TextSystem) -> Painter {
        Painter {
            text,
            fonts: HashMap::new(),
            layouts: HashMap::new(),
            images: HashMap::new(),
        }
    }

    /// Replays `list` with its top-left scrolled to `scroll` device-independent
    /// pixels above the viewport's top. `viewport` is in DIPs. Only what
    /// intersects the viewport is drawn; a tall newsletter costs only what is
    /// on screen. `background` shows wherever the document paints nothing of
    /// its own.
    pub fn paint(
        &mut self,
        list: &DisplayList,
        canvas: &mut dyn Canvas,
        viewport: Rect,
        scroll: f32,
        background: Color,
    ) {
        let t = std::time::Instant::now();
        let space = Space {
            scale: canvas.dpi() as f32 / LAYOUT_DPI as f32,
            scroll,
        };
        canvas.clear(background);
        let bounds = canvas.bounds();
        canvas.push_clip(bounds);

        // `viewport` translated into document space: the commands' own
        // coordinates.
        let visible = viewport.translate(0.0, scroll);
        let mut clips: Vec<Rect> = Vec::new();
        let mut drawn = 0usize;

        for cmd in &list.cmds {
            match cmd {
                Cmd::PushClip { rect, radii } => {
                    clips.push(*rect);
                    if is_rounded(radii) {
                        canvas.push_clip_rounded(space.rect(*rect), space.corners(*radii));
                    } else {
                        canvas.push_clip(space.rect(*rect));
                    }
                    continue;
                }
                Cmd::PopClip => {
                    clips.pop();
                    canvas.pop_clip();
                    continue;
                }
                _ => {}
            }
            if let Some(bounds) = cmd.bounds() {
                let narrowed = clips
                    .iter()
                    .fold(visible, |a, c| a.intersect(*c).unwrap_or(Rect::default()));
                if !narrowed.intersects(bounds) {
                    continue;
                }
            }
            drawn += 1;
            self.draw(canvas, space, list, cmd);
        }
        canvas.pop_clip();
        log::debug!(
            "paint: replayed {drawn}/{} cmds in {:?}",
            list.cmds.len(),
            t.elapsed()
        );
    }

    fn draw(&mut self, canvas: &mut dyn Canvas, space: Space, list: &DisplayList, cmd: &Cmd) {
        match cmd {
            Cmd::Rect { rect, radii, fill } => {
                if is_rounded(radii) {
                    canvas.fill_rounded_rect_corners(
                        space.rect(*rect),
                        space.corners(*radii),
                        to_rgba(*fill),
                    );
                } else {
                    canvas.fill_rect_rgba(space.rect(*rect), to_rgba(*fill));
                }
            }
            Cmd::Outline {
                rect,
                radii,
                stroke,
            } => {
                canvas.stroke_rounded_rect_corners(
                    space.rect(*rect),
                    space.corners(*radii),
                    to_rgba(stroke.color),
                    &space.stroke(*stroke, Dash::Solid),
                );
            }
            Cmd::Line { a, b, stroke, dash } => {
                canvas.draw_line_stroked(
                    space.point(*a),
                    space.point(*b),
                    to_rgba(stroke.color),
                    &space.stroke(*stroke, *dash),
                );
            }
            Cmd::Circle {
                center,
                radius,
                fill,
                stroke,
            } => {
                let r = radius * space.scale;
                if fill.a > 0 {
                    // The portable ellipse fill is opaque; a fully rounded
                    // rectangle carries the alpha.
                    let c = space.point(*center);
                    let extent = r.round() as i32;
                    let bounds =
                        PxRect::new(c.x - extent, c.y - extent, c.x + extent, c.y + extent);
                    canvas.fill_rounded_rect_corners(
                        bounds,
                        [Corner::uniform(r); 4],
                        to_rgba(*fill),
                    );
                }
                if stroke.width > 0.0 {
                    canvas.stroke_ellipse_stroked(
                        space.point(*center),
                        r,
                        r,
                        to_rgba(stroke.color),
                        &space.stroke(*stroke, Dash::Solid),
                    );
                }
            }
            Cmd::Text {
                origin,
                text,
                font,
                color,
                ..
            } => {
                if color.a == 0 {
                    return;
                }
                let dpi = canvas.dpi();
                let Some(layout) = self.layout(list, *font, text, dpi) else {
                    return;
                };
                canvas.draw_layout(layout, space.point(*origin), to_rgba(*color));
            }
            Cmd::Image { image, rect } => {
                let Some(image) = self.image(list, *image) else {
                    return;
                };
                canvas.draw_image(image, space.rect(*rect));
            }
            Cmd::LinearGradient { rect, gradient } => {
                let grad = PLinearGradient::new(
                    space.point(gradient.start),
                    space.point(gradient.end),
                    stops(&gradient.stops),
                );
                canvas.fill_rect_linear(space.rect(*rect), &grad);
            }
            Cmd::RadialGradient { rect, gradient } => {
                let grad = PRadialGradient::new(
                    space.point(gradient.center),
                    gradient.radius_x * space.scale,
                    gradient.radius_y * space.scale,
                    stops(&gradient.stops),
                );
                canvas.fill_rect_radial(space.rect(*rect), &grad);
            }
            Cmd::PushClip { .. } | Cmd::PopClip => unreachable!("handled in the replay loop"),
        }
    }

    fn font(&mut self, list: &DisplayList, key: FontKey) -> Option<Font> {
        if let Some(font) = self.fonts.get(&key) {
            return Some(font.clone());
        }
        let desc = list.fonts.get(key as usize)?;
        let font = self
            .text
            .font(&desc.family, desc.size, desc.weight, desc.italic);
        self.fonts.insert(key, font.clone());
        Some(font)
    }

    /// The shaped layout of `text` in font `key` at `dpi`, cached across frames.
    fn layout(
        &mut self,
        list: &DisplayList,
        key: FontKey,
        text: &str,
        dpi: u32,
    ) -> Option<&dyn TextLayout> {
        let cache_key = (key, dpi, text.to_string());
        if !self.layouts.contains_key(&cache_key) {
            let font = self.font(list, key)?;
            if self.layouts.len() >= MAX_LAYOUTS {
                self.layouts.clear();
            }
            let layout = self.text.shape(text, &font.spec, dpi);
            self.layouts.insert(cache_key.clone(), layout);
        }
        self.layouts.get(&cache_key).map(|l| &**l)
    }

    fn image(&mut self, list: &DisplayList, key: ImageKey) -> Option<&PImage> {
        if let std::collections::hash_map::Entry::Vacant(slot) = self.images.entry(key) {
            let image = list.images.get(key as usize)?;
            let decoded = PImage::from_rgba(image.width, image.height, image.rgba.clone()).ok()?;
            slot.insert(decoded);
        }
        self.images.get(&key)
    }
}
