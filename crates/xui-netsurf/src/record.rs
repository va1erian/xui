#![forbid(unsafe_code)]

//! Turns one NetSurf redraw into an `xui-litehtml` [`DisplayList`], so the
//! page is painted by the same [`Painter`](xui_litehtml::Painter) as a
//! litehtml page.
//!
//! The painter caches resolved fonts and decoded images by key across frames,
//! so keys must keep their meaning: a [`Registry`] lives for one page and only
//! ever appends. The view drops its painter when the page changes (see
//! `Frame::epoch`).

use std::collections::HashMap;
use std::sync::Arc;

use xui_litehtml::{
    Cmd, Dash, DisplayList, FontDesc, FontKey, Image, ImageKey, Point, Radius, Rect, Rgba, Stroke,
};

use crate::fonts::{FontReq, Fonts};

/// How a stroke or fill is drawn (`NSX_PLOT_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlotKind {
    None,
    Solid,
    Dot,
    Dash,
}

impl PlotKind {
    pub(crate) fn from_raw(raw: i32) -> PlotKind {
        match raw {
            1 => PlotKind::Solid,
            2 => PlotKind::Dot,
            3 => PlotKind::Dash,
            _ => PlotKind::None,
        }
    }
}

/// A NetSurf plot style with colours already straight RGBA.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Style {
    pub(crate) fill_kind: PlotKind,
    pub(crate) fill: Rgba,
    pub(crate) stroke_kind: PlotKind,
    pub(crate) stroke: Rgba,
    pub(crate) stroke_width: f32,
}

/// A straight `0xAARRGGBB` colour.
pub(crate) fn argb(c: u32) -> Rgba {
    Rgba::with_alpha((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
}

/// Pixels NetSurf handed over for one bitmap: its identity, version and RGBA
/// rows.
pub(crate) struct BitmapPixels<'a> {
    pub(crate) id: usize,
    pub(crate) generation: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: usize,
    pub(crate) rgba: &'a [u8],
}

/// The fonts and images one page's frames refer to, by stable key.
#[derive(Default)]
pub(crate) struct Registry {
    fonts: Vec<FontDesc>,
    font_keys: HashMap<(String, u32, u16, bool), FontKey>,
    images: Vec<Arc<Image>>,
    /// Per bitmap: the generation its slot holds, and the slot's key.
    image_keys: HashMap<usize, (u32, ImageKey)>,
}

impl Registry {
    fn font(&mut self, req: &FontReq) -> FontKey {
        let key = (
            req.family.clone(),
            req.size.to_bits(),
            req.weight,
            req.italic,
        );
        *self.font_keys.entry(key).or_insert_with(|| {
            self.fonts.push(FontDesc {
                family: req.family.clone(),
                size: req.size,
                weight: req.weight,
                italic: req.italic,
            });
            (self.fonts.len() - 1) as FontKey
        })
    }

    /// The key of `px`'s pixels. A bitmap holds one slot: a new generation
    /// (an animation frame) takes a fresh key and empties the old slot, which
    /// tells the painter to drop what it decoded from it.
    fn image(&mut self, px: &BitmapPixels<'_>) -> ImageKey {
        let old = self.image_keys.get(&px.id).copied();
        if let Some((generation, k)) = old
            && generation == px.generation
        {
            return k;
        }
        let row = px.width as usize * 4;
        let mut rgba = Vec::with_capacity(row * px.height as usize);
        for y in 0..px.height as usize {
            let start = y * px.stride;
            rgba.extend_from_slice(&px.rgba[start..start + row]);
        }
        self.images.push(Arc::new(Image {
            width: px.width,
            height: px.height,
            rgba,
        }));
        let k = (self.images.len() - 1) as ImageKey;
        self.image_keys.insert(px.id, (px.generation, k));
        if let Some((_, stale)) = old {
            self.images[stale as usize] = Arc::new(Image {
                width: 0,
                height: 0,
                rgba: Vec::new(),
            });
        }
        k
    }
}

/// Collects one redraw's plot calls.
pub(crate) struct Recorder<'a> {
    fonts: &'a Fonts,
    registry: &'a mut Registry,
    cmds: Vec<Cmd>,
    clipped: bool,
}

impl<'a> Recorder<'a> {
    pub(crate) fn new(fonts: &'a Fonts, registry: &'a mut Registry) -> Recorder<'a> {
        Recorder {
            fonts,
            registry,
            cmds: Vec::new(),
            clipped: false,
        }
    }

    /// The finished list for a document of `size` CSS pixels.
    pub(crate) fn finish(mut self, size: (f32, f32)) -> DisplayList {
        if self.clipped {
            self.cmds.push(Cmd::PopClip);
        }
        DisplayList {
            cmds: self.cmds,
            size,
            fonts: self.registry.fonts.clone(),
            images: self.registry.images.clone(),
        }
    }

    /// NetSurf's clips replace each other rather than nest.
    pub(crate) fn clip(&mut self, r: Rect) {
        if self.clipped {
            self.cmds.push(Cmd::PopClip);
        }
        self.cmds.push(Cmd::PushClip {
            rect: r,
            radii: [Radius::default(); 4],
        });
        self.clipped = true;
    }

    pub(crate) fn rect(&mut self, r: Rect, s: &Style) {
        if s.fill_kind != PlotKind::None && s.fill.a > 0 {
            self.cmds.push(Cmd::Rect {
                rect: r,
                radii: [Radius::default(); 4],
                fill: s.fill,
            });
        }
        if s.stroke_kind != PlotKind::None && s.stroke.a > 0 {
            let corners = [
                (r.left, r.top, r.right, r.top),
                (r.right, r.top, r.right, r.bottom),
                (r.right, r.bottom, r.left, r.bottom),
                (r.left, r.bottom, r.left, r.top),
            ];
            for (x0, y0, x1, y1) in corners {
                self.line(Point::new(x0, y0), Point::new(x1, y1), s);
            }
        }
    }

    pub(crate) fn line(&mut self, a: Point, b: Point, s: &Style) {
        let dash = match s.stroke_kind {
            PlotKind::None => return,
            PlotKind::Solid => Dash::Solid,
            PlotKind::Dot => Dash::Dotted,
            PlotKind::Dash => Dash::Dashed,
        };
        self.cmds.push(Cmd::Line {
            a,
            b,
            stroke: Stroke::solid(s.stroke_width.max(1.0), s.stroke),
            dash,
        });
    }

    pub(crate) fn disc(&mut self, center: Point, radius: f32, s: &Style) {
        let fill = if s.fill_kind == PlotKind::None {
            Rgba::TRANSPARENT
        } else {
            s.fill
        };
        let width = if s.stroke_kind == PlotKind::None {
            0.0
        } else {
            s.stroke_width.max(1.0)
        };
        self.cmds.push(Cmd::Circle {
            center,
            radius,
            fill,
            stroke: Stroke::solid(width, s.stroke),
        });
    }

    pub(crate) fn polygon(&mut self, points: Vec<Point>, s: &Style) {
        if s.fill_kind != PlotKind::None && points.len() >= 3 {
            self.cmds.push(Cmd::Polygon {
                points,
                fill: s.fill,
            });
        }
    }

    /// A bitmap scaled into `dest`, tiled across `area` when it repeats.
    pub(crate) fn bitmap(&mut self, px: &BitmapPixels<'_>, dest: Rect, repeat: (bool, bool)) {
        if px.width == 0 || px.height == 0 || dest.width() <= 0.0 || dest.height() <= 0.0 {
            return;
        }
        let image = self.registry.image(px);
        if !repeat.0 && !repeat.1 {
            self.cmds.push(Cmd::Image { image, rect: dest });
            return;
        }
        // NetSurf clips a repeating background to its box first, so tiling
        // over the current clip is enough; it is bounded by `MAX_TILES`.
        const MAX_TILES: usize = 4096;
        let (w, h) = (dest.width(), dest.height());
        let clip = self.current_clip().unwrap_or(dest);
        let xs = tile_starts(dest.left, w, clip.left, clip.right, repeat.0);
        let ys = tile_starts(dest.top, h, clip.top, clip.bottom, repeat.1);
        for y in ys.iter().take(MAX_TILES) {
            for x in xs.iter().take(MAX_TILES / ys.len().max(1)) {
                self.cmds.push(Cmd::Image {
                    image,
                    rect: Rect::from_min_size(*x, *y, w, h),
                });
            }
        }
    }

    fn current_clip(&self) -> Option<Rect> {
        self.cmds.iter().rev().find_map(|c| match c {
            Cmd::PushClip { rect, .. } => Some(*rect),
            _ => None,
        })
    }

    /// A text run whose baseline is at `y`.
    pub(crate) fn text(&mut self, font: &FontReq, x: f32, y: f32, text: &str, color: Rgba) {
        if text.is_empty() {
            return;
        }
        let key = self.registry.font(font);
        let width = self.fonts.width(font, text) as f32;
        let ascent = self.fonts.ascent(font);
        self.cmds.push(Cmd::Text {
            origin: Point::new(x, y - ascent),
            width,
            height: self.fonts.line_height(font),
            text: Arc::from(text),
            font: key,
            color,
        });
    }
}

/// The starts of the tiles of size `size` placed at `origin` that cover
/// `lo..hi`, or just `origin` when the axis does not repeat.
fn tile_starts(origin: f32, size: f32, lo: f32, hi: f32, repeat: bool) -> Vec<f32> {
    if !repeat || size <= 0.0 {
        return vec![origin];
    }
    let first = origin - ((origin - lo) / size).ceil().max(0.0) * size;
    let mut starts = Vec::new();
    let mut at = first;
    while at < hi {
        starts.push(at);
        at += size;
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_unpack_as_argb() {
        assert_eq!(argb(0x80FF0000), Rgba::with_alpha(0xFF, 0, 0, 0x80));
        assert_eq!(argb(0xFF00FF00), Rgba::rgb(0, 0xFF, 0));
    }

    #[test]
    fn tiles_cover_the_clip_from_before_the_origin() {
        assert_eq!(
            tile_starts(10.0, 20.0, 0.0, 45.0, true),
            vec![-10.0, 10.0, 30.0]
        );
        assert_eq!(tile_starts(10.0, 20.0, 0.0, 45.0, false), vec![10.0]);
    }

    #[test]
    fn a_new_bitmap_generation_replaces_its_slot() {
        let rgba = [0u8; 16];
        let px = |generation| BitmapPixels {
            id: 7,
            generation,
            width: 2,
            height: 2,
            stride: 8,
            rgba: &rgba,
        };
        let mut registry = Registry::default();
        let first = registry.image(&px(1));
        assert_eq!(registry.image(&px(1)), first);
        let second = registry.image(&px(2));
        assert_ne!(second, first);
        // The old frame's pixels are released; only the live one is held.
        assert_eq!(registry.images[first as usize].width, 0);
        assert_eq!(registry.images[second as usize].rgba.len(), 16);
    }
}
