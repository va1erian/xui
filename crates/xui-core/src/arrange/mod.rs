#![forbid(unsafe_code)]

//! Declarative layout: describe a window as nested [`row()`]s, [`column()`]s,
//! [`grid()`]s, [`wrap()`]s, [`overlay()`]s and [`scroll()`]s of widget
//! builders, then mount the tree with [`Ui::root`]. [`absolute()`] is the one
//! container for free positions: a designer surface, forms imported from
//! coordinates.
//!
//! A builder ([`label`], [`button`], …) describes a widget; it is created when
//! the layout is mounted, parented to the container the layout is mounted in,
//! so there is no `ui`, no `Rect` and no `unwrap` per widget. The first
//! constructor error surfaces from the mount. A widget the app changes later
//! is [`bind`](Build::bind)ed to a [`Handle`]. The mounted layout re-flows on
//! window resize and DPI change at once, and after any change that can alter
//! a widget's natural size (its text, its visibility, the theme) once the
//! event being handled is done, measuring each widget through [`Placeable`].
//! [`Ui::layout_report`] describes what it placed, with warnings.
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

mod bars;
mod build;
mod children;
mod containers;
mod ext;
mod layout;
mod mount;
mod report;
#[cfg(test)]
mod tests;
mod views;
mod widgets;

use std::rc::Rc;

use crate::app::Ui;
use crate::backend::{Result, WidgetId};
use crate::geometry::Size;
use crate::layout::{Constraints, Sizing};
use crate::units::Dip;
use crate::widget::Placeable;

pub use crate::layout::{Align, Anchor, Track};
pub use bars::{flow_text, grid_view, icon_view, material_status_bar, menu_bar, toolbar, top_bar};
pub use build::{Build, Handle, build};
pub use children::IntoChildren;
pub use containers::{GroupBuild, ScrollBuild, SplitBuild, TabsBuild, group, scroll, split, tabs};
pub use ext::LayoutExt;
pub use layout::{Layout, absolute, column, grid, overlay, row, stack, wrap};
pub use mount::Mounted;
pub use views::{
    PanelBuild, color_field, color_panel, color_picker, hue_slider, panel, radio_group, tree_view,
};
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
    align_x: Option<Align>,
    align_y: Option<Align>,
    size: [Option<Dip>; 2],
    max_width: Option<Dip>,
    max_height: Option<Dip>,
    span: usize,
    at: Option<[Dip; 4]>,
    anchor: Option<Anchor>,
}

impl<M: 'static> Entry<M> {
    fn new(kind: Kind<M>) -> Entry<M> {
        Entry {
            kind,
            sizing: Sizing::Auto,
            align_x: None,
            align_y: None,
            size: [None, None],
            max_width: None,
            max_height: None,
            span: 1,
            at: None,
            anchor: None,
        }
    }
}

/// Something a [`Layout`] can hold: a widget builder, a nested [`Layout`], or
/// an already-sized [`Entry`].
pub trait IntoEntry<M: 'static> {
    /// Wraps `self` as a layout entry.
    fn into_entry(self) -> Entry<M>;
}

impl<M: 'static> IntoEntry<M> for Entry<M> {
    fn into_entry(self) -> Entry<M> {
        self
    }
}

/// An empty, flexible gap: it takes leftover space, pushing its siblings apart.
pub fn spacer<M: 'static>() -> Entry<M> {
    let realize: Realize<M> = Box::new(|_| Ok(Rc::new(Spacer)));
    Entry::new(Kind::Widget(realize)).fill(1)
}
