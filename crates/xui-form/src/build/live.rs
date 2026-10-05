#![forbid(unsafe_code)]

//! [`Controls`] and [`LiveForm`], and the wrapper that gives every built-in
//! widget the common properties.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use xui_core::WidgetId;
use xui_core::app::Ui;
use xui_core::arrange::Mounted;
use xui_core::geometry::Rect;
use xui_core::layout::Placement;
use xui_core::units::Dip;

use super::tree::Named;
use super::{LiveWidget, SetError};
use crate::schema::Catalog;
use crate::value::{Value, ValueType};

/// The widget-specific property surface a built-in widget implements.
///
/// The [`Live`] wrapper adds the common properties (`visible`, `enabled` and,
/// in an absolute layout, the geometry) and the schema's access and type
/// rules.
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
    /// Applies state the builder cannot set before the widget exists. The
    /// default does nothing.
    fn ready(&self) {}
}

/// The geometry properties, in `at` order.
const GEOMETRY: [&str; 4] = ["left", "top", "width", "height"];

/// The common property state every built-in widget shares.
pub(super) struct Common<M: 'static> {
    ui: Ui<M>,
    placement: Option<Placement>,
    visible: Cell<bool>,
    enabled: Cell<bool>,
}

impl<M: 'static> Common<M> {
    /// The common state of a widget, placed by `placement` in an absolute
    /// layout.
    pub(super) fn new(ui: Ui<M>, placement: Option<Placement>) -> Common<M> {
        Common {
            ui,
            placement,
            visible: Cell::new(true),
            enabled: Cell::new(true),
        }
    }

    /// Records the state the file asks for, applied once the widget exists.
    pub(super) fn init(&self, visible: bool, enabled: bool) {
        self.visible.set(visible);
        self.enabled.set(enabled);
    }

    /// Reads a common property.
    fn get(&self, prop: &str) -> Option<Value> {
        if let Some(slot) = GEOMETRY.iter().position(|name| *name == prop) {
            let rect = self.placement.as_ref()?.rect();
            return Some(Value::Int(rect[slot].value().round() as i64));
        }
        match prop {
            "visible" => Some(Value::Bool(self.visible.get())),
            "enabled" => Some(Value::Bool(self.enabled.get())),
            _ => None,
        }
    }

    /// Records a common property already checked against the schema; `None`
    /// when `prop` is not a common property.
    fn record(&self, prop: &str, value: &Value) -> Option<Result<(), SetError>> {
        if let Some(slot) = GEOMETRY.iter().position(|name| *name == prop) {
            let Some(placement) = &self.placement else {
                return Some(Err(SetError::ReadOnly));
            };
            let mut rect = placement.rect();
            rect[slot] = Dip(value.as_int().unwrap_or_default() as f32);
            placement.set_rect(rect);
            return Some(Ok(()));
        }
        match (prop, value) {
            ("visible", Value::Bool(visible)) => self.visible.set(*visible),
            ("enabled", Value::Bool(enabled)) => self.enabled.set(*enabled),
            _ => return None,
        }
        Some(Ok(()))
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
            spec.access.writable_in_design()
        } else {
            spec.access.writable_at_runtime()
        };
        if !writable {
            return Err(SetError::ReadOnly);
        }
        if !spec.accepts(value) {
            return Err(SetError::TypeMismatch);
        }
        match self.common.record(prop, value) {
            Some(Ok(())) => {
                self.apply_common(prop);
                Ok(())
            }
            Some(error) => error,
            None => self.inner.set_own(prop, value),
        }
    }

    fn node_ids(&self) -> Vec<WidgetId> {
        self.inner.node_ids()
    }

    fn ready(&self) {
        for prop in ["visible", "enabled"] {
            if self.common.get(prop) == Some(Value::Bool(false)) {
                self.apply_common(prop);
            }
        }
        self.inner.ready();
    }
}

impl<M: 'static, W: WidgetProps<M>> Live<M, W> {
    /// Applies a common property that was just recorded to every node the
    /// widget owns, so a multi-node widget moves, hides and disables as one.
    fn apply_common(&self, prop: &str) {
        let ui = &self.common.ui;
        match prop {
            "visible" => {
                for id in self.inner.node_ids() {
                    ui.set_visible(id, self.common.visible.get());
                }
            }
            "enabled" => {
                let enabled = self.common.enabled.get();
                for id in self.inner.node_ids() {
                    ui.set_enabled(id, enabled);
                }
                self.inner.set_enabled_hint(enabled);
            }
            // Geometry lives in the placement the layout reads.
            _ => ui.relayout(),
        }
    }
}

/// A form's named widgets, read and written by name once the form's layout
/// is mounted. A control array's elements are named `name[index]`.
pub struct Controls<M: 'static> {
    ui: Ui<M>,
    catalog: Rc<Catalog>,
    widgets: Vec<Named<M>>,
    by_name: BTreeMap<String, usize>,
}

impl<M: 'static> Controls<M> {
    pub(super) fn new(ui: Ui<M>, catalog: Rc<Catalog>, widgets: Vec<Named<M>>) -> Controls<M> {
        let by_name = widgets
            .iter()
            .enumerate()
            .map(|(index, (name, ..))| (name.clone(), index))
            .collect();
        Controls {
            ui,
            catalog,
            widgets,
            by_name,
        }
    }

    /// Applies the state a widget can only take once it exists (a hidden or
    /// disabled widget, a tab control's selected page).
    /// [`build`](super::build) calls it; call it once after mounting the
    /// layout of a form [`describe`](super::describe)d.
    pub fn ready(&self) {
        for (_, _, widget) in &self.widgets {
            widget.ready();
        }
    }

    /// The widget named `name`, if any.
    pub fn widget(&self, name: &str) -> Option<&dyn LiveWidget<M>> {
        let index = *self.by_name.get(name)?;
        Some(self.widgets[index].2.as_ref())
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

    /// Every named widget's node identity, in tree order.
    pub fn ids(&self) -> Vec<WidgetId> {
        self.widgets
            .iter()
            .map(|(.., widget)| widget.id())
            .collect()
    }

    /// The widget names, in the order of [`Controls::ids`].
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.widgets.iter().map(|(name, ..)| name.as_str())
    }

    /// The pixel rectangle of the widget named `name`, if any.
    pub fn bounds(&self, name: &str) -> Option<Rect> {
        Some(self.ui.bounds(self.widget(name)?.id()))
    }

    /// The current bounds of every node the widget named `name` owns: one
    /// for most widgets, one per option for a `RadioGroup`.
    pub fn node_bounds(&self, name: &str) -> Vec<Rect> {
        self.widget(name)
            .map(|widget| {
                let ids = widget.node_ids();
                ids.into_iter().map(|id| self.ui.bounds(id)).collect()
            })
            .unwrap_or_default()
    }

    /// The catalog the form was built against.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The kind of the widget named `name`, if any.
    pub fn kind(&self, name: &str) -> Option<&str> {
        let index = *self.by_name.get(name)?;
        Some(self.widgets[index].1.as_str())
    }

    /// The schema type of `property` on the widget named `name`, which a
    /// runtime decodes a script value against.
    pub fn property_type(&self, name: &str, property: &str) -> Option<ValueType> {
        let kind = self.kind(name)?;
        self.catalog.property(kind, property).map(|spec| spec.ty)
    }
}

/// A built form: its widgets, mounted. Dropping it destroys them.
pub struct LiveForm<M: 'static> {
    controls: Controls<M>,
    mounted: Mounted<M>,
}

impl<M: 'static> LiveForm<M> {
    /// The form over `controls`, mounted as `mounted`.
    pub(super) fn new(controls: Controls<M>, mounted: Mounted<M>) -> LiveForm<M> {
        controls.ready();
        LiveForm { controls, mounted }
    }

    /// The form's named widgets.
    pub fn controls(&self) -> &Controls<M> {
        &self.controls
    }

    /// The widget named `name`, if any.
    pub fn widget(&self, name: &str) -> Option<&dyn LiveWidget<M>> {
        self.controls.widget(name)
    }

    /// The value of `prop` on the widget named `name`, if any.
    pub fn get(&self, name: &str, prop: &str) -> Option<Value> {
        self.controls.get(name, prop)
    }

    /// Sets `prop` on the widget named `name`.
    pub fn set(&self, name: &str, prop: &str, value: &Value) -> Result<(), SetError> {
        self.controls.set(name, prop, value)
    }

    /// The pixel rectangle of the widget named `name`, if any.
    pub fn bounds(&self, name: &str) -> Option<Rect> {
        self.controls.bounds(name)
    }

    /// Lays the form out again now. It re-flows on its own after every
    /// change, so this is only for reading new bounds within the same event
    /// handler.
    pub fn relayout(&self) {
        self.mounted.relayout();
    }
}
