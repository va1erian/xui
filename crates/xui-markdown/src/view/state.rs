#![forbid(unsafe_code)]

//! The view's state: the layout, the scroll position and the scrollbar.
//!
//! The scroll position is kept as an *anchor*, a block and an offset into
//! it. When blocks above the view are laid out and their estimated heights
//! turn out wrong, the anchor's block stays where it was on screen instead of
//! the text jumping.

use xui_core::geometry::{Point, Rect};
use xui_core::widget::scrollbar::{self, Orientation, Scroll, THICKNESS, TrackHit};

use crate::layout::Layout;

/// One wheel notch, in design units.
pub(crate) const WHEEL_DIP: f32 = 48.0;

/// The most times one [`State::settle`] re-clamps.
const SETTLE_PASSES: usize = 16;

pub(crate) struct State {
    pub layout: Layout,
    /// The scroll offset in device pixels from the top of the content.
    pub scroll: f32,
    /// The block the view is anchored to, and the scroll offset less that
    /// block's top: `(0, 0.0)` is the very top, padding included.
    anchor: (usize, f32),
    /// Whether the view stays on the end of the content (after End) while
    /// the blocks there are laid out and the end moves.
    pinned_end: bool,
    pub bounds: Rect,
    pub track: Rect,
    pub dpi: u32,
    /// The document padding in device pixels, as of the last prepare.
    pub pad: f32,
    pub padding: xui_core::Dip,
    /// A thumb drag: the pointer's y and the offset when it began.
    pub bar_drag: Option<(i32, i32)>,
    /// The link under the pointer.
    pub hover: Option<u32>,
    /// The link pressed, followed if released over.
    pub pressed: Option<u32>,
    pub focused: bool,
}

impl State {
    pub fn new(layout: Layout, padding: xui_core::Dip, dpi: u32) -> State {
        State {
            layout,
            scroll: 0.0,
            anchor: (0, 0.0),
            pinned_end: false,
            bounds: Rect::default(),
            track: Rect::default(),
            dpi,
            pad: 0.0,
            padding,
            bar_drag: None,
            hover: None,
            pressed: None,
            focused: false,
        }
    }

    /// Lays out what `bounds` shows at `dpi` and settles the scroll position.
    pub fn prepare(&mut self, bounds: Rect, dpi: u32) {
        self.fit(bounds, dpi);
        self.settle();
    }

    /// Sizes the scrollbar, the padding and the layout for `bounds` at `dpi`.
    pub fn fit(&mut self, bounds: Rect, dpi: u32) {
        self.bounds = bounds;
        if dpi != self.dpi {
            // The anchor's offset is in device pixels: keep the same text in
            // view at the new scale.
            self.anchor.1 *= dpi as f32 / self.dpi as f32;
            self.dpi = dpi;
        }
        let bar = THICKNESS.to_px(dpi).value().min(bounds.width());
        self.track = Rect::new(bounds.right - bar, bounds.top, bounds.right, bounds.bottom);
        self.pad = (self.padding.0 * dpi as f32 / 96.0).round();
        let width = (bounds.width() - bar) as f32 - 2.0 * self.pad;
        // A view squeezed to nothing keeps its last layout rather than laying
        // the document out a pixel wide.
        if width > 0.0 {
            self.layout.resize(width, dpi);
        }
    }

    /// Lays out the visible blocks and puts the anchor back where it was,
    /// clamped to the content.
    ///
    /// Laying out what comes into view corrects estimates, which can move the
    /// end of the content (a jump to the end lands by estimates first), so
    /// this repeats until the position is clamped and its blocks laid out.
    /// Every repeat lays out more blocks, so it ends; the bound only caps the
    /// work one paint does.
    pub fn settle(&mut self) {
        if !self.layout.sized() {
            return;
        }
        let viewport = self.viewport();
        for _ in 0..SETTLE_PASSES {
            if self.pinned_end {
                let end = self.max_scroll();
                self.anchor_to(end);
            }
            let (block, offset) = self.anchor;
            self.scroll = self.layout.top(block) + offset;
            let y0 = self.scroll - self.pad;
            self.layout.ensure(y0, y0 + viewport);
            let at = self.layout.top(block) + offset;
            let max = self.max_scroll();
            let target = if self.pinned_end {
                max
            } else {
                at.clamp(0.0, max)
            };
            if target == at && at == self.scroll {
                return;
            }
            self.anchor_to(target);
        }
    }

    pub fn viewport(&self) -> f32 {
        self.bounds.height().max(0) as f32
    }

    /// The content's full height, padding included.
    pub fn content_height(&mut self) -> f32 {
        self.layout.height() + 2.0 * self.pad
    }

    pub fn max_scroll(&mut self) -> f32 {
        (self.content_height() - self.viewport()).max(0.0)
    }

    /// Scrolls to `offset`, clamped, and re-anchors there.
    pub fn set_scroll(&mut self, offset: f32) {
        self.pinned_end = false;
        self.anchor_to(offset);
    }

    /// Scrolls to the end, and stays there as the last blocks are laid out.
    pub fn scroll_to_end(&mut self) {
        self.pinned_end = true;
        self.settle();
    }

    fn anchor_to(&mut self, offset: f32) {
        self.scroll = offset.clamp(0.0, self.max_scroll());
        let block = self.layout.block_at(self.scroll - self.pad);
        self.anchor = (block, self.scroll - self.layout.top(block));
    }

    pub fn scroll_by(&mut self, delta: f32) {
        self.set_scroll(self.scroll + delta);
    }

    /// Scrolls so block `index` starts at the top of the view.
    pub fn scroll_to_block(&mut self, index: usize) {
        self.pinned_end = false;
        self.anchor = (index, 0.0);
        self.settle();
    }

    /// Starts over at the top, for a new document or style.
    pub fn reset_scroll(&mut self) {
        self.anchor = (0, 0.0);
        self.pinned_end = false;
        self.scroll = 0.0;
        self.hover = None;
        self.pressed = None;
    }

    /// The link at the client pixel `point`.
    pub fn link_at(&mut self, point: Point) -> Option<u32> {
        if self.on_bar(point) {
            return None;
        }
        let x = (point.x - self.bounds.left) as f32 - self.pad;
        let y = (point.y - self.bounds.top) as f32 + self.scroll - self.pad;
        self.layout.link_at(x, y)
    }

    pub fn bar_scroll(&mut self) -> Scroll {
        Scroll {
            viewport: self.viewport().round() as i32,
            content: self.content_height().ceil() as i32,
            offset: self.scroll.round() as i32,
        }
    }

    pub fn on_bar(&self, point: Point) -> bool {
        !self.track.is_empty() && self.track.contains(point)
    }

    /// A press at client `y` on the bar: pages, or grabs the thumb. Returns
    /// whether the thumb was grabbed.
    pub fn press_bar(&mut self, y: i32) -> bool {
        let scroll = self.bar_scroll();
        match scrollbar::hit(self.track, scroll, Orientation::Vertical, y, self.dpi) {
            Some(TrackHit::Thumb) => {
                self.bar_drag = Some((y, scroll.offset));
                return true;
            }
            Some(TrackHit::Before) => self.scroll_by(-self.viewport()),
            Some(TrackHit::After) => self.scroll_by(self.viewport()),
            None => {}
        }
        false
    }

    /// Follows a thumb drag to client `y`. Returns whether one is in progress.
    pub fn drag_bar(&mut self, y: i32) -> bool {
        let Some((start, offset)) = self.bar_drag else {
            return false;
        };
        let scroll = self.bar_scroll();
        let to = scrollbar::offset_from_drag(
            self.track,
            scroll,
            Orientation::Vertical,
            offset,
            start,
            y,
            self.dpi,
        );
        self.set_scroll(to as f32);
        true
    }
}
