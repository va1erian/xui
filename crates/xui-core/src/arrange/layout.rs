#![forbid(unsafe_code)]

//! [`Layout`]: a container of entries before it is mounted, and the functions
//! that start one.

use std::rc::Rc;

use super::{Align, Entry, IntoChildren, IntoEntry, Kind, Track};
use crate::app::Ui;
use crate::backend::Result;
use crate::layout::{Group, Insets, Item, StackDirection};
use crate::units::Dip;
use crate::widget::Placeable;

#[derive(Clone)]
enum Arrangement {
    Stack(StackDirection),
    Grid(Vec<Track>),
    Wrap,
    Layered,
    Absolute(Option<(Dip, Dip)>),
}

/// A row, column, grid, wrap, overlay or absolute layout of entries, before
/// it is mounted.
pub struct Layout<M: 'static> {
    arrangement: Arrangement,
    gap: Dip,
    padding: Insets,
    align: Align,
    justify: Align,
    entries: Vec<Entry<M>>,
}

/// A layout that places its entries left to right.
pub fn row<M: 'static>() -> Layout<M> {
    Layout::new(Arrangement::Stack(StackDirection::Horizontal))
}

/// A layout that places its entries top to bottom.
pub fn column<M: 'static>() -> Layout<M> {
    Layout::new(Arrangement::Stack(StackDirection::Vertical))
}

/// A layout that flows its entries into `columns`, left to right and then row
/// by row: `grid([Track::Auto, Track::Fill(1)])` for label/field pairs.
pub fn grid<M: 'static>(columns: impl Into<Vec<Track>>) -> Layout<M> {
    Layout::new(Arrangement::Grid(columns.into()))
}

/// A layout that places its entries left to right at their natural sizes and
/// starts a new line when the next one does not fit: toolbars, chips, tiles.
/// [`gap`](Layout::gap) separates both the entries and the lines.
pub fn wrap<M: 'static>() -> Layout<M> {
    Layout::new(Arrangement::Wrap)
}

/// A layout that layers its entries over one another, the later above the
/// earlier, each at its natural size and centred unless it sets its own
/// [`align`](super::LayoutExt::align): a badge, a spinner or a hint over
/// content that is itself `.align(Align::Stretch)`.
pub fn overlay<M: 'static>() -> Layout<M> {
    Layout::new(Arrangement::Layered).align(Align::Center)
}

/// A layout that layers its entries over one another, the later above the
/// earlier, each filling the whole area unless it sets its own
/// [`align`](super::LayoutExt::align): a deck of pages the app shows one at a
/// time with [`Ui::set_visible`].
pub fn stack<M: 'static>() -> Layout<M> {
    Layout::new(Arrangement::Layered)
}

/// A layout that places each entry where its [`at`](super::LayoutExt::at)
/// says and moves it with its [`anchor`](super::LayoutExt::anchor) as the
/// layout grows or shrinks from its [`design_size`](Layout::design_size).
///
/// This is the one sanctioned home for free positioning: a designer surface,
/// a form imported from coordinates, a custom painter's hit regions. Prefer
/// rows, columns and grids everywhere else.
pub fn absolute<M: 'static>() -> Layout<M> {
    Layout::new(Arrangement::Absolute(None))
}

impl<M: 'static> Layout<M> {
    fn new(arrangement: Arrangement) -> Layout<M> {
        Layout {
            arrangement,
            gap: Dip(0.0),
            padding: Insets::default(),
            align: Align::Stretch,
            justify: Align::Start,
            entries: Vec::new(),
        }
    }

    /// The gap between adjacent entries (in a grid or a wrap, between rows
    /// and between columns), in design units.
    pub fn gap(mut self, gap: impl Into<Dip>) -> Layout<M> {
        self.gap = gap.into();
        self
    }

    /// Space inside the layout's edges, in design units: a number for every
    /// edge, or [`Insets`].
    pub fn padding(mut self, padding: impl Into<Insets>) -> Layout<M> {
        self.padding = padding.into();
        self
    }

    /// Where entries sit across the main axis (in a grid or an overlay,
    /// within their area on both axes; in a wrap, within their line) unless
    /// they set their own.
    pub fn align(mut self, align: Align) -> Layout<M> {
        self.align = align;
        self
    }

    /// Where a row's, column's or wrap line's entries sit along the main axis
    /// when none of them fills it. The default packs them at the start.
    pub fn justify(mut self, justify: Align) -> Layout<M> {
        self.justify = justify;
        self
    }

    /// For an [`absolute`] layout, the size its entries' positions were
    /// designed at (inside the padding): their anchors follow the difference
    /// between this and the size it is laid out at. Without it, the design
    /// size is the smallest that holds every entry. Other layouts ignore it.
    pub fn design_size(mut self, width: impl Into<Dip>, height: impl Into<Dip>) -> Layout<M> {
        if let Arrangement::Absolute(size) = &mut self.arrangement {
            *size = Some((width.into(), height.into()));
        }
        self
    }

    /// Appends a widget builder, a nested layout or an entry.
    pub fn child(mut self, entry: impl IntoEntry<M>) -> Layout<M> {
        self.entries.push(entry.into_entry());
        self
    }

    /// Appends several children at once: a tuple of builders and layouts, or
    /// a `Vec` of any one kind of them (an empty one needs its element type
    /// named, `Vec::<Entry<_>>::new()`).
    pub fn children(mut self, children: impl IntoChildren<M>) -> Layout<M> {
        children.push_into(&mut self.entries);
        self
    }

    /// Creates the widgets through `ui` and turns the tree into the pure
    /// [`Group`] whose leaf keys index `widgets`, failing with the first
    /// constructor error.
    pub(super) fn realize(
        self,
        ui: &Ui<M>,
        widgets: &mut Vec<Rc<dyn Placeable<M>>>,
    ) -> Result<Group<usize>> {
        let mut group = match self.arrangement {
            Arrangement::Stack(StackDirection::Horizontal) => Group::row(),
            Arrangement::Stack(StackDirection::Vertical) => Group::column(),
            Arrangement::Grid(columns) => Group::grid(columns),
            Arrangement::Wrap => Group::wrap(),
            Arrangement::Layered => Group::layered(),
            Arrangement::Absolute(None) => Group::absolute(),
            Arrangement::Absolute(Some((width, height))) => {
                Group::absolute().design_size(width, height)
            }
        }
        .spacing(self.gap)
        .margins(self.padding)
        .align(self.align)
        .justify(self.justify);
        for entry in self.entries {
            let mut item = match entry.kind {
                Kind::Widget(realize) => {
                    widgets.push(realize(ui)?);
                    Item::leaf(widgets.len() - 1)
                }
                Kind::Nested(nested) => Item::group(nested.realize(ui, widgets)?),
                Kind::Framed(realize, content) => {
                    widgets.push(realize(ui)?);
                    let key = widgets.len() - 1;
                    Item::framed(key, content.realize(ui, widgets)?)
                }
            }
            .sized(entry.sizing)
            .span(entry.span);
            if let Some(align) = entry.align_x {
                item = item.align_x(align);
            }
            if let Some(align) = entry.align_y {
                item = item.align_y(align);
            }
            if entry.size != [None, None] {
                item = item.size(entry.size[0], entry.size[1]);
            }
            if let Some(width) = entry.max_width {
                item = item.max_width(width);
            }
            if let Some(height) = entry.max_height {
                item = item.max_height(height);
            }
            if let Some([x, y, width, height]) = entry.at {
                item = item.at(x, y, width, height);
            }
            if let Some(anchor) = entry.anchor {
                item = item.anchor(anchor);
            }
            group = group.push(item);
        }
        Ok(group)
    }
}

impl<M: 'static> IntoEntry<M> for Layout<M> {
    fn into_entry(self) -> Entry<M> {
        Entry::new(Kind::Nested(self))
    }
}
