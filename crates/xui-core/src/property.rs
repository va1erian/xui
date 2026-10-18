#![forbid(unsafe_code)]

//! A generic property surface for widgets.
//!
//! A form designer (or any tool) needs to read and edit a widget's properties
//! without knowing its concrete type. A widget implements [`Properties`] and
//! reports a small, stable set: a name and a typed [`Value`]. Setters go
//! through the widget's own typed API, so editing a property is exactly what
//! the widget would do itself. [`Properties::schema`] describes the set for a
//! designer's editors.

mod value;

pub use value::{Font, Value};

/// What a property's value is, so a designer can build the right editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PropertyType {
    /// A boolean (a check box).
    Bool,
    /// A whole number (a stepper).
    Integer,
    /// A real number (a number field).
    Float,
    /// A string (a text field).
    Text,
    /// One of a fixed set of strings (a drop-down).
    Enum(&'static [&'static str]),
    /// A colour (a colour picker).
    Color,
    /// A font (a font editor).
    Font,
    /// A length in design units (a number with a unit).
    Dip,
    /// A picture (a picture editor).
    Image,
}

/// One property's schema: what a designer needs to present it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyInfo {
    /// The stable property name, as [`Properties::property`] takes it.
    pub name: &'static str,
    /// The value's type.
    pub ty: PropertyType,
    /// Whether the property is shown but not editable.
    pub read_only: bool,
    /// The group a property grid sorts it under (empty for none).
    pub category: &'static str,
    /// A one-line description, for a tooltip (empty for none).
    pub description: &'static str,
}

impl PropertyInfo {
    /// A read-write property with no category or description.
    pub const fn new(name: &'static str, ty: PropertyType) -> PropertyInfo {
        PropertyInfo {
            name,
            ty,
            read_only: false,
            category: "",
            description: "",
        }
    }

    /// Marks the property as shown but not editable.
    pub const fn read_only(mut self) -> PropertyInfo {
        self.read_only = true;
        self
    }

    /// Sets the group a property grid sorts it under and its description.
    pub const fn described(
        mut self,
        category: &'static str,
        description: &'static str,
    ) -> PropertyInfo {
        self.category = category;
        self.description = description;
        self
    }
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

    /// The schema of the widget's properties: one [`PropertyInfo`] per
    /// property the widget can report, in the same order, so a designer can
    /// build the right editor for each. A property with no current value (a
    /// colour picker's colour before one is selected) is in the schema but
    /// absent from [`properties`](Self::properties).
    ///
    /// The default infers each type from the current value and carries no
    /// category or description; a widget whose property is an enum, a colour,
    /// a font, a length or a picture overrides it with the exact type.
    fn schema(&self) -> Vec<PropertyInfo> {
        self.properties()
            .into_iter()
            .map(|property| {
                let ty = match property.value {
                    Value::Bool(_) => PropertyType::Bool,
                    Value::Integer(_) => PropertyType::Integer,
                    Value::Float(_) => PropertyType::Float,
                    Value::Text(_) => PropertyType::Text,
                    Value::Enum(_) => PropertyType::Enum(&[]),
                    Value::Color(_) => PropertyType::Color,
                    Value::Font(_) => PropertyType::Font,
                    Value::Dip(_) => PropertyType::Dip,
                    Value::Image(_) => PropertyType::Image,
                };
                PropertyInfo::new(property.name, ty)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A widget that reports one property of each kind.
    struct EveryKind;

    impl Properties for EveryKind {
        fn properties(&self) -> Vec<Property> {
            vec![
                Property {
                    name: "flag",
                    value: Value::Bool(true),
                },
                Property {
                    name: "step",
                    value: Value::Integer(2),
                },
            ]
        }

        fn set_property(&self, _name: &str, _value: Value) -> bool {
            false
        }
    }

    #[test]
    fn the_default_schema_infers_each_type() {
        assert_eq!(
            EveryKind.schema(),
            [
                PropertyInfo::new("flag", PropertyType::Bool),
                PropertyInfo::new("step", PropertyType::Integer),
            ]
        );
    }

    #[test]
    fn a_property_info_carries_its_metadata() {
        let info = PropertyInfo::new("font", PropertyType::Font)
            .read_only()
            .described("Appearance", "The label's font.");
        assert_eq!(info.name, "font");
        assert_eq!(info.ty, PropertyType::Font);
        assert!(info.read_only);
        assert_eq!(info.category, "Appearance");
        assert_eq!(info.description, "The label's font.");
    }
}
