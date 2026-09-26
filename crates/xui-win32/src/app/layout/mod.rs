#![forbid(unsafe_code)]

//! The widget-facing layout tree: `column!`/`row!` turn widgets into a tree the
//! window lays out again on every resize and DPI change.
//!
//! Items are any [`AsControl`] or a nested [`Layout`]. [`LayoutExt`] sizes an
//! item: `fill`/`min`/`fixed` along the parent's main axis, `width`/`height`
//! along their own axis, and by default a widget keeps its current size. Hidden
//! widgets take no space. [`Layout::compute`] is pure: it maps the tree and a
//! parent rectangle to one rectangle per visible leaf, using the existing
//! [`Stack`] arithmetic.

use crate::app::Ui;
use crate::geometry::{Rect, Size};
use crate::layout::{Insets, Stack, StackDirection};
use crate::units::Dip;

#[cfg(test)]
mod tests;

mod free;
mod item;
pub(crate) mod split;
pub(crate) mod tabs;

pub(crate) use item::{Content, Placed, Sizing, WidgetHandle};
pub use item::{IntoLayoutItem, LayoutExt, LayoutItem};

/// A row, column or absolute (free) arrangement of items, laid out as a tree
/// the window owns.
///
/// Build a stack with [`row!`](crate::row) / [`column!`](crate::column) or
/// [`Layout::row`] / [`Layout::column`], and an absolute layout with
/// [`Layout::free`]; install either with [`Ui::set_layout`](crate::Ui::set_layout).
#[derive(Clone)]
pub struct Layout {
    direction: StackDirection,
    spacing: Dip,
    margins: Insets,
    slots: Vec<LayoutItem>,
    /// `Some(design origin)` for a free layout: items are placed by their
    /// [`Anchor`] instead of being stacked. The origin is the parent's design
    /// size in device pixels at 96 DPI.
    origin: Option<Size>,
}

impl Layout {
    /// A layout that places its items left to right.
    pub const fn row() -> Layout {
        Layout {
            direction: StackDirection::Horizontal,
            spacing: Dip(0.0),
            margins: Insets::new(Dip(0.0), Dip(0.0), Dip(0.0), Dip(0.0)),
            slots: Vec::new(),
            origin: None,
        }
    }

    /// A layout that places its items top to bottom.
    pub const fn column() -> Layout {
        Layout {
            direction: StackDirection::Vertical,
            spacing: Dip(0.0),
            margins: Insets::new(Dip(0.0), Dip(0.0), Dip(0.0), Dip(0.0)),
            slots: Vec::new(),
            origin: None,
        }
    }

    /// An absolute layout: each item keeps the bounds it had when the layout
    /// was installed and follows the parent's resize per its
    /// [`LayoutExt::anchor`].
    ///
    /// `origin` is the parent's *design* client size in device pixels at 96 DPI
    /// (the window it was designed against); it is scaled to the window's DPI
    /// on every relayout, so the anchors are DPI-independent. Spacing and
    /// margins do not apply to a free layout.
    pub const fn free(origin: Size) -> Layout {
        Layout {
            direction: StackDirection::Vertical,
            spacing: Dip(0.0),
            margins: Insets::new(Dip(0.0), Dip(0.0), Dip(0.0), Dip(0.0)),
            slots: Vec::new(),
            origin: Some(origin),
        }
    }

    /// The gap between adjacent items, in design units.
    pub const fn spacing(mut self, spacing: Dip) -> Layout {
        self.spacing = spacing;
        self
    }

    /// Margins inside the parent, in design units.
    pub const fn margins(mut self, insets: Insets) -> Layout {
        self.margins = insets;
        self
    }

    /// Appends a widget or a nested layout.
    pub fn item<I: IntoLayoutItem>(mut self, item: I) -> Layout {
        self.slots.push(item.into_layout_item());
        self
    }

    /// The installed items, for the window's split-binding pass.
    pub(crate) fn items(&self) -> &[LayoutItem] {
        &self.slots
    }

    /// Wraps this layout as a weighted item of its parent.
    pub fn fill(self, weight: u32) -> LayoutItem {
        self.item_with(Sizing::Fill(weight))
    }

    /// Wraps this layout as a fixed-size item of its parent.
    pub fn fixed(self, size: Dip) -> LayoutItem {
        self.item_with(Sizing::Fixed(size))
    }

    /// Wraps this layout with a fixed width, whichever axis that is.
    pub fn width(self, size: Dip) -> LayoutItem {
        self.item_with(Sizing::Width(size))
    }

    /// Wraps this layout with a fixed height, whichever axis that is.
    pub fn height(self, size: Dip) -> LayoutItem {
        self.item_with(Sizing::Height(size))
    }

    /// Wraps this layout as a minimum-size item of its parent.
    pub fn min(self, size: Dip) -> LayoutItem {
        self.item_with(Sizing::Min(size))
    }

    fn item_with(self, sizing: Sizing) -> LayoutItem {
        LayoutItem {
            content: Content::Nested(Box::new(self)),
            sizing,
        }
    }

    /// Lays the tree out inside `rect`, returning one entry per visible leaf.
    pub(crate) fn compute(&self, rect: Rect, dpi: u32) -> Vec<Placed> {
        if let Some(origin) = self.origin {
            return free::compute(self, origin, rect, dpi);
        }

        let visible: Vec<&LayoutItem> =
            self.slots.iter().filter(|item| item.is_visible()).collect();
        if visible.is_empty() {
            return Vec::new();
        }

        let mut stack = match self.direction {
            StackDirection::Horizontal => Stack::horizontal(),
            StackDirection::Vertical => Stack::vertical(),
        }
        .spacing(self.spacing)
        .margins(self.margins);
        for item in &visible {
            stack = stack.push(item.stack_slot(self.direction));
        }

        let areas = stack.split(rect, dpi);
        let mut placed = Vec::new();
        for (item, area) in visible.iter().zip(areas) {
            let area = match item.cross_extent(self.direction) {
                Some(size) => cross_rect(area, self.direction, size.to_px(dpi).value()),
                None => area,
            };
            item.compute(area, dpi, &mut placed);
        }
        placed
    }

    /// The size the layout's content wants, in device pixels at `dpi`.
    ///
    /// A stack packs its visible items along its main axis: margins plus each
    /// item's natural extent plus one `spacing` gap between adjacent items; the
    /// cross axis is the margins plus the largest natural cross extent. A
    /// [`free`](Layout::free) layout reports the furthest edge of its items'
    /// design bounds. A split adds both panes and its divider; a tab node takes
    /// its largest page. Sizing (`fill`/`min`/`fixed`/…) is honoured as in
    /// [`compute`](Layout::compute): a `fill` item contributes no natural extent,
    /// so it is the caller's job to open at least a workable size.
    ///
    /// The arithmetic is the pure [`Stack::preferred_size`] /
    /// [`free_preferred`](crate::layout::free_preferred) in `xui-core`, so it is
    /// DPI-independent. Use [`Ui::pack`] to apply the result to a window.
    pub fn preferred_size(&self, dpi: u32) -> Size {
        if let Some(origin) = self.origin {
            return free::preferred(self, origin, dpi);
        }
        let naturals: Vec<Size> = self
            .slots
            .iter()
            .filter(|item| item.is_visible())
            .map(|item| item.natural_size(self.direction, dpi))
            .collect();
        let stack = match self.direction {
            StackDirection::Horizontal => Stack::horizontal(),
            StackDirection::Vertical => Stack::vertical(),
        }
        .spacing(self.spacing)
        .margins(self.margins);
        stack.preferred_size(&naturals, dpi)
    }
}

/// Narrows `rect` to `extent` pixels along the cross axis, keeping the start
/// edge (the top for a row, the left for a column).
fn cross_rect(rect: Rect, direction: StackDirection, extent: i32) -> Rect {
    match direction {
        StackDirection::Horizontal => Rect::new(
            rect.left,
            rect.top,
            rect.right,
            (rect.top + extent).min(rect.bottom),
        ),
        StackDirection::Vertical => Rect::new(
            rect.left,
            rect.top,
            (rect.left + extent).min(rect.right),
            rect.bottom,
        ),
    }
}

impl<M: 'static> Ui<M> {
    /// The installed layout's [`preferred_size`](Layout::preferred_size) in
    /// device pixels at the window's current DPI, or `None` when no layout was
    /// installed.
    ///
    /// A frontend uses it to choose the window's initial size; call it before
    /// the layout has been laid out, or after any change that affects content.
    pub fn layout_preferred_size(&self) -> Option<Size> {
        let core = self.core_weak().upgrade()?;
        core.layout_preferred_size(self.dpi())
    }

    /// Applies the installed layout's preferred size as the window's minimum
    /// tracking size, so the user cannot shrink it below its content, and
    /// returns that size for the app to also use as the initial window size.
    ///
    /// `None` when no layout is installed. This targets the top-level window.
    pub fn pack(&self) -> Option<Size> {
        let size = self.layout_preferred_size()?;
        crate::sys::window_ext::set_min_size(self.hwnd(), size);
        Some(size)
    }
}

/// Builds a [`Layout`] that places its items top to bottom.
#[macro_export]
macro_rules! column {
    ($($item:expr),* $(,)?) => {
        $crate::Layout::column()$(.item(&$item))*
    };
}

/// Builds a [`Layout`] that places its items left to right.
#[macro_export]
macro_rules! row {
    ($($item:expr),* $(,)?) => {
        $crate::Layout::row()$(.item(&$item))*
    };
}

/// Builds a [`Split`](crate::Split) with two side-by-side panes.
///
/// ```ignore
/// split_row![tree, list]
///     .position(dip(220.0))
///     .min(dip(120.0), dip(200.0))
///     .on_moved(|p| Some(Msg::SplitMoved(p)))
/// ```
#[macro_export]
macro_rules! split_row {
    ($a:expr, $b:expr $(,)?) => {
        $crate::Split::row().a(&$a).b(&$b)
    };
}

/// Builds a [`Split`](crate::Split) with two stacked panes.
#[macro_export]
macro_rules! split_col {
    ($a:expr, $b:expr $(,)?) => {
        $crate::Split::column().a(&$a).b(&$b)
    };
}

/// Builds a [`Tabs`](crate::Tabs) node from `(title, page)` pairs, where each
/// page is any widget or nested layout.
///
/// ```ignore
/// tabs![("General", general_layout), ("Accounts", accounts_layout)]
///     .on_change(|index| Some(Msg::Tab(index)))
/// ```
#[macro_export]
macro_rules! tabs {
    ($( ($title:expr, $item:expr) ),* $(,)?) => {
        $crate::Tabs::new()$(.page($title, &$item))*
    };
}
