//! The recording half of the renderer: a [`DocumentContainer`] that measures
//! text with DirectWrite and turns every draw callback into a [`Cmd`] in
//! document coordinates. (The [`Engine`](crate::engine::Engine) that runs one
//! layout+record pass lives in `engine.rs`.)
//!
//! This is a straight port of `egui-litehtml-webview`'s `painter.rs`, with the
//! egui types replaced by the portable text shaper and the neutral [`Cmd`]
//! from `list.rs`. The invariants are the same: the text engine that measures
//! must be the one that paints, and litehtml's values map identically.
//!
//! The container's `DocumentContainer` implementation lives in `record.rs` and
//! its background/gradient/image recording in `background.rs`; this file owns
//! the type, its fonts and its frame bookkeeping.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::text::{Font, TextSystem};
use litehtml::email::EMAIL_MASTER_CSS;
use litehtml::{BorderRadiuses, Color, FontHandle, Position, TextDecorationLine};

use crate::geom::{Point, Radius, Rect, Rgba};
use crate::list::{Cmd, FontDesc, FontKey, Image, ImageKey};

mod background;
mod record;

/// Height reported to litehtml as the viewport's, so `vh` units are stable.
const VIEWPORT_HEIGHT: f32 = 800.0;

/// litehtml's own UA stylesheet, restated (see `egui-litehtml-webview`'s
/// `painter.rs` for why it is restated rather than left to `from_html`).
const LITEHTML_MASTER_CSS: &str = include_str!("../litehtml_master.css");

/// The user-agent stylesheet: litehtml's defaults, then the email rules, then
/// the `table{text-align:left}` reset. In the master slot so the document's own
/// CSS wins (see `egui-litehtml-webview`).
pub(crate) fn ua_sheet() -> &'static str {
    static SHEET: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SHEET.get_or_init(|| {
        format!("{LITEHTML_MASTER_CSS}\n{EMAIL_MASTER_CSS}\ntable {{ text-align: left; }}\n")
    })
}

fn c32(c: Color) -> Rgba {
    Rgba::with_alpha(c.r, c.g, c.b, c.a)
}

fn rect_of(p: &Position) -> Rect {
    Rect::from_min_size(p.x, p.y, p.width, p.height)
}

/// litehtml's per-corner elliptical radii, in top-left, top-right,
/// bottom-right, bottom-left order (the order the D2D painter expects).
fn corner_radii(r: &BorderRadiuses) -> [Radius; 4] {
    [
        Radius::new(r.top_left_x, r.top_left_y),
        Radius::new(r.top_right_x, r.top_right_y),
        Radius::new(r.bottom_right_x, r.bottom_right_y),
        Radius::new(r.bottom_left_x, r.bottom_left_y),
    ]
}

// ─── The container ──────────────────────────────────────────────────────────

/// One resolved font, kept for measuring and for the slot's metrics.
#[derive(Clone)]
struct Slot {
    key: FontKey,
    font: Font,
    ascent: f32,
    /// Height of a line of this font (ascent + descent + line gap).
    height: f32,
    font_size: f32,
    decoration: TextDecorationLine,
    decoration_color: Color,
}

/// The worker-side `DocumentContainer`.
pub(crate) struct D2dContainer {
    text: TextSystem,
    /// `Rc<RefCell>` so [`D2dContainer::text_measure`] can capture the live
    /// font table without borrowing the container (a `Document` holds its
    /// mutable borrow while text runs are collected).
    slots: Rc<std::cell::RefCell<HashMap<usize, Slot>>>,
    next_font: usize,
    fonts: Vec<FontDesc>,
    viewport: Position,
    cmds: Vec<Cmd>,
    images: HashMap<String, ImageKey>,
    image_data: Vec<Arc<Image>>,
    pending_images: Vec<(String, bool)>,
    requested_images: HashSet<String>,
    /// Diagnostic counters for the render-job log line.
    measure_calls: Cell<u64>,
    measure_hits: Cell<u64>,
}

impl D2dContainer {
    pub(crate) fn new(text: TextSystem) -> Self {
        Self {
            text,
            slots: Rc::new(std::cell::RefCell::new(HashMap::new())),
            next_font: 1,
            fonts: Vec::new(),
            viewport: Position {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: VIEWPORT_HEIGHT,
            },
            cmds: Vec::new(),
            images: HashMap::new(),
            image_data: Vec::new(),
            pending_images: Vec::new(),
            requested_images: HashSet::new(),
            measure_calls: Cell::new(0),
            measure_hits: Cell::new(0),
        }
    }

    pub(crate) fn begin(&mut self, width: f32) {
        self.viewport = Position {
            x: 0.0,
            y: 0.0,
            width,
            height: VIEWPORT_HEIGHT,
        };
        self.cmds.clear();
        self.measure_calls.set(0);
        self.measure_hits.set(0);
    }

    pub(crate) fn load_image_data(&mut self, url: &str, bytes: &[u8]) {
        if self.images.contains_key(url) {
            return;
        }
        let Ok(img) = image::load_from_memory(bytes) else {
            return;
        };
        let rgba = img.to_rgba8();
        let key = self.image_data.len() as ImageKey;
        self.image_data.push(Arc::new(Image {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        }));
        self.images.insert(url.to_string(), key);
    }

    /// Records a text run whose box is `pos`. Shared by `draw_text` and
    /// numbered list markers.
    fn push_text(
        &mut self,
        text: &str,
        font: FontHandle,
        color: Color,
        pos: Position,
        decorate: bool,
    ) {
        let Some(slot) = self.slots.borrow().get(&font.0).cloned() else {
            return;
        };
        let (key, ascent, height, font_size) = (slot.key, slot.ascent, slot.height, slot.font_size);
        let (decoration, decoration_color) = (slot.decoration, slot.decoration_color);
        let color32 = c32(color);
        let lines = if decorate {
            decoration
        } else {
            TextDecorationLine::NONE
        };
        if text.trim().is_empty() && lines == TextDecorationLine::NONE {
            return;
        }
        let origin = Point::new(pos.x, pos.y + pos.height - height);
        if !text.trim().is_empty() {
            self.cmds.push(Cmd::Text {
                origin,
                width: pos.width,
                height,
                text: Arc::from(text),
                font: key,
                color: color32,
            });
        }
        if lines != TextDecorationLine::NONE {
            let col = if decoration_color.a == 0 {
                color32
            } else {
                c32(decoration_color)
            };
            let baseline = origin.y + ascent;
            let thick = (font_size / 14.0).clamp(1.0, 3.0);
            let mut line = |top: f32| {
                self.cmds.push(Cmd::Rect {
                    rect: Rect::from_min_size(pos.x, top, pos.width, thick),
                    radii: [Radius::default(); 4],
                    fill: col,
                });
            };
            if lines.contains(TextDecorationLine::UNDERLINE) {
                line(baseline + font_size * 0.1);
            }
            if lines.contains(TextDecorationLine::LINE_THROUGH) {
                line(baseline - font_size * 0.3 - thick / 2.0);
            }
            if lines.contains(TextDecorationLine::OVERLINE) {
                line(origin.y);
            }
        }
    }

    /// Forget which image URLs were already requested.
    pub(crate) fn clear_pending_images(&mut self) {
        self.pending_images.clear();
        self.requested_images.clear();
    }

    /// Image URLs layout discovered that are not loaded yet.
    pub(crate) fn take_pending_images(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.pending_images)
    }

    /// Diagnostic counters, for the render-job log line.
    pub(crate) fn stats(&self) -> (usize, usize, usize, u64, u64) {
        (
            self.cmds.len(),
            self.fonts.len(),
            self.image_data.len(),
            self.measure_calls.get(),
            self.measure_hits.get(),
        )
    }

    /// Closures for [`TextRunTable::collect`](crate::TextRunTable::collect)
    /// that do not borrow the container: the `Document` holds its mutable
    /// borrow while collection runs. They share the container's live font
    /// table (fonts are created during layout, after this is captured), so the
    /// widths they return match the ones litehtml measured with.
    pub(crate) fn text_measure(
        &self,
    ) -> (
        impl Fn(&str, FontHandle) -> f32 + use<>,
        impl Fn(FontHandle) -> FontKey + use<>,
    ) {
        let widths = Rc::clone(&self.slots);
        let keys = Rc::clone(&self.slots);
        (
            move |text, font| {
                widths
                    .borrow()
                    .get(&font.0)
                    .map_or(text.len() as f32 * 8.0, |s| s.font.width(text))
            },
            move |font| keys.borrow().get(&font.0).map_or(0, |s| s.key),
        )
    }

    /// The recorded commands, fonts and images of the finished pass.
    pub(crate) fn take_frame(&mut self) -> (Vec<Cmd>, Vec<FontDesc>, Vec<Arc<Image>>) {
        let cmds = std::mem::take(&mut self.cmds);
        let fonts = std::mem::take(&mut self.fonts);
        (cmds, fonts, self.image_data.clone())
    }
}
