#![forbid(unsafe_code)]

//! Declarative layout: describe a window as nested [`row()`]s, [`column()`]s
//! and [`grid()`]s of widget builders, then mount the tree with [`Ui::root`].
//!
//! A builder ([`label`], [`button`], …) describes a widget; it is created when
//! the layout is mounted, parented to the container the layout is mounted in,
//! so there is no `ui`, no `Rect` and no `unwrap` per widget. The first
//! constructor error surfaces from the mount. A widget the app changes later
//! is [`bind`](Build::bind)ed to a [`Handle`]. The mounted layout re-flows on
//! window resize, DPI change and visibility change, measuring each widget
//! through [`Placeable`].
//!
//! ```ignore
//! let name = Handle::new();
//! ui.root(
//!     column().padding(16).gap(8).children((
//!         label("Your name"),
//!         row().gap(8).children((
//!             edit().bind(&name).fill(1),
//!             button("Greet").on_click(Msg::Greet),
//!         )),
//!     )),
//! )?;
//! ```

mod build;
mod children;
mod containers;
mod mount;
#[cfg(test)]
mod tests;
mod widgets;

use std::rc::Rc;

use crate::app::Ui;
use crate::backend::{Result, WidgetId};
use crate::geometry::Size;
use crate::layout::{Constraints, Group, Insets, Item, Sizing, StackDirection};
use crate::units::Dip;
use crate::widget::Placeable;

pub use crate::layout::{Align, Track};
pub use build::{Build, Handle, build};
pub use children::IntoChildren;
pub use containers::{TabsBuild, group, tabs};
pub use mount::Mounted;
pub use widgets::{
    button, checkbox, combo_box, edit, hyperlink, label, list, multiline_edit, number_field,
    progress, separator, slider, status_bar, toggle_button,
};

/// A widget of no size that soaks up leftover space, for pushing its siblings
/// apart.
struct Spacer;

impl<M: 'static> Placeable<M> for Spacer {
    fn id(&self) -> WidgetId {
        WidgetId::NONE
    }

    fn measure(&self, _ui: &Ui<M>, _constraints: Constraints) -> Size {
        Size::new(0, 0)
    }
}

/// Creates a widget once the layout knows its parent.
type Realize<M> = Box<dyn FnOnce(&Ui<M>) -> Result<Rc<dyn Placeable<M>>>>;

enum Kind<M: 'static> {
    Widget(Realize<M>),
    Nested(Layout<M>),
    /// A widget with a layout placed inside its content insets.
    Framed(Realize<M>, Layout<M>),
}

/// One child of a [`Layout`] with its sizing and placement: a widget, a nested
/// layout, or a frame with a layout inside it.
///
/// Rarely named: [`IntoEntry`] and [`LayoutExt`] build them.
pub struct Entry<M: 'static> {
    kind: Kind<M>,
    sizing: Sizing,
    align: Option<Align>,
    max_width: Option<Dip>,
    max_height: Option<Dip>,
    span: usize,
}

impl<M: 'static> Entry<M> {
    fn new(kind: Kind<M>) -> Entry<M> {
        Entry {
            kind,
            sizing: Sizing::Auto,
            align: None,
            max_width: None,
            max_height: None,
            span: 1,
        }
    }
}

/// Something a [`Layout`] can hold: a widget builder, a nested [`Layout`], or
/// an already-sized [`Entry`].
pub trait IntoEntry<M: 'static> {
    /// Wraps `self` as a layout entry.
    fn into_entry(self) -> Entry<M>;
}

impl<M: 'static> IntoEntry<M> for Layout<M> {
    fn into_entry(self) -> Entry<M> {
        Entry::new(Kind::Nested(self))
    }
}

impl<M: 'static> IntoEntry<M> for Entry<M> {
    fn into_entry(self) -> Entry<M> {
        self
    }
}

/// Sizing and placement for anything a [`Layout`] can hold:
/// `edit().fill(1)`, `row().fixed(40)`, `label("x").align(Align::End)`.
pub trait LayoutExt<M: 'static>: IntoEntry<M> + Sized {
    /// Sizes the entry with `sizing`.
    fn sized(self, sizing: Sizing) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.sizing = sizing;
        entry
    }

    /// A share of the parent's leftover space, by `weight`. In a grid, makes
    /// the entry's row fill.
    fn fill(self, weight: u32) -> Entry<M> {
        self.sized(Sizing::Fill(weight))
    }

    /// Exactly `size` along the parent's main axis.
    fn fixed(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Fixed(size.into()))
    }

    /// At least `size` along the parent's main axis.
    fn min(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Min(size.into()))
    }

    /// Exactly `size` wide: the main axis in a row, the cross axis in a column.
    fn width(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Width(size.into()))
    }

    /// Exactly `size` tall: the main axis in a column, the cross axis in a row.
    fn height(self, size: impl Into<Dip>) -> Entry<M> {
        self.sized(Sizing::Height(size.into()))
    }

    /// Where the entry sits across its parent's main axis (in a grid, within
    /// its cell), instead of the parent's [`Layout::align`].
    fn align(self, align: Align) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.align = Some(align);
        entry
    }

    /// Caps the entry's width at `width` design units.
    fn max_width(self, width: impl Into<Dip>) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.max_width = Some(width.into());
        entry
    }

    /// Caps the entry's height at `height` design units.
    fn max_height(self, height: impl Into<Dip>) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.max_height = Some(height.into());
        entry
    }

    /// In a grid, covers `columns` columns.
    fn span(self, columns: usize) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.span = columns.max(1);
        entry
    }
}

impl<M: 'static, T: IntoEntry<M>> LayoutExt<M> for T {}

/// An empty, flexible gap: it takes leftover space, pushing its siblings apart.
pub fn spacer<M: 'static>() -> Entry<M> {
    let realize: Realize<M> = Box::new(|_| Ok(Rc::new(Spacer)));
    Entry::new(Kind::Widget(realize)).fill(1)
}

#[derive(Clone)]
enum Arrangement {
    Stack(StackDirection),
    Grid(Vec<Track>),
}

/// A row, column or grid of entries, before it is mounted.
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

    /// The gap between adjacent entries (in a grid, between rows and between
    /// columns), in design units.
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

    /// Where entries sit across the main axis (in a grid, within their cells)
    /// unless they set their own. The default stretches them.
    pub fn align(mut self, align: Align) -> Layout<M> {
        self.align = align;
        self
    }

    /// Where a row's or column's entries sit along the main axis when none of
    /// them fills it. The default packs them at the start.
    pub fn justify(mut self, justify: Align) -> Layout<M> {
        self.justify = justify;
        self
    }

    /// Appends a widget builder, a nested layout or an entry.
    pub fn child(mut self, entry: impl IntoEntry<M>) -> Layout<M> {
        self.entries.push(entry.into_entry());
        self
    }

    /// Appends several children at once: a tuple of builders and layouts, or
    /// a `Vec` of entries.
    pub fn children(mut self, children: impl IntoChildren<M>) -> Layout<M> {
        children.push_into(&mut self.entries);
        self
    }

    /// Creates the widgets through `ui` and turns the tree into the pure
    /// [`Group`] whose leaf keys index `widgets`, failing with the first
    /// constructor error.
    fn realize(self, ui: &Ui<M>, widgets: &mut Vec<Rc<dyn Placeable<M>>>) -> Result<Group<usize>> {
        let mut group = match self.arrangement {
            Arrangement::Stack(StackDirection::Horizontal) => Group::row(),
            Arrangement::Stack(StackDirection::Vertical) => Group::column(),
            Arrangement::Grid(columns) => Group::grid(columns),
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
            if let Some(align) = entry.align {
                item = item.align(align);
            }
            if let Some(width) = entry.max_width {
                item = item.max_width(width);
            }
            if let Some(height) = entry.max_height {
                item = item.max_height(height);
            }
            group = group.push(item);
        }
        Ok(group)
    }
}
