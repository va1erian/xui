#![forbid(unsafe_code)]

//! The form schema: widget specs, their properties and events, and the
//! [`Catalog`] that maps kinds to specs.
//!
//! The catalog is the single source of truth shared by the document decoder,
//! validation, the designer's property grid and the runtime builder. The
//! built-in catalog, [`Catalog::xui`], describes the portable `xui-core`
//! widgets; a consumer may register its own widgets or alias a kind, so it can
//! expose `CommandButton` as `Button` without copying the spec.
//!
//! [`Catalog`] is [`Serialize`], so a tool can export it as JSON for the
//! designer, completion and documentation.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};
use xui_core::layout::Anchor;
use xui_core::units::Dip;

use crate::value::{Value, ValueType};

/// The twelve anchor names, in the order the designer shows them.
pub const ANCHOR_NAMES: [&str; 12] = [
    "top_left",
    "top",
    "top_right",
    "left",
    "center",
    "right",
    "bottom_left",
    "bottom",
    "bottom_right",
    "stretch_horizontal",
    "stretch_vertical",
    "fill",
];

/// The property-grid section a property belongs to.
pub const CATEGORY_APPEARANCE: &str = "Appearance";
/// The property-grid section for geometry.
pub const CATEGORY_LAYOUT: &str = "Layout";
/// The property-grid section for behaviour.
pub const CATEGORY_BEHAVIOR: &str = "Behavior";
/// The property-grid section for data.
pub const CATEGORY_DATA: &str = "Data";

/// The [`Anchor`] a name denotes, if any.
pub fn anchor_from_name(name: &str) -> Option<Anchor> {
    Some(match name {
        "top_left" => Anchor::TopLeft,
        "top" => Anchor::Top,
        "top_right" => Anchor::TopRight,
        "left" => Anchor::Left,
        "center" => Anchor::Center,
        "right" => Anchor::Right,
        "bottom_left" => Anchor::BottomLeft,
        "bottom" => Anchor::Bottom,
        "bottom_right" => Anchor::BottomRight,
        "stretch_horizontal" => Anchor::StretchHorizontal,
        "stretch_vertical" => Anchor::StretchVertical,
        "fill" => Anchor::Fill,
        _ => return None,
    })
}

/// The name an [`Anchor`] is written as.
pub fn anchor_name(anchor: Anchor) -> &'static str {
    match anchor {
        Anchor::TopLeft => "top_left",
        Anchor::Top => "top",
        Anchor::TopRight => "top_right",
        Anchor::Left => "left",
        Anchor::Center => "center",
        Anchor::Right => "right",
        Anchor::BottomLeft => "bottom_left",
        Anchor::Bottom => "bottom",
        Anchor::BottomRight => "bottom_right",
        Anchor::StretchHorizontal => "stretch_horizontal",
        Anchor::StretchVertical => "stretch_vertical",
        Anchor::Fill => "fill",
    }
}

/// When a property may be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// Readable and writable in the designer and at runtime.
    ReadWrite,
    /// Writable in the designer, read-only at runtime.
    DesignOnly,
    /// Read-only in the designer, writable at runtime.
    RuntimeOnly,
    /// Never writable.
    ReadOnly,
}

impl Access {
    /// Whether a value may be written in design mode.
    pub fn writable_in_design(self) -> bool {
        matches!(self, Access::ReadWrite | Access::DesignOnly)
    }

    /// Whether a value may be written at runtime.
    pub fn writable_at_runtime(self) -> bool {
        matches!(self, Access::ReadWrite | Access::RuntimeOnly)
    }
}

/// One property a widget kind accepts.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PropertySpec {
    /// The property name, as written on disk.
    pub name: String,
    /// The value's type; the schema decodes the raw literal against it.
    pub ty: ValueType,
    /// The value used when the property is absent.
    pub default: Value,
    /// The property-grid grouping.
    pub category: String,
    /// Help text for the property grid and completion.
    pub description: String,
    /// When the property may be written.
    pub access: Access,
}

impl PropertySpec {
    /// Whether `value` is acceptable for this property.
    pub fn accepts(&self, value: &Value) -> bool {
        self.ty.accepts(value)
    }
}

/// One argument of an event handler.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArgSpec {
    /// The argument's name.
    pub name: String,
    /// The argument's type.
    pub ty: ValueType,
}

/// One event a widget kind raises.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventSpec {
    /// The event name, given to [`crate::Binder::bind`].
    pub name: String,
    /// The handler's arguments, in order.
    pub args: Vec<ArgSpec>,
    /// Whether this is the event a double-click opens by default.
    pub is_default: bool,
    /// Help text for completion.
    pub description: String,
}

/// What a widget kind may contain.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Children {
    /// No children.
    None,
    /// Any registered kind.
    Any,
    /// Only these kinds (by canonical name).
    Only(Vec<String>),
}

impl Children {
    /// Whether `kind` (already resolved to a canonical name) is allowed.
    pub fn accepts(&self, kind: &str) -> bool {
        match self {
            Children::None => false,
            Children::Any => true,
            Children::Only(allowed) => allowed.iter().any(|candidate| candidate == kind),
        }
    }

    /// Whether this is the [`Children::None`] rule.
    pub fn is_none(&self) -> bool {
        matches!(self, Children::None)
    }
}

/// The properties, events and child rules of one widget kind.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WidgetSpec {
    /// The canonical kind name.
    pub kind: String,
    /// A one-line description, for the toolbox and docs.
    pub description: String,
    /// The widget-specific properties; the common ones are not repeated here.
    pub properties: Vec<PropertySpec>,
    /// The events the widget raises.
    pub events: Vec<EventSpec>,
    /// What the widget may contain.
    pub children: Children,
    /// The size a toolbox drop gives the widget, in design units.
    #[serde(serialize_with = "serialize_size")]
    pub default_size: (Dip, Dip),
}

impl WidgetSpec {
    /// The widget-specific property named `name`, if declared.
    pub fn property(&self, name: &str) -> Option<&PropertySpec> {
        self.properties.iter().find(|spec| spec.name == name)
    }

    /// The event named `name`, if declared.
    pub fn event(&self, name: &str) -> Option<&EventSpec> {
        self.events.iter().find(|spec| spec.name == name)
    }

    /// The default event, if one is declared.
    pub fn default_event(&self) -> Option<&EventSpec> {
        self.events.iter().find(|spec| spec.is_default)
    }
}

/// Serialises a design size as a two-element array.
fn serialize_size<S>((width, height): &(Dip, Dip), serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeTuple;
    let mut tuple = serializer.serialize_tuple(2)?;
    tuple.serialize_element(&width.value())?;
    tuple.serialize_element(&height.value())?;
    tuple.end()
}

/// The catalog of widget kinds: the schema every tool reads.
#[derive(Clone, Debug, Serialize)]
pub struct Catalog {
    /// Canonical kind to spec.
    kinds: BTreeMap<String, WidgetSpec>,
    /// Alias to canonical kind.
    aliases: BTreeMap<String, String>,
    /// The properties every node has, defined once.
    common: Vec<PropertySpec>,
    /// The window's own spec (it is not a widget kind).
    window: WidgetSpec,
}

impl Default for Catalog {
    fn default() -> Catalog {
        Catalog::new()
    }
}

impl Catalog {
    /// A catalog with the common properties and the window spec, and no widget
    /// kinds.
    pub fn new() -> Catalog {
        Catalog {
            kinds: BTreeMap::new(),
            aliases: BTreeMap::new(),
            common: builtin::common_properties(),
            window: builtin::window_spec(),
        }
    }

    /// The built-in catalog of portable `xui-core` widgets.
    pub fn xui() -> Catalog {
        let mut catalog = Catalog::new();
        for spec in widgets::widget_specs() {
            catalog.register(spec);
        }
        catalog
    }

    /// Adds a widget spec, replacing any spec with the same kind.
    pub fn register(&mut self, spec: WidgetSpec) {
        self.kinds.insert(spec.kind.clone(), spec);
    }

    /// Adds `alias` as another name for the canonical `kind`.
    ///
    /// The alias need not resolve yet; [`Catalog::get`] fails until it does.
    pub fn alias(&mut self, alias: impl Into<String>, kind: impl Into<String>) {
        self.aliases.insert(alias.into(), kind.into());
    }

    /// The canonical kind a name resolves to, following one alias.
    pub fn resolve<'a>(&'a self, name: &'a str) -> Option<&'a str> {
        if self.kinds.contains_key(name) {
            return Some(name);
        }
        self.aliases
            .get(name)
            .and_then(|kind| self.kinds.contains_key(kind).then_some(kind.as_str()))
    }

    /// The spec for `kind` (or an alias), if known.
    pub fn get(&self, kind: &str) -> Option<&WidgetSpec> {
        self.kinds.get(self.resolve(kind)?)
    }

    /// Whether `kind` (or an alias) is known.
    pub fn contains(&self, kind: &str) -> bool {
        self.resolve(kind).is_some()
    }

    /// The canonical kind names, in sorted order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.kinds.keys().map(String::as_str)
    }

    /// The aliases, as `(alias, canonical kind)` pairs in sorted order.
    pub fn aliases(&self) -> impl Iterator<Item = (&str, &str)> {
        self.aliases
            .iter()
            .map(|(alias, kind)| (alias.as_str(), kind.as_str()))
    }

    /// The window's spec.
    pub fn window_spec(&self) -> &WidgetSpec {
        &self.window
    }

    /// Replaces the window spec.
    pub fn set_window_spec(&mut self, spec: WidgetSpec) {
        self.window = spec;
    }

    /// The properties every node has, in save order.
    pub fn common_properties(&self) -> &[PropertySpec] {
        &self.common
    }

    /// The property spec for `name` on `kind`, checking the widget first and
    /// the common properties second.
    ///
    /// `width` and `height` take their default from the widget's
    /// [`WidgetSpec::default_size`], so a node that omits them is sized for its
    /// kind.
    pub fn property(&self, kind: &str, name: &str) -> Option<PropertySpec> {
        if let Some(spec) = self.get(kind) {
            if let Some(property) = spec.property(name) {
                return Some(property.clone());
            }
            if name == "width" || name == "height" {
                let common = self.common.iter().find(|property| property.name == name)?;
                let rounded = if name == "width" {
                    spec.default_size.0.value().round() as i64
                } else {
                    spec.default_size.1.value().round() as i64
                };
                let mut property = common.clone();
                property.default = Value::Int(rounded);
                return Some(property);
            }
        }
        self.common
            .iter()
            .find(|property| property.name == name)
            .cloned()
    }

    /// The names of every property a node of `kind` accepts: the widget's own
    /// first, then the common ones, in declaration order.
    pub fn property_names(&self, kind: &str) -> Vec<String> {
        let own = self.get(kind).map(|spec| spec.properties.as_slice());
        let mut names: Vec<String> = Vec::new();
        for property in own.unwrap_or_default().iter().chain(&self.common) {
            if !names.contains(&property.name) {
                names.push(property.name.clone());
            }
        }
        names
    }

    /// Whether `kind` (or an alias) is a container.
    pub fn is_container(&self, kind: &str) -> bool {
        self.get(kind).is_some_and(|spec| !spec.children.is_none())
    }

    /// Whether a node of `child_kind` may be placed inside `parent_kind`.
    pub fn accepts_child(&self, parent_kind: &str, child_kind: &str) -> bool {
        let (Some(parent), Some(child)) = (self.get(parent_kind), self.get(child_kind)) else {
            return false;
        };
        parent.children.accepts(&child.kind)
    }
}

mod builtin;
#[cfg(test)]
mod tests;
mod widgets;
