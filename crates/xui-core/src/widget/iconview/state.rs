#![forbid(unsafe_code)]

//! The icon view's private state, plus the viewport arithmetic its painter, its
//! event mapper and its scrollbar share.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::layout;
use super::metrics::Metrics;
use super::model::{IconModel, IconSize};
use crate::geometry::Rect;
use crate::widget::SelectionMode;

/// The column count and scroll metrics of one viewport. Computed once from the
/// node's bounds and DPI, so the painter, the hit test and the scrollbar agree
/// exactly, including the width the scrollbar reserves.
pub(crate) struct Viewport {
    /// How many tiles fit across the content width.
    pub(crate) columns: usize,
    /// The content width, excluding the scrollbar when it shows.
    pub(crate) width: i32,
    /// The viewport height.
    pub(crate) height: i32,
    /// The width the scrollbar occupies (`0` when it is hidden).
    pub(crate) bar_width: i32,
    /// The total content height in pixels.
    pub(crate) content_height: i32,
}

/// Everything the icon view reads on paint and writes on input.
pub(crate) struct State {
    pub(crate) model: Rc<dyn IconModel>,
    pub(crate) size: IconSize,
    pub(crate) mode: SelectionMode,
    /// Selected items, ascending and unique.
    pub(crate) selected: BTreeSet<usize>,
    /// The item the keyboard acts on.
    pub(crate) focused: Option<usize>,
    /// The item a Shift range is measured from.
    pub(crate) anchor: Option<usize>,
    pub(crate) hover: Option<usize>,
    /// The first visible content pixel, i.e. the scroll position.
    pub(crate) offset: i32,
    pub(crate) enabled: bool,
    /// Whether the control currently holds the keyboard focus, so the painter
    /// draws the focus rectangle.
    pub(crate) has_focus: bool,
    /// The metrics of the current `(size, dpi)`, so the paint hot path does not
    /// rederive them per tile.
    cache: Cell<Option<(IconSize, u32, Metrics)>>,
}

impl State {
    /// A state over `model`, with the first item selected (or none when empty).
    pub(crate) fn new(model: Rc<dyn IconModel>) -> State {
        let focused = (model.items() > 0).then_some(0);
        let mut selected = BTreeSet::new();
        if let Some(item) = focused {
            selected.insert(item);
        }
        State {
            model,
            size: IconSize::default(),
            mode: SelectionMode::Single,
            selected,
            focused,
            anchor: focused,
            hover: None,
            offset: 0,
            enabled: true,
            has_focus: false,
            cache: Cell::new(None),
        }
    }

    /// The number of items.
    pub(crate) fn len(&self) -> usize {
        self.model.items()
    }

    /// Every selected item, ascending.
    pub(crate) fn selection(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// The item messages and the keyboard act on: the focused item, else the
    /// first selected one.
    pub(crate) fn primary(&self) -> Option<usize> {
        self.focused
            .or_else(|| self.selected.iter().next().copied())
    }

    /// Replaces the selection with `item`.
    pub(crate) fn set_single(&mut self, item: usize) {
        self.selected.clear();
        self.selected.insert(item);
        self.focused = Some(item);
        self.anchor = Some(item);
    }

    /// Toggles `item` in or out of the selection (Ctrl+click).
    pub(crate) fn toggle(&mut self, item: usize) {
        if !self.selected.remove(&item) {
            self.selected.insert(item);
        }
        self.focused = Some(item);
        self.anchor = Some(item);
    }

    /// Replaces the selection with the range from the anchor to `item`
    /// (Shift+click/arrow).
    pub(crate) fn extend(&mut self, item: usize) {
        let anchor = self.anchor.unwrap_or(item);
        let (low, high) = if anchor <= item {
            (anchor, item)
        } else {
            (item, anchor)
        };
        self.selected = (low..=high).collect();
        self.focused = Some(item);
        if self.anchor.is_none() {
            self.anchor = Some(item);
        }
    }

    /// Clears the selection, focus and anchor. Returns whether anything changed.
    pub(crate) fn clear(&mut self) -> bool {
        let changed = !self.selected.is_empty() || self.focused.is_some() || self.anchor.is_some();
        self.selected.clear();
        self.focused = None;
        self.anchor = None;
        changed
    }

    /// The tile and text metrics at `dpi`, cached per `(size, dpi)`.
    pub(crate) fn metrics(&self, dpi: u32) -> Metrics {
        if let Some((size, cached_dpi, metrics)) = self.cache.get()
            && size == self.size
            && cached_dpi == dpi
        {
            return metrics;
        }
        let metrics = Metrics::of(self.size, dpi);
        self.cache.set(Some((self.size, dpi, metrics)));
        metrics
    }

    /// Drops the metrics cache, so the next [`metrics`](State::metrics) call
    /// rederives it. Used after an icon-size change.
    pub(crate) fn cache_reset(&self) {
        self.cache.set(None);
    }

    /// The viewport metrics of `bounds` at `dpi`: reserve the scrollbar's width
    /// only when the content overflows, then reflow the columns to the width
    /// that leaves.
    pub(crate) fn viewport(&self, bounds: Rect, dpi: u32) -> Viewport {
        let metrics = self.metrics(dpi);
        let width = bounds.width().max(0);
        let height = bounds.height().max(0);
        let full_columns = layout::columns_for(width, metrics.width, metrics.gap);
        let full_height = layout::content_height(self.len(), full_columns, metrics);
        let bar_width = if full_height > height {
            crate::widget::scrollbar::THICKNESS.to_px(dpi).value()
        } else {
            0
        };
        let content_width = (width - bar_width).max(0);
        let columns = layout::columns_for(content_width, metrics.width, metrics.gap);
        let content_height = layout::content_height(self.len(), columns, metrics);
        Viewport {
            columns,
            width: content_width,
            height,
            bar_width,
            content_height,
        }
    }

    /// The largest offset that still shows content in `bounds`.
    pub(crate) fn max_offset(&self, bounds: Rect, dpi: u32) -> i32 {
        (self.viewport(bounds, dpi).content_height - bounds.height().max(0)).max(0)
    }

    /// Clamps the offset into `[0, max_offset]`.
    pub(crate) fn clamp_offset(&mut self, bounds: Rect, dpi: u32) {
        self.offset = self.offset.clamp(0, self.max_offset(bounds, dpi));
    }

    /// Scrolls just enough that `item` is fully visible.
    pub(crate) fn ensure_visible(&mut self, item: usize, bounds: Rect, dpi: u32) {
        if item >= self.len() {
            return;
        }
        let viewport = self.viewport(bounds, dpi);
        let metrics = self.metrics(dpi);
        let tile = layout::tile_rect(item, viewport.columns, metrics);
        if tile.top < self.offset {
            self.offset = tile.top;
        } else if tile.bottom > self.offset + viewport.height {
            self.offset = tile.bottom - viewport.height;
        }
        self.clamp_offset(bounds, dpi);
    }

    /// Replaces the model, keeping the selection where it still exists and
    /// dropping everything past the new end. The scroll resets to the top.
    pub(crate) fn replace_model(&mut self, model: Rc<dyn IconModel>) {
        self.model = model;
        let len = self.len();
        self.selected.retain(|item| *item < len);
        if self.focused.is_none_or(|item| item >= len) {
            self.focused = if len == 0 { None } else { Some(len - 1) };
        }
        if self.selected.is_empty()
            && let Some(focused) = self.focused
        {
            self.selected.insert(focused);
        }
        self.anchor = self.focused;
        self.hover = None;
        self.offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(len: usize) -> State {
        let model: Vec<String> = (0..len).map(|index| format!("item {index}")).collect();
        State::new(Rc::new(model))
    }

    #[test]
    fn a_new_view_selects_the_first_item() {
        let state = state(4);
        assert_eq!(state.selected, [0].into_iter().collect());
        assert_eq!(state.focused, Some(0));
        assert!(State::new(Rc::new(Vec::<String>::new())).focused.is_none());
    }

    #[test]
    fn shift_extends_between_the_anchor_and_the_item() {
        let mut state = state(6);
        state.set_single(1);
        state.extend(4);
        assert_eq!(state.selection(), vec![1, 2, 3, 4]);
        state.extend(0);
        assert_eq!(state.selection(), vec![0, 1], "still measured from item 1");
    }

    #[test]
    fn clearing_drops_everything() {
        let mut state = state(3);
        assert!(state.clear());
        assert!(!state.clear());
        assert!(state.selection().is_empty());
        assert_eq!(state.focused, None);
    }

    #[test]
    fn replacing_a_model_clamps_the_state() {
        let mut state = state(10);
        state.set_single(8);
        state.offset = 500;
        state.replace_model(Rc::new(vec!["a".to_string(), "b".to_string()]));
        assert!(state.selection().iter().all(|item| *item < 2));
        assert!(state.focused.is_some_and(|item| item < 2));
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn the_metrics_cache_follows_size_and_dpi() {
        let state = state(3);
        let at96 = state.metrics(96);
        assert_eq!(state.metrics(96), at96);
        assert_ne!(state.metrics(144), at96);
    }
}
