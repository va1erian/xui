#![forbid(unsafe_code)]

//! A generic property surface for widgets.
//!
//! A form designer (or any tool) needs to read and edit a widget's properties
//! without knowing its concrete type. A widget implements [`Properties`] and
//! reports a small, stable set: a name and a typed [`Value`]. Setters go
//! through the widget's own typed API, so editing a property is exactly what
//! the widget would do itself.

/// A property's value.
///
/// More variants (a colour) join when a widget reports them; the set stays
/// small so every widget that reports a property can act on it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A boolean (enabled, visible, checked).
    Bool(bool),
    /// A whole number (a progress value or range).
    Integer(i64),
    /// A real number (a slider's value).
    Float(f64),
    /// A string (a label, an edit's text).
    Text(String),
}

/// One named property of a widget.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// The stable property name (a form file keys on it).
    pub name: &'static str,
    /// The current value.
    pub value: Value,
}

/// A widget that reports and edits a small, generic property set.
pub trait Properties {
    /// A snapshot of the widget's properties, in a stable order.
    fn properties(&self) -> Vec<Property>;

    /// The value of the property named `name`, if the widget has it.
    fn property(&self, name: &str) -> Option<Value> {
        self.properties()
            .into_iter()
            .find(|property| property.name == name)
            .map(|property| property.value)
    }

    /// Sets the property named `name` to `value`, returning whether it applied.
    /// A mismatched value type is ignored (returns `false`).
    fn set_property(&self, name: &str, value: Value) -> bool;
}
