#![forbid(unsafe_code)]

//! Typed runtime property values and their schema types.
//!
//! A live form reads and writes properties as [`Value`]s, checked against
//! each property's [`ValueType`] from the schema. An [`Value::Int`] is
//! accepted wherever a [`ValueType::Float`] is expected.

use serde::Serialize;
use xui_core::Color;

/// A typed property value.
///
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A boolean.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A real number; an [`Value::Int`] is also accepted where a float is
    /// expected.
    Float(f64),
    /// Free-form text.
    Text(String),
    /// One of a fixed set of strings.
    Enum(String),
    /// An opaque colour.
    Color(Color),
    /// A list of strings (list and combo items).
    List(Vec<String>),
}

impl Value {
    /// The variant's name, for diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Text(_) => "text",
            Value::Enum(_) => "enum",
            Value::Color(_) => "color",
            Value::List(_) => "list",
        }
    }

    /// The boolean inside, if any.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The integer inside, if any.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// The real number inside, widening an [`Value::Int`].
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(value) => Some(*value),
            Value::Int(value) => Some(*value as f64),
            _ => None,
        }
    }

    /// The string inside, for both [`Value::Text`] and [`Value::Enum`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(value) | Value::Enum(value) => Some(value),
            _ => None,
        }
    }

    /// The colour inside, if any.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Value::Color(value) => Some(*value),
            _ => None,
        }
    }

    /// The string list inside, if any.
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Value::List(value) => Some(value),
            _ => None,
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Bool(value) => serializer.serialize_bool(*value),
            Value::Int(value) => serializer.serialize_i64(*value),
            Value::Float(value) => serializer.serialize_f64(*value),
            Value::Text(value) | Value::Enum(value) => serializer.serialize_str(value),
            Value::Color(color) => serializer.serialize_str(&format_color(*color)),
            Value::List(items) => items.serialize(serializer),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Text(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value)
    }
}

impl From<Color> for Value {
    fn from(value: Color) -> Self {
        Value::Color(value)
    }
}

/// The type a property's value must have.
///
/// The bounds and variants are part of the schema, so a designer can build the
/// right editor and validation can check without a second source of truth.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueType {
    /// A boolean.
    Bool,
    /// A whole number, optionally bounded (inclusive).
    Int {
        /// The smallest allowed value.
        min: Option<i64>,
        /// The largest allowed value.
        max: Option<i64>,
    },
    /// A real number, optionally bounded (inclusive); an integer is accepted.
    Float {
        /// The smallest allowed value.
        min: Option<f64>,
        /// The largest allowed value.
        max: Option<f64>,
    },
    /// Free-form text, possibly multi-line.
    Text {
        /// Whether the editor should allow line breaks.
        multiline: bool,
    },
    /// One of a fixed set of strings.
    Enum {
        /// The allowed strings, in display order.
        variants: Vec<String>,
    },
    /// An opaque `#rrggbb` colour.
    Color,
    /// A list of strings.
    List,
}

impl ValueType {
    /// The type's display name, for diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            ValueType::Bool => "bool",
            ValueType::Int { .. } => "int",
            ValueType::Float { .. } => "float",
            ValueType::Text { .. } => "text",
            ValueType::Enum { .. } => "enum",
            ValueType::Color => "color",
            ValueType::List => "list",
        }
    }

    /// The allowed enum variants, or an empty slice for other kinds.
    pub fn variants(&self) -> &[String] {
        match self {
            ValueType::Enum { variants } => variants,
            _ => &[],
        }
    }

    /// Whether `value` has a compatible type; enum membership and the numeric
    /// bounds are checked here too.
    pub fn accepts(&self, value: &Value) -> bool {
        match self {
            ValueType::Bool => matches!(value, Value::Bool(_)),
            ValueType::Int { min, max } => match value {
                Value::Int(value) => in_int_range(*value, *min, *max),
                _ => false,
            },
            ValueType::Float { min, max } => match value.as_float() {
                Some(value) => in_float_range(value, *min, *max),
                None => false,
            },
            ValueType::Text { .. } => matches!(value, Value::Text(_)),
            ValueType::Enum { variants } => match value {
                Value::Enum(value) => variants.iter().any(|variant| variant == value),
                _ => false,
            },
            ValueType::Color => matches!(value, Value::Color(_)),
            ValueType::List => matches!(value, Value::List(_)),
        }
    }
}

/// Whether `value` lies within the inclusive integer bounds.
fn in_int_range(value: i64, min: Option<i64>, max: Option<i64>) -> bool {
    min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
}

/// Whether `value` lies within the inclusive float bounds.
fn in_float_range(value: f64, min: Option<f64>, max: Option<f64>) -> bool {
    min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
}

/// Formats an opaque colour as `#rrggbb`.
pub(crate) fn format_color(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

#[cfg(test)]
mod tests;
