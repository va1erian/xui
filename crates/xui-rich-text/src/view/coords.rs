#![forbid(unsafe_code)]

//! Converting between layout coordinates (the flow's own pixels) and view
//! coordinates (the node's client pixels, with the text margin and scroll).

use xui_core::geometry::{Point, Rect};

use super::state::{State, pad_px};
use crate::model::{Affinity, DocPos};

impl State {
    /// Lays the document out for the current bounds if it is not yet.
    pub fn ready(&mut self) {
        let (bounds, dpi) = (self.bounds, self.dpi);
        self.prepare(bounds, dpi);
    }

    /// A layout rectangle in view coordinates.
    pub fn to_view(&self, rect: Rect) -> Rect {
        rect.offset(pad_px(self.dpi), -self.shift())
    }

    /// A view point in layout coordinates.
    pub fn to_layout(&self, point: Point) -> Point {
        Point::new(point.x - pad_px(self.dpi), point.y + self.shift())
    }

    /// The caret box at `pos` in view coordinates.
    pub fn caret_rect_view(&self, pos: DocPos, affinity: Affinity) -> Rect {
        self.to_view(self.layout.caret_rect_with(&self.ed.doc, pos, affinity))
    }

    /// The position nearest the view point `point`.
    pub fn pos_at_view(&self, point: Point) -> DocPos {
        self.layout.pos_at(&self.ed.doc, self.to_layout(point))
    }

    /// Scrolls the least that brings `rect` (layout coordinates) into the
    /// viewport; a rectangle taller than the viewport shows its top.
    pub fn ensure_visible(&mut self, rect: Rect) {
        let pad = pad_px(self.dpi) as f32;
        let (top, bottom) = (rect.top as f32 + pad, rect.bottom as f32 + pad);
        if top < self.scroll || bottom - top > self.viewport {
            self.scroll = top;
        } else if bottom > self.scroll + self.viewport {
            self.scroll = bottom - self.viewport;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }
}
