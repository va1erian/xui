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
use crate::selection::{Selection, TextPos};
use crate::text::{Font, LAYOUT_DPI, TextSystem};
use crate::text_runs::{TextRun, TextRunTable};

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

    /// Resolves the font for `key`, for the widget's hit-testing and selection
    /// boxes (which rebuild a the shaper layout per run).
    pub(crate) fn resolve_font(&mut self, list: &DisplayList, key: FontKey) -> Option<Font> {
        self.font(list, key)
    }

    /// The caret nearest `doc`, using the shaper hit-testing for the
    /// character boundary (accurate for right-to-left and complex text, where
    /// the run table's left-to-right `offsets` are not).
    pub fn caret_at(
        &mut self,
        list: &DisplayList,
        runs: &TextRunTable,
        doc: Point,
    ) -> Option<TextPos> {
        let run = runs.nearest_run(doc)?;
        let text_run = &runs.runs[run];
        let font = self.resolve_font(list, text_run.font)?;
        let layout = font.layout(&text_run.text);
        let hit = layout.hit_test_point(doc.x - text_run.rect.left, 0.0);
        let byte = hit.byte_index.min(text_run.text.len());
        let ch = text_run.text[..byte].chars().count();
        Some(TextPos { run, ch })
    }

    /// The highlight boxes of `sel`, in document coordinates, from the shaper's
    /// per-run selection rects, merged across words the way the run table's own
    /// offsets-based selection does (so a whole line highlights as one box).
    pub fn selection_rects(
        &mut self,
        list: &DisplayList,
        runs: &TextRunTable,
        sel: &Selection,
    ) -> Vec<Rect> {
        let (start, end) = sel.ordered();
        let mut out: Vec<Rect> = Vec::new();
        let Some(last_run) = runs.runs.len().checked_sub(1) else {
            return out;
        };
        for i in start.run..=end.run.min(last_run) {
            let run = &runs.runs[i];
            let from = if i == start.run { start.ch } else { 0 };
            let to = (if i == end.run {
                end.ch
            } else {
                run.char_count()
            })
            .min(run.char_count());
            if from >= to {
                continue;
            }
            let rect = run_rect(self, list, run, from, to).unwrap_or_else(|| Rect {
                left: run.rect.left + run.offsets[from],
                top: run.rect.top,
                right: run.rect.left + run.offsets[to],
                bottom: run.rect.bottom,
            });
            let extends_last = out.last().is_some_and(|last| {
                (last.center().y - rect.center().y).abs() < rect.height() * 0.5
                    && (rect.left - last.right).abs() < 2.0
            });
            if extends_last {
                let last = out.last_mut().unwrap();
                *last = last.union(rect);
            } else if !run.text.trim().is_empty() {
                out.push(rect);
            }
        }
        out
    }
}

/// The horizontal highlight extent of `run[from..to]`, from the shaper's
/// selection rects (the vertical is the run's own box, matching the highlight
/// of the egui widget). Falls back to `None` when the font cannot be resolved.
fn run_rect(
    painter: &mut Painter,
    list: &DisplayList,
    run: &TextRun,
    from: usize,
    to: usize,
) -> Option<Rect> {
    let font = painter.resolve_font(list, run.font)?;
    let layout = font.layout(&run.text);
    let from_byte = char_to_byte(&run.text, from);
    let to_byte = char_to_byte(&run.text, to);
    let boxes = layout.selection_rects(from_byte, to_byte);
    if boxes.is_empty() {
        return None;
    }
    let left = boxes
        .iter()
        .map(|b| b.left as f32)
        .fold(f32::INFINITY, f32::min);
    let right = boxes
        .iter()
        .map(|b| b.right as f32)
        .fold(f32::NEG_INFINITY, f32::max);
    Some(Rect {
        left: run.rect.left + left,
        top: run.rect.top,
        right: run.rect.left + right,
        bottom: run.rect.bottom,
    })
}

/// The byte offset of the `ch`-th character (clamped to the text's end).
fn char_to_byte(text: &str, ch: usize) -> usize {
    text.char_indices().nth(ch).map_or(text.len(), |(i, _)| i)
}
