#![forbid(unsafe_code)]

//! [`LiveForm`] and the wrapper that gives every built-in widget the common
//! properties.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use xui_core::WidgetId;
use xui_core::app::Ui;
use xui_core::arrange::Mounted;
use xui_core::geometry::Rect;
use xui_core::layout::Placement;
use xui_core::units::Dip;

use super::{BuiltNode, LiveWidget, SetError};
use crate::doc::FormDoc;
use crate::schema::{Access, Catalog, anchor_from_name, anchor_name};
use crate::value::{Value, ValueType};

/// The widget-specific property surface a built-in widget implements.
///
/// The [`Live`] wrapper adds the common properties (geometry, `anchor`,
/// `visible`, `enabled`, `tab_index`) and the schema's access and type rules.
pub(crate) trait WidgetProps<M: 'static>: 'static {
    /// The widget's node identity.
    fn id(&self) -> WidgetId;
    /// Reads a widget-specific property.
    fn get_own(&self, prop: &str) -> Option<Value>;
    /// Writes a widget-specific property.
    fn set_own(&self, prop: &str, value: &Value) -> Result<(), SetError>;
    /// Tells a widget with its own enabled state (it dims itself) that the
    /// `enabled` common property changed. The default does nothing.
    fn set_enabled_hint(&self, _enabled: bool) {}
    /// Every node the widget owns; the first is [`WidgetProps::id`].
    fn node_ids(&self) -> Vec<WidgetId> {
        vec![self.id()]
    }
}

/// The common property state every built-in widget shares.
pub(super) struct Common<M: 'static> {
    ui: Ui<M>,
    placement: Placement,
    visible: Cell<bool>,
    enabled: Cell<bool>,
    tab_index: Cell<i64>,
}

impl<M: 'static> Common<M> {
    /// The common state of a widget placed by `placement`.
    pub(super) fn new(ui: Ui<M>, placement: Placement) -> Common<M> {
        Common {
            ui,
            placement,
            visible: Cell::new(true),
            enabled: Cell::new(true),
            tab_index: Cell::new(0),
        }
    }

    /// Reads a common property.
    fn get(&self, prop: &str) -> Option<Value> {
        let rect = self.placement.rect();
        let int = |dip: Dip| Value::Int(dip.value().round() as i64);
        Some(match prop {
            "left" => int(rect[0]),
            "top" => int(rect[1]),
            "width" => int(rect[2]),
            "height" => int(rect[3]),
            "anchor" => Value::Enum(anchor_name(self.placement.anchor()).to_owned()),
            "visible" => Value::Bool(self.visible.get()),
            "enabled" => Value::Bool(self.enabled.get()),
            "tab_index" => Value::Int(self.tab_index.get()),
            _ => return None,
        })
    }

    /// Records a common property already checked against the schema;
    /// `false` when `prop` is not a common property.
    fn record(&self, prop: &str, value: &Value) -> bool {
        let geometry = ["left", "top", "width", "height"];
        match (prop, value) {
            (_, Value::Int(value)) if geometry.contains(&prop) => {
                let mut rect = self.placement.rect();
                let slot = geometry.iter().position(|name| *name == prop);
                rect[slot.unwrap_or_default()] = Dip(*value as f32);
                self.placement.set_rect(rect);
            }
            ("anchor", Value::Enum(name)) => {
                if let Some(anchor) = anchor_from_name(name) {
                    self.placement.set_anchor(anchor);
                }
            }
            ("visible", Value::Bool(visible)) => self.visible.set(*visible),
            ("enabled", Value::Bool(enabled)) => self.enabled.set(*enabled),
            ("tab_index", Value::Int(index)) => self.tab_index.set(*index),
            _ => return false,
        }
        true
    }
}

/// A [`LiveWidget`] that adds the common properties and the schema's access
/// and type rules to a widget's own surface.
pub(super) struct Live<M: 'static, W: WidgetProps<M>> {
    pub(super) common: Common<M>,
    pub(super) inner: W,
    pub(super) catalog: Rc<Catalog>,
    pub(super) kind: String,
    pub(super) design_mode: bool,
}

impl<M: 'static, W: WidgetProps<M>> LiveWidget<M> for Live<M, W> {
    fn id(&self) -> WidgetId {
        self.inner.id()
    }

    fn get(&self, prop: &str) -> Option<Value> {
        self.common.get(prop).or_else(|| self.inner.get_own(prop))
    }

    fn set(&self, prop: &str, value: &Value) -> Result<(), SetError> {
        let Some(spec) = self.catalog.property(&self.kind, prop) else {
            return Err(SetError::UnknownProperty);
        };
        let writable = if self.design_mode {
            matches!(spec.access, Access::ReadWrite | Access::DesignOnly)
        } else {
            matches!(spec.access, Access::ReadWrite | Access::RuntimeOnly)
        };
        if !writable {
            return Err(SetError::ReadOnly);
        }
        if !spec.accepts(value) {
            return Err(SetError::TypeMismatch);
        }
        if self.common.record(prop, value) {
            self.apply_common(prop, value);
            return Ok(());
        }
        self.inner.set_own(prop, value)
    }

    fn node_ids(&self) -> Vec<WidgetId> {
        self.inner.node_ids()
    }
}

impl<M: 'static, W: WidgetProps<M>> Live<M, W> {
    /// Applies a common property that was just recorded to every node the
    /// widget owns, so a multi-node widget moves, hides and disables as one.
    fn apply_common(&self, prop: &str, value: &Value) {
        let ui = &self.common.ui;
        match (prop, value) {
            ("visible", Value::Bool(visible)) => {
                for id in self.inner.node_ids() {
                    ui.set_visible(id, *visible);
                }
            }
            ("enabled", Value::Bool(enabled)) => {
                for id in self.inner.node_ids() {
                    ui.set_enabled(id, *enabled);
                }
                self.inner.set_enabled_hint(*enabled);
            }
            ("tab_index", _) => {}
            // Geometry and anchor live in the placement the layout reads.
            _ => ui.relayout(),
        }
    }
}

/// One node's identity, kept for lookups by name.
struct NodeMeta {
    name: String,
    kind: String,
}

/// A built form: every live widget, mounted in an absolute layout that keeps
/// them anchored. Dropping it destroys the widgets.
pub struct LiveForm<M: 'static> {
    ui: Ui<M>,
    mounted: Mounted<M>,
    catalog: Rc<Catalog>,
    widgets: Vec<Box<dyn LiveWidget<M>>>,
    by_name: BTreeMap<String, usize>,
    nodes: Vec<NodeMeta>,
}

impl<M: 'static> LiveForm<M> {
    /// The form over the widgets `built` mounted as `mounted`, with the
    /// document's common properties applied.
    pub(super) fn new(
        ui: Ui<M>,
        mounted: Mounted<M>,
        catalog: Rc<Catalog>,
        built: Vec<BuiltNode<M>>,
        doc: &FormDoc,
    ) -> LiveForm<M> {
        let mut form = LiveForm {
            ui,
            mounted,
            catalog,
            widgets: Vec::with_capacity(built.len()),
            by_name: BTreeMap::new(),
            nodes: Vec::with_capacity(built.len()),
        };
        for (name, kind, widget) in built {
            form.by_name.insert(name.clone(), form.widgets.len());
            form.widgets.push(widget);
            form.nodes.push(NodeMeta { name, kind });
        }
        // Geometry and anchors are already in the placements; the rest of
        // the common state needs the widgets, which exist now.
        for node in &doc.nodes {
            for prop in ["visible", "enabled", "tab_index"] {
                if let (Some(value), Some(widget)) = (node.prop(prop), form.widget(&node.name)) {
                    let _ = widget.set(prop, value);
                }
            }
        }
        form
    }

    /// The widget named `name`, if any.
    pub fn widget(&self, name: &str) -> Option<&dyn LiveWidget<M>> {
        self.by_name
            .get(name)
            .and_then(|index| self.widgets.get(*index))
            .map(Box::as_ref)
    }

    /// The value of `prop` on the widget named `name`, if any.
    pub fn get(&self, name: &str, prop: &str) -> Option<Value> {
        self.widget(name)?.get(prop)
    }

    /// Sets `prop` on the widget named `name`.
    pub fn set(&self, name: &str, prop: &str, value: &Value) -> Result<(), SetError> {
        self.widget(name)
            .ok_or(SetError::UnknownWidget)?
            .set(prop, value)
    }

    /// Every widget's node identity, in document order (a container before
    /// its children).
    pub fn ids(&self) -> Vec<WidgetId> {
        self.widgets.iter().map(|widget| widget.id()).collect()
    }

    /// The pixel rectangle of the widget named `name`, if any.
    pub fn bounds(&self, name: &str) -> Option<Rect> {
        Some(self.ui.bounds(self.widget(name)?.id()))
    }

    /// The node names, in the order of [`LiveForm::ids`].
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|node| node.name.as_str())
    }

    /// The catalog the form was built against.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The canonical widget kind of the node named `name`, if any.
    ///
    /// An alias resolves to the canonical kind, so a `CommandButton` node
    /// reports `Button`.
    pub fn kind(&self, name: &str) -> Option<&str> {
        self.nodes
            .iter()
            .find(|node| node.name == name)
            .map(|node| node.kind.as_str())
    }

    /// The schema type of `property` on the node named `name`, if the catalog
    /// declares that property for the node's kind.
    ///
    /// This is the schema lookup a runtime needs to decode a script value into
    /// the right [`Value`] variant (for example an `enum` property).
    pub fn property_type(&self, name: &str, property: &str) -> Option<ValueType> {
        let kind = self.kind(name)?;
        self.catalog.property(kind, property).map(|spec| spec.ty)
    }

    /// The current bounds of every node the widget named `name` owns: one for
    /// most widgets, one per option for a `RadioGroup`. A designer outlines
    /// the union of these.
    pub fn node_bounds(&self, name: &str) -> Vec<Rect> {
        self.widget(name)
            .map(|widget| {
                widget
                    .node_ids()
                    .into_iter()
                    .map(|id| self.ui.bounds(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Lays the form out again now. It re-anchors itself on every resize, so
    /// this is only for reading new bounds within the same event handler.
    pub fn relayout(&self) {
        self.mounted.relayout();
    }
}
