#![forbid(unsafe_code)]

//! The grid view's private state and the device-pixel metrics its painter and
//! event mapper share.

use std::rc::Rc;

use super::layout;
use super::model::{GridModel, Tile, TileSize};

/// Everything the grid view reads on paint and writes on input.
pub(crate) struct State {
    pub(crate) model: Rc<dyn GridModel>,
    pub(crate) size: TileSize,
    /// The selected tile, if any.
    pub(crate) selected: Option<usize>,
    /// The hovered tile, if any.
    pub(crate) hover: Option<usize>,
    /// The first visible content pixel, i.e. the scroll position.
    pub(crate) offset: i32,
    pub(crate) enabled: bool,
}

/// A tile size and gap converted to device pixels at one dpi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Metrics {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) gap: i32,
}

impl Metrics {
    /// Converts `size` at `dpi`, never yielding a zero tile.
    pub(crate) fn of(size: TileSize, dpi: u32) -> Metrics {
        Metrics {
            width: size.width.to_px(dpi).value().max(1),
            height: size.height.to_px(dpi).value().max(1),
            gap: size.gap.to_px(dpi).value().max(0),
        }
    }

    /// The horizontal distance between a column and the next.
    pub(crate) fn col_stride(self) -> i32 {
        self.width + self.gap
    }

    /// The vertical distance between a row and the next.
    pub(crate) fn row_stride(self) -> i32 {
        self.height + self.gap
    }
}

impl State {
    /// A state over `model`, with the first tile selected (or none when empty).
    pub(crate) fn new(model: Rc<dyn GridModel>) -> State {
        let selected = (!model.is_empty()).then_some(0);
        State {
            model,
            size: TileSize::default(),
            selected,
            hover: None,
            offset: 0,
            enabled: true,
        }
    }

    /// The number of tiles.
    pub(crate) fn len(&self) -> usize {
        self.model.len()
    }

    /// The tile at `index`, borrowed from the model.
    pub(crate) fn tile(&self, index: usize) -> Option<Tile<'_>> {
        self.model.tile(index)
    }

    /// How many columns fit `viewport_width` at the current size.
    pub(crate) fn columns(&self, viewport_width: i32, dpi: u32) -> usize {
        let metrics = Metrics::of(self.size, dpi);
        layout::columns_for_width(viewport_width, metrics.width, metrics.gap)
    }

    /// The largest offset that still shows content in the viewport.
    pub(crate) fn max_offset(&self, viewport_width: i32, viewport_height: i32, dpi: u32) -> i32 {
        let metrics = Metrics::of(self.size, dpi);
        let columns = self.columns(viewport_width, dpi);
        let content = layout::content_height_px(self.len(), columns, metrics.height, metrics.gap);
        (content - viewport_height).max(0)
    }

    /// Clamps the offset into `[0, max_offset]`.
    pub(crate) fn clamp_offset(&mut self, viewport_width: i32, viewport_height: i32, dpi: u32) {
        self.offset = self
            .offset
            .clamp(0, self.max_offset(viewport_width, viewport_height, dpi));
    }

    /// Scrolls just enough that `index` is fully visible.
    pub(crate) fn ensure_visible(
        &mut self,
        index: usize,
        viewport_width: i32,
        viewport_height: i32,
        dpi: u32,
    ) {
        if index >= self.len() {
            return;
        }
        let metrics = Metrics::of(self.size, dpi);
        let columns = self.columns(viewport_width, dpi).max(1);
        let top = (index / columns) as i32 * metrics.row_stride();
        let bottom = top + metrics.height;
        if top < self.offset {
            self.offset = top;
        } else if bottom > self.offset + viewport_height {
            self.offset = bottom - viewport_height;
        }
        self.clamp_offset(viewport_width, viewport_height, dpi);
    }
}
