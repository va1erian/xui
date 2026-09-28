//! Text measurement over the portable [`TextShaper`], laid out at 96 dpi so a
//! measured width is in device-independent pixels, the unit the document is
//! laid out in.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use xui_core::Dip;
use xui_core::backend::{FontSpec, TextLayout, TextShaper};

/// The dpi at which text is shaped for layout: one device pixel per DIP.
pub(crate) const LAYOUT_DPI: u32 = 96;

/// A cloneable, `Send` handle to a backend's text shaper.
#[derive(Clone)]
pub struct TextSystem {
    shaper: Arc<dyn TextShaper>,
}

impl TextSystem {
    /// Wraps `shaper`, as obtained from `Ui::text_shaper`.
    pub fn new(shaper: Box<dyn TextShaper>) -> TextSystem {
        TextSystem {
            shaper: Arc::from(shaper),
        }
    }

    /// A text system over the Win32 backend's shaper, for tests.
    #[cfg(test)]
    pub(crate) fn for_tests() -> TextSystem {
        use xui_core::backend::Backend;
        TextSystem::new(xui_win32::Win32Backend::new().text_shaper())
    }

    /// A font for `family` at `size` DIPs.
    pub(crate) fn font(&self, family: &str, size: f32, weight: u16, italic: bool) -> Font {
        let mut spec = FontSpec::new(Dip(size)).weight(weight).italic(italic);
        if !family.is_empty() {
            spec = spec.family(family);
        }
        Font {
            shaper: Arc::clone(&self.shaper),
            spec,
            widths: Rc::default(),
        }
    }

    /// Shapes `text` at `dpi`, unwrapped, for drawing at a device scale.
    pub(crate) fn shape(&self, text: &str, spec: &FontSpec, dpi: u32) -> Box<dyn TextLayout> {
        self.shaper.layout(text, spec, f32::INFINITY, dpi)
    }
}

/// Font metrics in DIPs.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    pub(crate) ascent: f32,
    pub(crate) x_height: f32,
    height: f32,
}

impl Metrics {
    /// The height of one line.
    pub(crate) fn line_height(&self) -> f32 {
        self.height
    }
}

/// A resolved font at the layout scale, with a per-font width cache.
#[derive(Clone)]
pub(crate) struct Font {
    shaper: Arc<dyn TextShaper>,
    pub(crate) spec: FontSpec,
    widths: Rc<RefCell<HashMap<String, f32>>>,
}

impl Font {
    /// The unwrapped layout of `text`.
    pub(crate) fn layout(&self, text: &str) -> Box<dyn TextLayout> {
        self.shaper
            .layout(text, &self.spec, f32::INFINITY, LAYOUT_DPI)
    }

    /// The advance width of `text`, cached.
    pub(crate) fn width(&self, text: &str) -> f32 {
        if let Some(width) = self.widths.borrow().get(text) {
            return *width;
        }
        let width = self.layout(text).width();
        let mut widths = self.widths.borrow_mut();
        if widths.len() > 4096 {
            widths.clear();
        }
        widths.insert(text.to_string(), width);
        width
    }

    /// How many distinct widths are cached, for the hit-rate log line.
    pub(crate) fn cached_widths(&self) -> usize {
        self.widths.borrow().len()
    }

    /// The font's metrics. The portable shaper reports a line's height; the
    /// ascent and x-height are proportional estimates of it.
    pub(crate) fn metrics(&self) -> Metrics {
        let height = self.layout("Hg").height();
        Metrics {
            ascent: height * 0.8,
            x_height: self.spec.size.value() * 0.5,
            height,
        }
    }
}
