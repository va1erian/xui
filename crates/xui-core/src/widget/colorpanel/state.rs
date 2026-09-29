#![forbid(unsafe_code)]

//! Shared state and view-syncing helpers for [`ColorPanel`](super::ColorPanel).
//!
//! The panel, its child views and the closures they register all read and write
//! one `Shared` value: a single HSV triple, the hue cell and the editable boxes.
//! Keeping it here holds `mod.rs` to the widget's own surface.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::model::Hsv;
use super::text::Field;
use crate::Color;
use crate::app::Ui;
use crate::backend::WidgetId;
use crate::widget::{Edit, HasText};

/// A replaceable colour-to-message mapper.
type ColorMapper<M> = RefCell<Option<Rc<dyn Fn(Color) -> Option<M>>>>;

/// State shared between the panel, its child views and their event closures.
pub(crate) struct Shared<M: 'static> {
    pub(crate) hsv: Rc<Cell<Hsv>>,
    pub(crate) hue: Rc<Cell<f32>>,
    pub(crate) preview: Cell<WidgetId>,
    pub(crate) field: Cell<WidgetId>,
    pub(crate) hue_node: Cell<WidgetId>,
    pub(crate) texts: RefCell<Vec<(Field, Weak<Edit<M>>)>>,
    pub(crate) on_change: ColorMapper<M>,
    pub(crate) on_commit: ColorMapper<M>,
}

impl<M: 'static> Shared<M> {
    /// The edit widget for `field`, if it is still alive.
    pub(crate) fn edit(&self, field: Field) -> Option<Rc<Edit<M>>> {
        self.texts
            .borrow()
            .iter()
            .find(|(kind, _)| *kind == field)
            .and_then(|(_, edit)| edit.upgrade())
    }

    /// Raises the `on_change` message, if any.
    pub(crate) fn change(&self, color: Color) -> Option<M> {
        let mapper = self.on_change.borrow().clone();
        mapper.and_then(|mapper| mapper(color))
    }

    /// Raises the `on_commit` message, if any.
    pub(crate) fn commit(&self, color: Color) -> Option<M> {
        let mapper = self.on_commit.borrow().clone();
        mapper.and_then(|mapper| mapper(color))
    }
}

/// Releases a drag on the field or hue slider, if one is in progress.
pub(crate) fn end_drags<M: 'static>(ui: &Ui<M>, field: &Rc<Cell<bool>>, hue: &Rc<Cell<bool>>) {
    let dragging = field.replace(false) | hue.replace(false);
    if dragging {
        ui.release_capture();
    }
}

/// Applies a hue change from the hue slider, updating the other views.
pub(crate) fn set_hue<M: 'static>(
    ui: &Ui<M>,
    shared: &Shared<M>,
    degrees: f32,
    commit: bool,
) -> Option<M> {
    let old = shared.hsv.get();
    let triple = Hsv::new(degrees, old.s, old.v);
    shared.hsv.set(triple);
    sync(ui, shared, None);
    let color = triple.to_color();
    if commit {
        shared.commit(color)
    } else {
        shared.change(color)
    }
}

/// Refreshes every view from the shared triple, skipping a box mid-edit.
pub(crate) fn sync<M: 'static>(ui: &Ui<M>, shared: &Shared<M>, skip: Option<Field>) {
    let triple = shared.hsv.get();
    shared.hue.set(triple.h);
    ui.invalidate(shared.hue_node.get());
    ui.invalidate(shared.field.get());
    ui.invalidate(shared.preview.get());
    // Clone the targets out before setting any text: on a native field the set
    // can re-enter and borrow `texts` again.
    let targets: Vec<(Field, Weak<Edit<M>>)> = shared
        .texts
        .borrow()
        .iter()
        .map(|(kind, edit)| (*kind, Weak::clone(edit)))
        .collect();
    for (kind, edit) in targets {
        if Some(kind) == skip {
            continue;
        }
        if let Some(edit) = edit.upgrade() {
            edit.set_text(&kind.format(triple));
        }
    }
}
