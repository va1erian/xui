//! Background recording for the container: solid fills, gradients and tiled
//! images. These are the `DocumentContainer` draw callbacks that all map onto
//! the neutral [`Cmd`] display list, split out of `mod.rs` so each file stays
//! small. Behaviour is unchanged.

use litehtml::{
    BackgroundLayer, BackgroundRepeat, Color, ColorPoint, ConicGradient, LinearGradient, Position,
    RadialGradient,
};

use super::{D2dContainer, c32, corner_radii, rect_of};
use crate::geom::{Point, Radius, Rect};
use crate::list::{Cmd, normalize_stops};

/// Most tiles of a repeated background image drawn along one axis / in total.
const MAX_TILES_PER_AXIS: usize = 2048;
const MAX_TILES: usize = 20_000;

impl D2dContainer {
    pub(super) fn fill(&mut self, layer: &BackgroundLayer, color: Color) {
        if color.a == 0 {
            return;
        }
        let b = layer.border_box();
        if b.width <= 0.0 || b.height <= 0.0 {
            return;
        }
        self.cmds.push(Cmd::Rect {
            rect: rect_of(&b),
            radii: corner_radii(&layer.border_radius()),
            fill: c32(color),
        });
    }

    fn gradient_stops(points: &[ColorPoint]) -> Vec<crate::list::GradientStop> {
        normalize_stops(
            &points
                .iter()
                .map(|p| (p.offset, c32(p.color)))
                .collect::<Vec<_>>(),
        )
    }

    pub(super) fn record_image(&mut self, layer: &BackgroundLayer, url: &str) {
        let Some(&key) = self.images.get(url) else {
            return;
        };
        let Some(img) = self.image_data.get(key as usize) else {
            return;
        };
        if img.width == 0 || img.height == 0 {
            return;
        }
        // `origin_box` is one tile *after* width/height attributes,
        // `background-size` and `background-position` are applied.
        let origin = layer.origin_box();
        let (tile_w, tile_h) = if origin.width > 0.0 && origin.height > 0.0 {
            (origin.width, origin.height)
        } else {
            (img.width as f32, img.height as f32)
        };
        let clip_box = layer.clip_box();
        let clip = if clip_box.width > 0.0 && clip_box.height > 0.0 {
            clip_box
        } else {
            Position {
                x: origin.x,
                y: origin.y,
                width: tile_w,
                height: tile_h,
            }
        };

        let repeat = layer.repeat();
        let repeat_x = matches!(repeat, BackgroundRepeat::Repeat | BackgroundRepeat::RepeatX);
        let repeat_y = matches!(repeat, BackgroundRepeat::Repeat | BackgroundRepeat::RepeatY);
        let xs = tile_starts(origin.x, tile_w, clip.x, clip.x + clip.width, repeat_x);
        let ys = tile_starts(origin.y, tile_h, clip.y, clip.y + clip.height, repeat_y);

        // Clip only when some tile actually spills over the clip box.
        let eps = 0.5;
        let spills = xs.first().is_some_and(|&x| x < clip.x - eps)
            || xs
                .last()
                .is_some_and(|&x| x + tile_w > clip.x + clip.width + eps)
            || ys.first().is_some_and(|&y| y < clip.y - eps)
            || ys
                .last()
                .is_some_and(|&y| y + tile_h > clip.y + clip.height + eps);
        if spills {
            self.cmds.push(Cmd::PushClip {
                rect: rect_of(&clip),
                radii: [Radius::default(); 4],
            });
        }
        let mut count = 0;
        'rows: for &y in &ys {
            for &x in &xs {
                if count >= MAX_TILES {
                    break 'rows;
                }
                count += 1;
                let rect = Rect::from_min_size(x, y, tile_w, tile_h);
                self.cmds.push(Cmd::Image { image: key, rect });
            }
        }
        if spills {
            self.cmds.push(Cmd::PopClip);
        }
    }

    pub(super) fn record_linear_gradient(
        &mut self,
        layer: &BackgroundLayer,
        gradient: &LinearGradient,
    ) {
        let points = gradient.color_points();
        let stops = Self::gradient_stops(&points);
        if stops.len() < 2 {
            if let Some(p) = points.first() {
                self.fill(layer, p.color);
            }
            return;
        }
        let b = layer.border_box();
        if b.width <= 0.0 || b.height <= 0.0 {
            return;
        }
        // litehtml's gradient line is in document coordinates already.
        let (s, e) = (gradient.start(), gradient.end());
        let start = Point::new(s.x, s.y);
        let d = Point::new(e.x - s.x, e.y - s.y);
        if d.x * d.x + d.y * d.y < 1e-6 {
            if let Some(p) = points.last() {
                self.fill(layer, p.color);
            }
            return;
        }
        self.cmds.push(Cmd::LinearGradient {
            rect: rect_of(&b),
            gradient: crate::list::LinearGradient {
                start,
                end: Point::new(e.x, e.y),
                stops,
            },
        });
    }

    pub(super) fn record_radial_gradient(
        &mut self,
        layer: &BackgroundLayer,
        gradient: &RadialGradient,
    ) {
        let points = gradient.color_points();
        let stops = Self::gradient_stops(&points);
        if stops.len() < 2 {
            if let Some(p) = points.first() {
                self.fill(layer, p.color);
            }
            return;
        }
        let b = layer.border_box();
        if b.width <= 0.0 || b.height <= 0.0 {
            return;
        }
        let c = gradient.position();
        let r = gradient.radius();
        self.cmds.push(Cmd::RadialGradient {
            rect: rect_of(&b),
            gradient: crate::list::RadialGradient {
                center: Point::new(c.x, c.y),
                radius_x: r.x.max(0.001),
                radius_y: r.y.max(0.001),
                stops,
            },
        });
    }

    pub(super) fn record_conic_gradient(
        &mut self,
        layer: &BackgroundLayer,
        gradient: &ConicGradient,
    ) {
        // Unsupported: fall back to the first colour.
        if let Some(p) = gradient.color_points().first() {
            self.fill(layer, p.color);
        }
    }
}

/// Origins along one axis of every tile of a background of `size` starting at
/// `origin` that could be visible in `[clip_start, clip_end)`.
fn tile_starts(origin: f32, size: f32, clip_start: f32, clip_end: f32, repeat: bool) -> Vec<f32> {
    if !repeat || size <= 0.0 {
        return vec![origin];
    }
    let k = ((origin - clip_start) / size).ceil();
    let mut x = origin - k * size;
    let mut out = Vec::new();
    while x < clip_end && out.len() < MAX_TILES_PER_AXIS {
        out.push(x);
        x += size;
    }
    out
}
