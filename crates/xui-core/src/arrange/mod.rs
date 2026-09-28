#![forbid(unsafe_code)]

//! Declarative layout: nest [`row()`]s and [`column()`]s of widgets, then mount the
//! tree on a window with [`Ui::mount`].
//!
//! The tree owns the widgets it is given, so the app keeps a handle only for
//! the ones it mutates later (share one with an `Rc`). A constructor's
//! `Result` goes straight in: the first failure surfaces from `mount`, so there
//! is no `unwrap` per widget. The mounted layout re-flows on window resize, DPI
//! change and visibility change, using each widget's [`Placeable`] natural size.
//!
//! ```ignore
//! let root = column()
//!     .spacing(dip(8.0))
//!     .margins(Insets::all(dip(16.0)))
//!     .child(Label::auto(ui, "type something"))
//!     .child(&name) // an `Rc<Edit<_>>` the app kept
//!     .child(row().child(Button::auto(ui, "OK")).child(spacer()));
//! let mounted = ui.mount(root)?; // keep this alive
//! ```

mod mount;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use crate::app::Ui;
use crate::backend::{BackendError, Result, WidgetId};
use crate::geometry::Size;
use crate::layout::{Group, Insets, Item, Sizing, StackDirection};
use crate::units::Dip;
use crate::widget::{
    Button, CheckBox, ComboBox, Edit, Hyperlink, Label, MultilineEdit, NumberField, Placeable,
    ProgressBar, Separator, Slider, ToggleButton,
};

pub use mount::Mounted;

/// A widget of no size that soaks up leftover space, for pushing its siblings
/// apart.
struct Spacer;

impl<M: 'static> Placeable<M> for Spacer {
    fn id(&self) -> WidgetId {
        WidgetId::NONE
    }

    fn natural_size(&self, _ui: &Ui<M>, _dpi: u32) -> Size {
        Size::new(0, 0)
    }
}

enum Kind<M: 'static> {
    Widget(Rc<dyn Placeable<M>>),
    Nested(Layout<M>),
}

/// One child of a [`Layout`] with its sizing: a widget, or a nested layout.
///
/// Rarely named: [`IntoEntry`] and [`LayoutExt`] build them.
pub struct Entry<M: 'static> {
    kind: Result<Kind<M>>,
    sizing: Sizing,
}

/// Something a [`Layout`] can hold: a widget, a widget's `Result`, a shared
/// `&Rc<widget>`, a nested [`Layout`], or an already-sized [`Entry`].
pub trait IntoEntry<M: 'static> {
    /// Wraps `self` as a layout entry.
    fn into_entry(self) -> Entry<M>;
}

/// Wraps a custom [`Placeable`] widget as an entry (the built-in widgets are
/// accepted directly).
pub fn widget<M: 'static>(widget: impl Placeable<M> + 'static) -> Entry<M> {
    shared_entry(Rc::new(widget))
}

fn shared_entry<M: 'static>(widget: Rc<dyn Placeable<M>>) -> Entry<M> {
    Entry {
        kind: Ok(Kind::Widget(widget)),
        sizing: Sizing::Auto,
    }
}

/// Implements [`IntoEntry`] for a built-in widget by value, as a constructor
/// `Result`, and as a shared `&Rc`.
macro_rules! entry_for {
    ($($widget:ident),* $(,)?) => {$(
        impl<M: 'static> IntoEntry<M> for $widget<M> {
            fn into_entry(self) -> Entry<M> {
                shared_entry(Rc::new(self))
            }
        }

        impl<M: 'static> IntoEntry<M> for Result<$widget<M>> {
            fn into_entry(self) -> Entry<M> {
                match self {
                    Ok(widget) => widget.into_entry(),
                    Err(error) => Entry {
                        kind: Err(error),
                        sizing: Sizing::Auto,
                    },
                }
            }
        }

        impl<M: 'static> IntoEntry<M> for &Rc<$widget<M>> {
            fn into_entry(self) -> Entry<M> {
                let shared: Rc<$widget<M>> = Rc::clone(self);
                shared_entry(shared)
            }
        }
    )*};
}
entry_for!(
    Button,
    CheckBox,
    ComboBox,
    Edit,
    Hyperlink,
    Label,
    MultilineEdit,
    NumberField,
    ProgressBar,
    Separator,
    Slider,
    ToggleButton,
);

impl<M: 'static> IntoEntry<M> for Layout<M> {
    fn into_entry(self) -> Entry<M> {
        Entry {
            kind: Ok(Kind::Nested(self)),
            sizing: Sizing::Fill(1),
        }
    }
}

impl<M: 'static> IntoEntry<M> for Entry<M> {
    fn into_entry(self) -> Entry<M> {
        self
    }
}

/// Sizing for anything a [`Layout`] can hold: `widget.fill(1)`,
/// `row().fixed(dip(40.0))`.
pub trait LayoutExt<M: 'static>: IntoEntry<M> + Sized {
    /// Sizes the entry with `sizing`.
    fn sized(self, sizing: Sizing) -> Entry<M> {
        let mut entry = self.into_entry();
        entry.sizing = sizing;
        entry
    }

    /// A share of the parent's leftover space, by `weight`.
    fn fill(self, weight: u32) -> Entry<M> {
        self.sized(Sizing::Fill(weight))
    }

    /// Exactly `size` along the parent's main axis.
    fn fixed(self, size: Dip) -> Entry<M> {
        self.sized(Sizing::Fixed(size))
    }

    /// At least `size` along the parent's main axis.
    fn min(self, size: Dip) -> Entry<M> {
        self.sized(Sizing::Min(size))
    }

    /// Exactly `size` wide: the main axis in a row, the cross axis in a column.
    fn width(self, size: Dip) -> Entry<M> {
        self.sized(Sizing::Width(size))
    }

    /// Exactly `size` tall: the main axis in a column, the cross axis in a row.
    fn height(self, size: Dip) -> Entry<M> {
        self.sized(Sizing::Height(size))
    }
}

impl<M: 'static, T: IntoEntry<M>> LayoutExt<M> for T {}

/// An empty, flexible gap: it takes leftover space, pushing its siblings apart.
pub fn spacer<M: 'static>() -> Entry<M> {
    Entry {
        kind: Ok(Kind::Widget(Rc::new(Spacer))),
        sizing: Sizing::Fill(1),
    }
}

/// A row or column of entries, before it is mounted.
pub struct Layout<M: 'static> {
    direction: StackDirection,
    spacing: Dip,
    margins: Insets,
    entries: Vec<Entry<M>>,
}

/// A layout that places its entries left to right.
pub fn row<M: 'static>() -> Layout<M> {
    Layout::new(StackDirection::Horizontal)
}

/// A layout that places its entries top to bottom.
pub fn column<M: 'static>() -> Layout<M> {
    Layout::new(StackDirection::Vertical)
}

impl<M: 'static> Layout<M> {
    fn new(direction: StackDirection) -> Layout<M> {
        Layout {
            direction,
            spacing: Dip(0.0),
            margins: Insets::default(),
            entries: Vec::new(),
        }
    }

    /// The gap between adjacent entries, in design units.
    pub fn spacing(mut self, spacing: Dip) -> Layout<M> {
        self.spacing = spacing;
        self
    }

    /// Margins inside the parent, in design units.
    pub fn margins(mut self, margins: Insets) -> Layout<M> {
        self.margins = margins;
        self
    }

    /// Appends a widget, a nested layout or an entry. The layout takes
    /// ownership: the widgets live as long as the layout is mounted.
    pub fn child(mut self, entry: impl IntoEntry<M>) -> Layout<M> {
        self.entries.push(entry.into_entry());
        self
    }

    /// Turns the tree into the pure [`Group`] and the widgets its leaf keys
    /// index, failing with the first constructor error.
    fn flatten(
        self,
        widgets: &mut Vec<Rc<dyn Placeable<M>>>,
    ) -> std::result::Result<Group<usize>, BackendError> {
        let mut group = match self.direction {
            StackDirection::Horizontal => Group::row(),
            StackDirection::Vertical => Group::column(),
        }
        .spacing(self.spacing)
        .margins(self.margins);
        for entry in self.entries {
            let item = match entry.kind? {
                Kind::Widget(widget) => {
                    widgets.push(widget);
                    Item::leaf(widgets.len() - 1)
                }
                Kind::Nested(nested) => Item::group(nested.flatten(widgets)?),
            };
            group = group.push(item.sized(entry.sizing));
        }
        Ok(group)
    }
}
