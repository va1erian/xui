#![forbid(unsafe_code)]

//! [`Build`]: a widget described now and created when its layout is mounted,
//! and [`Handle`]: the app's typed reference to it afterwards.

use std::cell::RefCell;
use std::rc::Rc;

use super::{Entry, IntoEntry, Kind, Realize};
use crate::app::Ui;
use crate::backend::Result;
use crate::widget::Placeable;

/// A typed reference to a widget a layout created, for the app to change it
/// later: `self.status.get().set_text(0, "Saved")`.
///
/// Make one with [`Handle::new`], give it to a builder with
/// [`Build::bind`], and keep it in the app. It is filled when the layout is
/// mounted; the layout and the handle share the widget, so it lives while
/// either does.
pub struct Handle<W>(Rc<RefCell<Option<Rc<W>>>>);

impl<W> Handle<W> {
    /// An empty handle, filled by the mount of a builder bound to it.
    pub fn new() -> Handle<W> {
        Handle(Rc::new(RefCell::new(None)))
    }

    /// The widget.
    ///
    /// # Panics
    ///
    /// When the builder bound to this handle has not been mounted: a handle
    /// is only read from `update`, after the layout is up.
    pub fn get(&self) -> Rc<W> {
        self.try_get()
            .expect("Handle::get before its layout was mounted")
    }

    /// The widget, or `None` before the builder bound to this handle is
    /// mounted.
    pub fn try_get(&self) -> Option<Rc<W>> {
        self.0.borrow().clone()
    }

    fn set(&self, widget: Rc<W>) {
        *self.0.borrow_mut() = Some(widget);
    }
}

impl<W> Default for Handle<W> {
    fn default() -> Handle<W> {
        Handle::new()
    }
}

impl<W> Clone for Handle<W> {
    fn clone(&self) -> Handle<W> {
        Handle(Rc::clone(&self.0))
    }
}

/// A deferred widget constructor.
type Make<W, M> = Box<dyn FnOnce(&Ui<M>) -> Result<W>>;

/// A widget described now and created, through the [`Ui`] of the container
/// it is mounted in, when its layout is mounted.
///
/// The functions in [`arrange`](super) ([`label`](super::label),
/// [`button`](super::button), …) return one, with each widget's options and
/// event mappings as methods; [`build`] wraps any other widget's constructor.
pub struct Build<W: 'static, M: 'static> {
    make: Make<W, M>,
    handle: Option<Handle<W>>,
}

/// A builder for any widget: `build(|ui| MyPane::new(ui))`. The constructor
/// runs at mount time, with the [`Ui`] of the container the layout is mounted
/// in.
pub fn build<W: 'static, M: 'static>(
    make: impl FnOnce(&Ui<M>) -> Result<W> + 'static,
) -> Build<W, M> {
    Build {
        make: Box::new(make),
        handle: None,
    }
}

impl<W: 'static, M: 'static> Build<W, M> {
    /// Fills `handle` with the widget when it is created.
    pub fn bind(mut self, handle: &Handle<W>) -> Build<W, M> {
        self.handle = Some(handle.clone());
        self
    }

    /// Applies `f` to the widget once it is created, for an option that has
    /// no builder method: `.then(|list| list.multi_select(true))`.
    pub fn then(self, f: impl FnOnce(W) -> W + 'static) -> Build<W, M> {
        let make = self.make;
        Build {
            make: Box::new(move |ui| make(ui).map(f)),
            handle: self.handle,
        }
    }

    /// Like [`then`](Self::then), for a step that needs the container's
    /// [`Ui`] or can fail.
    pub fn then_with(self, f: impl FnOnce(W, &Ui<M>) -> Result<W> + 'static) -> Build<W, M> {
        let make = self.make;
        Build {
            make: Box::new(move |ui| f(make(ui)?, ui)),
            handle: self.handle,
        }
    }

    /// Creates the widget through `ui`, filling any bound handle.
    pub(super) fn create(self, ui: &Ui<M>) -> Result<Rc<W>> {
        let widget = Rc::new((self.make)(ui)?);
        if let Some(handle) = self.handle {
            handle.set(Rc::clone(&widget));
        }
        Ok(widget)
    }
}

impl<W: Placeable<M> + 'static, M: 'static> Build<W, M> {
    /// The deferred constructor a layout entry holds.
    pub(super) fn realizer(self) -> Realize<M> {
        Box::new(move |ui| {
            let widget: Rc<dyn Placeable<M>> = self.create(ui)?;
            Ok(widget)
        })
    }
}

impl<W: Placeable<M> + 'static, M: 'static> IntoEntry<M> for Build<W, M> {
    fn into_entry(self) -> Entry<M> {
        Entry::new(Kind::Widget(self.realizer()))
    }
}
