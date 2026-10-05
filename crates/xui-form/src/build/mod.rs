#![forbid(unsafe_code)]

//! Building live `xui` widgets from a [`FormDoc`].
//!
//! The host supplies a [`Binder`] that decides what each event becomes, and a
//! [`Factories`] registry that knows how to describe each kind. [`build`]
//! turns the flat node list into one [`absolute`] layout per container, each
//! node an entry at its design rectangle with its anchor, mounts it, and
//! returns a [`LiveForm`] that owns the widgets and reads and writes their
//! properties. The mounted layout re-anchors the widgets whenever the window
//! (or the container the form is built in) is resized.
//!
//! A factory only ever sees the portable [`crate::schema`], the [`BuildCx`]
//! helpers and the `xui_core::arrange` builders, so registering a custom
//! widget needs no change here.

use std::collections::BTreeMap;
use std::rc::Rc;

use xui_core::WidgetId;
use xui_core::app::Ui;
use xui_core::arrange::{Entry, IntoEntry, Layout, LayoutExt, absolute};
use xui_core::backend::BackendError;
use xui_core::layout::{Anchor, Placement};
use xui_core::units::Dip;

use crate::doc::{FormDoc, Node};
use crate::schema::{Catalog, EventSpec, WidgetSpec};
use crate::value::Value;

mod cx;
mod live;

pub use cx::BuildCx;
pub use live::LiveForm;
pub(crate) use live::WidgetProps;

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
    /// The property cannot be written here (read-only, or design-only at
    /// runtime, or runtime-only in the designer).
    #[error("the property is read-only here")]
    ReadOnly,
}

/// How a build failed.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// A node names a kind the catalog does not know.
    #[error("unknown widget kind `{kind}` for node `{node}`")]
    UnknownKind {
        /// The unknown kind.
        kind: String,
        /// The node that used it.
        node: String,
    },
    /// A node names a kind no factory is registered for.
    #[error("no factory registered for kind `{kind}` (node `{node}`)")]
    UnknownFactory {
        /// The kind without a factory.
        kind: String,
        /// The node that used it.
        node: String,
    },
    /// A node names a parent that is missing or is not a container.
    #[error("parent `{parent}` of node `{node}` does not exist or is not a container")]
    UnknownParent {
        /// The node with the bad parent.
        node: String,
        /// The parent name.
        parent: String,
    },
    /// The backend could not create a node.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// Options for [`build_with`].
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

/// A reference to one event a document binds.
#[derive(Clone, Copy, Debug)]
pub struct EventRef<'a> {
    /// The node that raises the event.
    pub node: &'a str,
    /// The event's name.
    pub event: &'a str,
    /// The event's schema.
    pub spec: &'a EventSpec,
}

/// Decides what a document's event bindings become.
///
/// Returning `None` leaves the event unwired. The closure receives the event's
/// arguments as typed [`Value`]s and returns the host's message.
pub trait Binder<M> {
    /// Binds one event, or returns `None` to leave it unwired.
    fn bind(&self, event: EventRef<'_>) -> Option<EventHandler<M>>;
}

/// A live widget created from a [`Node`].
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
    /// The canonical kind this factory builds.
    fn kind(&self) -> &str;

    /// Describes a widget for `node`, wiring its events through `cx`. The
    /// widget itself is created when the form is mounted.
    fn create(&self, cx: &mut BuildCx<'_, M>, node: &Node) -> Created<M>;
}

/// A registry of widget factories, keyed by canonical kind.
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

    /// The factory for the canonical `kind`, if any.
    pub fn get(&self, kind: &str) -> Option<&dyn WidgetFactory<M>> {
        self.factories.get(kind).map(Box::as_ref)
    }

    /// The kinds with a registered factory, in sorted order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }
}

/// Builds every widget in `doc`, wiring events through `binder`.
pub fn build<M: 'static>(
    ui: &Ui<M>,
    doc: &FormDoc,
    catalog: &Catalog,
    factories: &Factories<M>,
    binder: &dyn Binder<M>,
) -> Result<LiveForm<M>, BuildError> {
    build_with(ui, doc, catalog, factories, binder, BuildOptions::default())
}

/// Builds every widget in `doc` with explicit [`BuildOptions`].
pub fn build_with<M: 'static>(
    ui: &Ui<M>,
    doc: &FormDoc,
    catalog: &Catalog,
    factories: &Factories<M>,
    binder: &dyn Binder<M>,
    options: BuildOptions,
) -> Result<LiveForm<M>, BuildError> {
    let mut builder = Builder {
        ui,
        doc,
        catalog: Rc::new(catalog.clone()),
        factories,
        binder,
        options,
        built: Vec::new(),
    };
    let window = (
        window_extent(doc, "width", 320),
        window_extent(doc, "height", 200),
    );
    let root = builder.level(None, window)?;
    // A node never reached from the window has a missing parent, a parent
    // that is not a container, or sits in a parent cycle.
    if let Some(orphan) = doc.nodes.iter().find(|node| !builder.has(&node.name)) {
        return Err(BuildError::UnknownParent {
            node: orphan.name.clone(),
            parent: orphan.parent.clone().unwrap_or_default(),
        });
    }
    let mounted = match options.container {
        Some(container) => ui.mount_in(container, root)?,
        None => ui.mount(root)?,
    };
    Ok(LiveForm::new(
        ui.clone(),
        mounted,
        builder.catalog,
        builder.built,
        doc,
    ))
}

/// One described node: its name, canonical kind and live widget.
pub(crate) type BuiltNode<M> = (String, String, Box<dyn LiveWidget<M>>);

/// The state of one [`build_with`] call.
struct Builder<'a, M: 'static> {
    ui: &'a Ui<M>,
    doc: &'a FormDoc,
    catalog: Rc<Catalog>,
    factories: &'a Factories<M>,
    binder: &'a dyn Binder<M>,
    options: BuildOptions,
    built: Vec<BuiltNode<M>>,
}

impl<M: 'static> Builder<'_, M> {
    /// Whether the node named `name` has been described.
    fn has(&self, name: &str) -> bool {
        self.built.iter().any(|(built, ..)| built == name)
    }

    /// The absolute layout of `parent`'s children (the window's for `None`),
    /// designed at `size`. Only nodes reachable from the window are visited,
    /// so a parent cycle cannot recurse.
    fn level(&mut self, parent: Option<&str>, size: (i64, i64)) -> Result<Layout<M>, BuildError> {
        let mut layout = absolute().design_size(size.0 as f32, size.1 as f32);
        let doc = self.doc;
        for node in doc.nodes.iter().filter(|n| n.parent.as_deref() == parent) {
            if self.has(&node.name) {
                // A duplicate name: validation reports it; the first wins.
                continue;
            }
            let spec = self
                .catalog
                .get(&node.kind)
                .ok_or_else(|| BuildError::UnknownKind {
                    kind: node.kind.clone(),
                    node: node.name.clone(),
                })?
                .clone();
            let factory =
                self.factories
                    .get(&spec.kind)
                    .ok_or_else(|| BuildError::UnknownFactory {
                        kind: spec.kind.clone(),
                        node: node.name.clone(),
                    })?;
            let design = design_rect(node, &spec);
            let placement = Placement::new(
                Dip(design.0 as f32),
                Dip(design.1 as f32),
                Dip(design.2 as f32),
                Dip(design.3 as f32),
                anchor_of(node),
            );
            // The container goes before its children, in document order.
            let index = self.built.len();
            let content = if spec.children.is_none() {
                None
            } else {
                Some(self.level(Some(&node.name), (design.2, design.3))?)
            };
            let mut cx = BuildCx::new(
                self.ui,
                node,
                &spec,
                self.binder,
                self.options.design_mode,
                Rc::clone(&self.catalog),
                placement.clone(),
                content,
            );
            let created = factory.create(&mut cx, node);
            layout = layout.child(created.entry.placement(&placement));
            self.built.insert(
                index,
                (node.name.clone(), spec.kind.clone(), created.widget),
            );
        }
        Ok(layout)
    }
}

/// A window extent property, or `default`.
fn window_extent(doc: &FormDoc, name: &str, default: i64) -> i64 {
    doc.window
        .prop(name)
        .and_then(Value::as_int)
        .unwrap_or(default)
}

/// The design rectangle of a node, in DIPs, filling in the widget's default
/// size for an omitted `width`/`height`.
fn design_rect(node: &Node, spec: &WidgetSpec) -> (i64, i64, i64, i64) {
    let int = |name: &str, default: f32| {
        node.prop(name)
            .and_then(Value::as_int)
            .unwrap_or(default.round() as i64)
    };
    (
        int("left", 0.0),
        int("top", 0.0),
        int("width", spec.default_size.0.value()),
        int("height", spec.default_size.1.value()),
    )
}

/// The anchor a node declares, defaulting to `top_left`.
fn anchor_of(node: &Node) -> Anchor {
    node.prop("anchor")
        .and_then(Value::as_str)
        .and_then(crate::schema::anchor_from_name)
        .unwrap_or(Anchor::TopLeft)
}
