#![forbid(unsafe_code)]

//! The device-pixel tile and text metrics the icon view lays out and paints
//! with, derived once per `(IconSize, dpi)`.

use super::model::IconSize;
use crate::geometry::Rect;
use crate::units::Dip;

/// The gap between tiles, on both axes.
const GAP: Dip = Dip(4.0);

/// A tile size and its text metrics, converted to device pixels at one dpi.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Metrics {
    /// Tile width.
    pub(crate) width: i32,
    /// Tile height.
    pub(crate) height: i32,
    /// Gap between tiles, on both axes.
    pub(crate) gap: i32,
    /// Icon side.
    pub(crate) icon: i32,
    /// Inset from the tile's left/right edge.
    pub(crate) pad_x: i32,
    /// Gap between the icon and the text block.
    pub(crate) icon_gap: i32,
    /// One text line's height.
    pub(crate) line_height: i32,
    /// The text size as a design value.
    pub(crate) text_size: Dip,
    /// How many of the up-to-three lines fit the tile (`1..=3`).
    pub(crate) lines: usize,
}

/// The per-size design values a [`Metrics`] is derived from.
struct Spec {
    tile_w: Dip,
    tile_h: Dip,
    line: Dip,
    text: Dip,
    pad_x: Dip,
    pad_y: Dip,
    icon_gap: Dip,
}

/// The design spec of `size`; each one is documented by
/// [`IconSize`](super::model::IconSize).
fn spec(size: IconSize) -> Spec {
    match size {
        IconSize::Small => Spec {
            tile_w: Dip(180.0),
            tile_h: Dip(22.0),
            line: Dip(14.0),
            text: Dip(11.0),
            pad_x: Dip(5.0),
            pad_y: Dip(2.0),
            icon_gap: Dip(6.0),
        },
        IconSize::Medium => Spec {
            tile_w: Dip(190.0),
            tile_h: Dip(50.0),
            line: Dip(15.0),
            text: Dip(12.0),
            pad_x: Dip(6.0),
            pad_y: Dip(2.0),
            icon_gap: Dip(6.0),
        },
        IconSize::Large => Spec {
            tile_w: Dip(210.0),
            tile_h: Dip(66.0),
            line: Dip(16.0),
            text: Dip(13.0),
            pad_x: Dip(7.0),
            pad_y: Dip(2.0),
            icon_gap: Dip(8.0),
        },
    }
}

impl Metrics {
    /// Converts `size` at `dpi`, never yielding a zero tile or an empty text
    /// block.
    pub(crate) fn of(size: IconSize, dpi: u32) -> Metrics {
        let spec = spec(size);
        let tile_h = spec.tile_h.to_px(dpi).value().max(1);
        let line_height = spec.line.to_px(dpi).value().max(1);
        let pad_y = spec.pad_y.to_px(dpi).value().max(0);
        let lines = (((tile_h - 2 * pad_y).max(0) / line_height) as usize).clamp(1, 3);
        Metrics {
            width: spec.tile_w.to_px(dpi).value().max(1),
            height: tile_h,
            gap: GAP.to_px(dpi).value().max(0),
            icon: size.icon().to_px(dpi).value().max(1),
            pad_x: spec.pad_x.to_px(dpi).value().max(0),
            icon_gap: spec.icon_gap.to_px(dpi).value().max(0),
            line_height,
            text_size: spec.text,
            lines,
        }
    }

    /// The horizontal distance from one column to the next.
    pub(crate) fn stride_x(self) -> i32 {
        self.width + self.gap
    }

    /// The vertical distance from one row to the next.
    pub(crate) fn stride_y(self) -> i32 {
        self.height + self.gap
    }

    /// The icon's square, centred vertically at the tile's left.
    pub(crate) fn icon_rect(self, tile: Rect) -> Rect {
        let left = tile.left + self.pad_x;
        let top = tile.top + (tile.height() - self.icon) / 2;
        Rect::new(left, top, left + self.icon, top + self.icon)
    }

    /// The block the text lines are drawn into, vertically centred against the
    /// icon.
    pub(crate) fn text_rect(self, tile: Rect) -> Rect {
        let left = tile.left + self.pad_x + self.icon + self.icon_gap;
        let right = (tile.right - self.pad_x).max(left);
        let block = self.lines as i32 * self.line_height;
        let top = tile.top + (tile.height() - block) / 2;
        Rect::new(left, top, right, top + block)
    }

    /// The `line`-th text line's rectangle inside `text`, or `None` past the
    /// lines that fit.
    pub(crate) fn line_rect(self, text: Rect, line: usize) -> Option<Rect> {
        if line >= self.lines {
            return None;
        }
        let top = text.top + line as i32 * self.line_height;
        Some(Rect::new(
            text.left,
            top,
            text.right,
            top + self.line_height,
        ))
    }
}
