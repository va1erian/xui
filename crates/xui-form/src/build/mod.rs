#![forbid(unsafe_code)]

//! Building live `xui` widgets from a [`Form`].
//!
//! [`describe`] turns the form's tree into the same `xui_core::arrange`
//! builders a Rust app writes by hand (a `Row` becomes [`row()`], a `Button`
//! becomes [`button()`], …), wiring events through a host-supplied
//! [`Binder`]: the [`Layout`] can be mounted alone or inside a larger one.
//! [`build`] describes and mounts it as the window's content and returns a
//! [`LiveForm`] that reads and writes the widgets' properties by name.
//!
//! The [`Factories`] registry knows how to describe each widget kind; a
//! factory only sees the [`BuildCx`] helpers and the `arrange` builders.
//!
//! [`row()`]: xui_core::arrange::row
//! [`button()`]: xui_core::arrange::button

use std::collections::BTreeMap;
use std::rc::Rc;

use xui_core::WidgetId;
use xui_core::app::Ui;
use xui_core::arrange::{Entry, IntoEntry, Layout};
use xui_core::backend::BackendError;

use crate::model::Form;
use crate::schema::{Catalog, EventSpec};
use crate::value::Value;

mod cx;
mod handlers;
mod live;
mod tree;

pub use cx::BuildCx;
pub use handlers::Handlers;
pub(crate) use live::WidgetProps;
pub use live::{Controls, LiveForm};

/// How a property write failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SetError {
    /// No widget of that name exists in the form.
    #[error("unknown widget")]
    UnknownWidget,
    /// No property of that name exists.
    #[error("unknown property")]
    UnknownProperty,
    /// The property exists but the value has the wrong type.
    #[error("the value has the wrong type")]
    TypeMismatch,
    /// The property cannot be written here (design-only at runtime, or a
    /// geometry property of a widget outside an `Absolute` layout).
    #[error("the property is read-only here")]
    ReadOnly,
}

/// How a build failed.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// A node names a kind no factory is registered for.
    #[error("no factory registered for kind `{kind}` (node `{node}`)")]
    UnknownFactory {
        /// The kind without a factory.
        kind: String,
        /// The node that used it.
        node: String,
    },
    /// The backend could not create a widget.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// Options for [`build_with`] and [`describe`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// Whether the form is being built for the designer. In design mode no
    /// binder is consulted, and design-only properties may be written.
    pub design_mode: bool,
    /// The container node to build the form inside (a designer's preview
    /// panel), following its size; `None` builds it as the window's content.
    pub container: Option<WidgetId>,
}

/// A host event handler: it receives the event's typed arguments and returns
/// the host's message, or `None` to ignore the event.
pub type EventHandler<M> = Rc<dyn Fn(&[Value]) -> Option<M>>;

/// A reference to one event a form binds.
#[derive(Clone, Copy, Debug)]
pub struct EventRef<'a> {
    /// The name of the widget that raises the event; for a control array,
    /// the array's name.
    pub node: &'a str,
    /// For an element of a control array, its index.
    pub index: Option<usize>,
    /// The event's name.
    pub event: &'a str,
    /// The event's schema.
    pub spec: &'a EventSpec,
}

/// Decides what a form's events become.
///
/// Returning `None` leaves the event unwired. The closure receives the event's
/// arguments as typed [`Value`]s and returns the host's message.
pub trait Binder<M> {
    /// Binds one event, or returns `None` to leave it unwired.
    fn bind(&self, event: EventRef<'_>) -> Option<EventHandler<M>>;
}

/// A live widget created from a widget node.
///
/// Its methods are called once the form is mounted, which is when the
/// widget exists.
pub trait LiveWidget<M: 'static> {
    /// The widget's node identity.
    fn id(&self) -> WidgetId;

    /// The property named `prop`, common or widget-specific, if it has one.
    fn get(&self, prop: &str) -> Option<Value>;

    /// Sets the property named `prop`.
    fn set(&self, prop: &str, value: &Value) -> Result<(), SetError>;

    /// Every node the widget owns; the first is [`LiveWidget::id`]. A
    /// `RadioGroup` owns one per option.
    fn node_ids(&self) -> Vec<WidgetId> {
        vec![self.id()]
    }

    /// Applies, once the form is mounted, the state the widget's builder
    /// could not set before it existed. The default does nothing.
    fn ready(&self) {}
}

/// What a factory makes of a node: the layout entry that creates the widget
/// when the form is mounted, and the live widget that reads and writes it
/// afterwards.
pub struct Created<M: 'static> {
    entry: Entry<M>,
    widget: Box<dyn LiveWidget<M>>,
}

impl<M: 'static> Created<M> {
    /// Pairs a builder (anything a layout holds) with its live widget.
    pub fn new(entry: impl IntoEntry<M>, widget: impl LiveWidget<M> + 'static) -> Created<M> {
        Created {
            entry: entry.into_entry(),
            widget: Box::new(widget),
        }
    }
}

/// Describes the live widget for one kind.
pub trait WidgetFactory<M: 'static> {
    /// The kind this factory builds, as written in a file.
    fn kind(&self) -> &str;

    /// Describes the widget `cx` is about, wiring its events through `cx`.
    /// The widget itself is created when the form is mounted.
    fn create(&self, cx: &mut BuildCx<'_, M>) -> Created<M>;
}

/// A registry of widget factories, keyed by kind.
pub struct Factories<M: 'static> {
    factories: BTreeMap<String, Box<dyn WidgetFactory<M>>>,
}

impl<M: 'static> Default for Factories<M> {
    fn default() -> Self {
        Factories::new()
    }
}

impl<M: 'static> Factories<M> {
    /// An empty registry.
    pub fn new() -> Self {
        Factories {
            factories: BTreeMap::new(),
        }
    }

    /// Registers a factory, replacing any with the same kind.
    pub fn register(&mut self, factory: impl WidgetFactory<M> + 'static) {
        self.factories
            .insert(factory.kind().to_owned(), Box::new(factory));
    }

    /// The factory for `kind`, if any.
    pub fn get(&self, kind: &str) -> Option<&dyn WidgetFactory<M>> {
        self.factories.get(kind).map(Box::as_ref)
    }

    /// The kinds with a registered factory, in sorted order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }
}

/// A form described as builders, before it is mounted.
pub struct Described<M: 'static> {
    /// The form's root as a layout, to mount alone or inside another one.
    pub layout: Layout<M>,
    /// The form's named widgets, readable and writable once `layout` is
    /// mounted.
    pub controls: Controls<M>,
}

/// Describes `form` as the `arrange` builders Rust code uses, wiring events
/// through `binder`.
pub fn describe<M: 'static>(
    ui: &Ui<M>,
    form: &Form,
    factories: &Factories<M>,
    binder: &dyn Binder<M>,
    options: BuildOptions,
) -> Result<Described<M>, BuildError> {
    let catalog = Rc::new(Catalog::xui());
    let mut walk = tree::Walk::new(ui, Rc::clone(&catalog), factories, binder, options);
    let layout = walk.content(&form.root)?;
    Ok(Described {
        layout,
        controls: Controls::new(ui.clone(), catalog, walk.into_widgets()),
    })
}

/// Builds every widget in `form` as the window's content, wiring events
/// through `binder`.
pub fn build<M: 'static>(
    ui: &Ui<M>,
    form: &Form,
    factories: &Factories<M>,
    binder: &dyn Binder<M>,
) -> Result<LiveForm<M>, BuildError> {
    build_with(ui, form, factories, binder, BuildOptions::default())
}

/// Builds every widget in `form` with explicit [`BuildOptions`].
pub fn build_with<M: 'static>(
    ui: &Ui<M>,
    form: &Form,
    factories: &Factories<M>,
    binder: &dyn Binder<M>,
    options: BuildOptions,
) -> Result<LiveForm<M>, BuildError> {
    let described = describe(ui, form, factories, binder, options)?;
    let mounted = match options.container {
        Some(container) => ui.mount_in(container, described.layout)?,
        None => ui.mount(described.layout)?,
    };
    Ok(LiveForm::new(described.controls, mounted))
}
