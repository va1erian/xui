#![forbid(unsafe_code)]

//! The form schema: what each node kind accepts, generated from the same
//! declarations as the file format (see [`crate::model`]).
//!
//! The [`Catalog`] is the one source of truth for the designer's property
//! grid, validation, the live form's typed reads and writes, the script
//! binding and `xui-form schema --json`: it is [`Serialize`].

use std::collections::BTreeMap;

use serde::Serialize;

use crate::value::{Value, ValueType};

mod catalog;
#[cfg(test)]
mod tests;

pub use catalog::Catalog;

/// The property-grid section for appearance.
pub const CATEGORY_APPEARANCE: &str = "Appearance";
/// The property-grid section for geometry.
pub const CATEGORY_LAYOUT: &str = "Layout";
/// The property-grid section for behaviour.
pub const CATEGORY_BEHAVIOR: &str = "Behavior";
/// The property-grid section for data.
pub const CATEGORY_DATA: &str = "Data";

/// When a property may be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// Readable and writable in the designer and at runtime.
    ReadWrite,
    /// Writable in the designer, read-only at runtime: it is used when the
    /// widget is created.
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

/// One property a widget kind has at runtime: what a live form reads and
/// writes and a script sees as `control.name`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PropertySpec {
    /// The property name.
    pub name: String,
    /// The value's type.
    pub ty: ValueType,
    /// The value used when the file omits it.
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
    /// The event name, given to [`crate::Binder::bind`]; a script handler is
    /// `<name>_<event>` in snake_case.
    pub name: String,
    /// The handler's arguments, in order (a control array's elements pass
    /// their index first).
    pub args: Vec<ArgSpec>,
    /// Whether this is the event a double-click opens by default.
    pub is_default: bool,
    /// Help text for completion.
    pub description: String,
}

/// One field of the file format: what a node may be written with.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FieldSpec {
    /// The field name.
    pub name: String,
    /// Its Rust type, which reads as its RON shape (`Option<Length>`).
    pub ty: String,
    /// Its default, as RON; a field equal to it is not written.
    pub default: String,
    /// Help text.
    pub description: String,
}

impl FieldSpec {
    /// A field from its declaration.
    pub(crate) fn new(name: &str, ty: &str, default: &impl Serialize, doc: &[&str]) -> FieldSpec {
        FieldSpec {
            name: name.to_owned(),
            ty: ty.split_whitespace().collect(),
            default: crate::codec::to_ron_compact(default),
            description: crate::model::doc(doc),
        }
    }
}

/// A widget kind: its runtime properties, its events and its fields.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WidgetSpec {
    /// The kind, as written in a file.
    pub kind: String,
    /// A one-line description, for the toolbox and docs.
    pub description: String,
    /// The widget's own runtime properties; the [common
    /// ones](Catalog::common_properties) are not repeated.
    pub properties: Vec<PropertySpec>,
    /// The events the widget raises.
    pub events: Vec<EventSpec>,
    /// Every field a node of this kind may be written with.
    pub fields: Vec<FieldSpec>,
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

/// A layout kind (`Row`, `Grid`, …): it holds nodes and raises nothing.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LayoutSpec {
    /// The kind, as written in a file.
    pub kind: String,
    /// A one-line description.
    pub description: String,
    /// Every field a node of this kind may be written with.
    pub fields: Vec<FieldSpec>,
}

/// The runtime properties every widget has, keyed by name.
pub(crate) fn common_properties() -> Vec<PropertySpec> {
    let flag = |name: &str, description: &str| PropertySpec {
        name: name.to_owned(),
        ty: ValueType::Bool,
        default: Value::Bool(true),
        category: CATEGORY_BEHAVIOR.to_owned(),
        description: description.to_owned(),
        access: Access::ReadWrite,
    };
    let geometry = |name: &str, description: &str| PropertySpec {
        name: name.to_owned(),
        ty: ValueType::Int {
            min: None,
            max: None,
        },
        default: Value::Int(0),
        category: CATEGORY_LAYOUT.to_owned(),
        description: format!("{description} Only in an `Absolute` layout."),
        access: Access::ReadWrite,
    };
    vec![
        flag("visible", "Whether the widget is shown."),
        flag("enabled", "Whether the widget accepts input."),
        geometry("left", "The x of its `at` rectangle, in design units."),
        geometry("top", "The y of its `at` rectangle, in design units."),
        geometry("width", "The width of its `at` rectangle, in design units."),
        geometry(
            "height",
            "The height of its `at` rectangle, in design units.",
        ),
    ]
}

/// Specs by kind, for the catalog.
pub(crate) type Specs = BTreeMap<String, WidgetSpec>;
