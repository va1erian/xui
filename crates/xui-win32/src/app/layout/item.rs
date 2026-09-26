#![forbid(unsafe_code)]

//! The sizing/Content plumbing behind [`Layout`](super::Layout): how an item is
//! sized, the widget handle the tree moves, and the [`LayoutItem`] entries a
//! [`Layout`](super::Layout) is built from.

use std::cell::Cell;
use std::rc::Rc;

use crate::controls::control::{AsControl, Control};
use crate::geometry::{Rect, Size};
use crate::hwnd::Hwnd;
use crate::layout::{Anchor, StackDirection, StackSlot};
use crate::sys;
use crate::units::{Dip, Px};

use super::Layout;
use super::split::SplitNode;
use super::tabs::TabsNode;

/// How an item is sized: along the parent's main axis, or, for `width`/
/// `height`, along a named axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Sizing {
    /// The widget's natural size along the main axis.
    Auto,
    /// Exactly this many design units along the main axis.
    Fixed(Dip),
    /// At least this many design units along the main axis; shrinks only when
    /// the parent is too small to honour every item.
    Min(Dip),
    /// A share of the leftover main-axis space, proportional to its weight.
    Fill(u32),
    /// Exactly this many design units along the width axis.
    Width(Dip),
    /// Exactly this many design units along the height axis.
    Height(Dip),
    /// An absolute placement in a [`Layout::free`](super::Layout::free) parent:
    /// the widget keeps its bounds and follows the parent per its [`Anchor`].
    /// Ignored by a stack layout, where it sizes as `Auto`.
    Anchored(Anchor),
}

/// A widget the layout can move: its handle plus the shared cells that keep the
/// widget's [`Control`] bounds and visibility in sync with the OS window.
#[derive(Clone)]
pub(crate) struct WidgetHandle {
    hwnd: Hwnd,
    bounds: Rc<Cell<Rect>>,
    pub(crate) visible: Rc<Cell<bool>>,
    /// The widget's natural extent per axis, captured the first time the
    /// installed layout measures it. Cached so an `Auto` slot that overflow
    /// shrinking resized does not feed its shrunken size back in as the new
    /// natural size (which would ratchet the layout down on every relayout).
    natural: Rc<Cell<[Option<i32>; 2]>>,
    /// The widget's design bounds (device pixels at 96 DPI), captured the first
    /// time a free layout anchors it. The anchor maths must run against the
    /// *design* bounds, not the current, already-resized ones, and in
    /// DPI-independent units so a `WM_DPICHANGED` re-scales rather than re-reads.
    design: Rc<Cell<Option<Rect>>>,
}

impl WidgetHandle {
    fn from_control(control: &Control) -> WidgetHandle {
        WidgetHandle {
            hwnd: control.hwnd(),
            bounds: control.bounds_handle(),
            visible: control.visible_handle(),
            natural: Rc::new(Cell::new([None, None])),
            design: Rc::new(Cell::new(None)),
        }
    }

    /// A handle sharing `bounds`/`visible` with a widget laid out by a node
    /// kind of its own (a split divider, a tab control).
    pub(crate) fn new(hwnd: Hwnd, bounds: Rc<Cell<Rect>>, visible: Rc<Cell<bool>>) -> WidgetHandle {
        WidgetHandle {
            hwnd,
            bounds,
            visible,
            natural: Rc::new(Cell::new([None, None])),
            design: Rc::new(Cell::new(None)),
        }
    }

    /// Whether the widget takes part in layout.
    pub(crate) fn is_visible(&self) -> bool {
        self.visible.get()
    }

    /// The widget's natural extent along `direction`, in device pixels.
    fn natural(&self, direction: StackDirection) -> i32 {
        let axis = match direction {
            StackDirection::Horizontal => 0,
            StackDirection::Vertical => 1,
        };
        if let Some(cached) = self.natural.get()[axis] {
            return cached;
        }
        let bounds = self.bounds.get();
        let extent = match direction {
            StackDirection::Horizontal => bounds.width(),
            StackDirection::Vertical => bounds.height(),
        }
        .max(0);
        let mut cache = self.natural.get();
        cache[axis] = Some(extent);
        self.natural.set(cache);
        extent
    }

    /// The widget's design bounds, in device pixels at 96 DPI, captured from
    /// its current bounds at `dpi` the first time a free layout needs them.
    /// Later relayouts (a resize, a DPI change) reuse the capture, so the
    /// anchor always grows the *original* control rather than the last result.
    pub(crate) fn design_bounds(&self, dpi: u32) -> Rect {
        if let Some(design) = self.design.get() {
            return design;
        }
        let bounds = self.bounds.get();
        let design = Rect::new(
            Px(bounds.left).to_dip(dpi).to_px(96).value(),
            Px(bounds.top).to_dip(dpi).to_px(96).value(),
            Px(bounds.right).to_dip(dpi).to_px(96).value(),
            Px(bounds.bottom).to_dip(dpi).to_px(96).value(),
        );
        self.design.set(Some(design));
        design
    }

    /// Records the bounds the layout assigned (the OS move is batched).
    pub(crate) fn set_bounds(&self, bounds: Rect) {
        self.bounds.set(bounds);
    }

    /// Shows or hides the widget, keeping the shared flag in sync. A node that
    /// pages its content (tabs) toggles this as the selection changes.
    pub(crate) fn set_visible(&self, visible: bool) {
        if self.visible.get() == visible {
            return;
        }
        self.visible.set(visible);
        if !self.hwnd.is_null() {
            let kind = if visible {
                sys::window::ShowKind::Normal
            } else {
                sys::window::ShowKind::Hidden
            };
            sys::window::show(self.hwnd, kind);
        }
    }

    /// The widget's handle.
    pub(crate) fn hwnd(&self) -> Hwnd {
        self.hwnd
    }
}

/// One widget positioned by [`Layout::compute`](super::Layout::compute).
pub(crate) struct Placed {
    pub(crate) handle: WidgetHandle,
    pub(crate) rect: Rect,
}

/// Wraps a widget's control with an explicit sizing.
pub(super) fn widget_item(control: &Control, sizing: Sizing) -> LayoutItem {
    LayoutItem {
        content: Content::Widget(WidgetHandle::from_control(control)),
        sizing,
    }
}

/// What a [`LayoutItem`] contains. This is the seam for new node kinds: a split
/// divider (#11) or a stack of pages (#15) becomes another variant, handled in
/// [`Layout::compute`](super::Layout::compute) alongside the nested layout.
#[derive(Clone)]
pub(crate) enum Content {
    Widget(WidgetHandle),
    Nested(Box<Layout>),
    Split(Box<SplitNode>),
    Tabs(Box<TabsNode>),
}

/// One entry in a [`Layout`](super::Layout): a widget or a nested layout, with
/// its sizing.
///
/// Built for you by [`LayoutExt`] and the [`Layout`](super::Layout) builders, so
/// it is rarely named directly.
#[derive(Clone)]
pub struct LayoutItem {
    pub(crate) content: Content,
    pub(crate) sizing: Sizing,
}

impl LayoutItem {
    pub(crate) fn is_visible(&self) -> bool {
        match &self.content {
            Content::Widget(handle) => handle.is_visible(),
            Content::Nested(nested) => nested.slots.iter().any(LayoutItem::is_visible),
            Content::Split(node) => node.is_visible(),
            Content::Tabs(node) => node.is_visible(),
        }
    }

    /// Shows or hides every widget in this item's subtree. Tabs use it to page
    /// their content; hidden widgets take no space and are skipped by the tab
    /// order.
    pub(crate) fn set_tree_visible(&self, visible: bool) {
        match &self.content {
            Content::Widget(handle) => handle.set_visible(visible),
            Content::Nested(nested) => {
                for item in &nested.slots {
                    item.set_tree_visible(visible);
                }
            }
            Content::Split(node) => node.set_tree_visible(visible),
            Content::Tabs(node) => node.set_tree_visible(visible),
        }
    }

    /// The item's content kind, for the window's split-binding pass.
    pub(crate) fn content(&self) -> &Content {
        &self.content
    }

    /// Lays this item out inside `rect`, appending its visible leaves to `out`.
    pub(crate) fn compute(&self, rect: Rect, dpi: u32, out: &mut Vec<Placed>) {
        match &self.content {
            Content::Widget(handle) => out.push(Placed {
                handle: handle.clone(),
                rect,
            }),
            Content::Nested(nested) => out.extend(nested.compute(rect, dpi)),
            Content::Split(node) => node.compute(rect, dpi, out),
            Content::Tabs(node) => node.compute(rect, dpi, out),
        }
    }

    pub(crate) fn stack_slot(&self, direction: StackDirection) -> StackSlot {
        match self.sizing {
            Sizing::Fill(weight) => StackSlot::Fill(weight),
            Sizing::Fixed(size) => StackSlot::Fixed(size),
            Sizing::Min(size) => StackSlot::Min(size),
            Sizing::Width(size) if direction == StackDirection::Horizontal => {
                StackSlot::Fixed(size)
            }
            Sizing::Height(size) if direction == StackDirection::Vertical => StackSlot::Fixed(size),
            // A named-axis size that does not match the main axis sizes the
            // cross axis instead, so the main axis keeps its natural size. An
            // anchored item in a stack parent is not absolute, so it also keeps
            // its natural size.
            Sizing::Auto | Sizing::Width(_) | Sizing::Height(_) | Sizing::Anchored(_) => {
                match &self.content {
                    Content::Widget(handle) => StackSlot::FixedPx(Px(handle.natural(direction))),
                    Content::Nested(_) | Content::Split(_) | Content::Tabs(_) => StackSlot::Fill(1),
                }
            }
        }
    }

    /// The item's absolute [`Anchor`] in a free layout, or
    /// [`Anchor::TopLeft`] when it has none.
    pub(crate) fn anchor(&self) -> Anchor {
        match self.sizing {
            Sizing::Anchored(anchor) => anchor,
            _ => Anchor::TopLeft,
        }
    }

    /// The item's natural size in device pixels at `dpi`, for the parent
    /// stack's preferred-size pass. `direction` is the parent's main axis:
    /// `Fixed`/`Min`/`Fill` resolve along it (a `Fill` item contributes no
    /// natural main extent), while `Width`/`Height` pin the axis they name.
    pub(crate) fn natural_size(&self, direction: StackDirection, dpi: u32) -> Size {
        let content = match &self.content {
            Content::Widget(handle) => Size::new(
                handle.natural(StackDirection::Horizontal),
                handle.natural(StackDirection::Vertical),
            ),
            Content::Nested(nested) => nested.preferred_size(dpi),
            Content::Split(node) => node.preferred_size(dpi),
            Content::Tabs(node) => node.preferred_size(dpi),
        };
        let px = |value: Dip| value.to_px(dpi).value().max(0);
        match self.sizing {
            Sizing::Width(value) => Size::new(px(value), content.height),
            Sizing::Height(value) => Size::new(content.width, px(value)),
            sizing => {
                let (main, cross) = match direction {
                    StackDirection::Horizontal => (content.width, content.height),
                    StackDirection::Vertical => (content.height, content.width),
                };
                let main = match sizing {
                    Sizing::Fixed(value) => px(value),
                    Sizing::Min(value) => main.max(px(value)),
                    Sizing::Fill(_) => 0,
                    // A widget, or an anchored item in a stack, keeps its size.
                    _ => main,
                };
                match direction {
                    StackDirection::Horizontal => Size::new(main, cross),
                    StackDirection::Vertical => Size::new(cross, main),
                }
            }
        }
    }

    /// The cross-axis extent this item asked for, if any.
    pub(crate) fn cross_extent(&self, direction: StackDirection) -> Option<Dip> {
        match (self.sizing, direction) {
            (Sizing::Width(size), StackDirection::Vertical) => Some(size),
            (Sizing::Height(size), StackDirection::Horizontal) => Some(size),
            _ => None,
        }
    }
}

/// Converts into a [`LayoutItem`], so [`row!`](crate::row) /
/// [`column!`](crate::column) accept widgets and nested layouts alike.
pub trait IntoLayoutItem {
    /// Wraps `self` as a layout item.
    fn into_layout_item(self) -> LayoutItem;
}

impl<T: AsControl + ?Sized> IntoLayoutItem for &T {
    fn into_layout_item(self) -> LayoutItem {
        widget_item(self.control(), Sizing::Auto)
    }
}

impl IntoLayoutItem for &LayoutItem {
    fn into_layout_item(self) -> LayoutItem {
        self.clone()
    }
}

impl IntoLayoutItem for &Layout {
    fn into_layout_item(self) -> LayoutItem {
        LayoutItem {
            content: Content::Nested(Box::new(self.clone())),
            sizing: Sizing::Fill(1),
        }
    }
}

impl IntoLayoutItem for LayoutItem {
    fn into_layout_item(self) -> LayoutItem {
        self
    }
}

impl IntoLayoutItem for Layout {
    fn into_layout_item(self) -> LayoutItem {
        LayoutItem {
            content: Content::Nested(Box::new(self)),
            sizing: Sizing::Fill(1),
        }
    }
}

/// Builder methods that turn any widget into a [`LayoutItem`].
pub trait LayoutExt: AsControl {
    /// Wraps the widget with its natural size along the parent's main axis.
    fn layout_item(&self) -> LayoutItem {
        widget_item(self.control(), Sizing::Auto)
    }

    /// A share of the parent's leftover space.
    fn fill(&self, weight: u32) -> LayoutItem {
        widget_item(self.control(), Sizing::Fill(weight))
    }

    /// A fixed size along the parent's main axis.
    fn fixed(&self, size: Dip) -> LayoutItem {
        widget_item(self.control(), Sizing::Fixed(size))
    }

    /// A fixed width: the main axis in a row, the cross axis in a column.
    fn width(&self, size: Dip) -> LayoutItem {
        widget_item(self.control(), Sizing::Width(size))
    }

    /// A fixed height: the main axis in a column, the cross axis in a row.
    fn height(&self, size: Dip) -> LayoutItem {
        widget_item(self.control(), Sizing::Height(size))
    }

    /// A minimum size along the parent's main axis.
    fn min(&self, size: Dip) -> LayoutItem {
        widget_item(self.control(), Sizing::Min(size))
    }

    /// Places the widget absolutely in a [`Layout::free`](super::Layout::free)
    /// parent: it keeps its current bounds and follows the parent per `anchor`.
    fn anchor(&self, anchor: Anchor) -> LayoutItem {
        widget_item(self.control(), Sizing::Anchored(anchor))
    }
}

impl<T: AsControl + ?Sized> LayoutExt for T {}
