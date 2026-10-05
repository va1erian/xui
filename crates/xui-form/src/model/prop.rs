#![forbid(unsafe_code)]

//! [`PropType`]: how a widget property's Rust type maps onto the runtime
//! [`Value`] and its schema [`ValueType`].

use super::Orientation;
use crate::value::{Value, ValueType};

/// A Rust type a widget property can have.
pub trait PropType {
    /// The schema type of a property of this type.
    fn value_type() -> ValueType;

    /// The value as a runtime [`Value`].
    fn to_value(&self) -> Value;
}

impl PropType for String {
    fn value_type() -> ValueType {
        ValueType::Text { multiline: false }
    }

    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
}

impl PropType for bool {
    fn value_type() -> ValueType {
        ValueType::Bool
    }

    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
}

impl PropType for i64 {
    fn value_type() -> ValueType {
        ValueType::Int {
            min: None,
            max: None,
        }
    }

    fn to_value(&self) -> Value {
        Value::Int(*self)
    }
}

impl PropType for f64 {
    fn value_type() -> ValueType {
        ValueType::Float {
            min: None,
            max: None,
        }
    }

    fn to_value(&self) -> Value {
        Value::Float(*self)
    }
}

impl PropType for Vec<String> {
    fn value_type() -> ValueType {
        ValueType::List
    }

    fn to_value(&self) -> Value {
        Value::List(self.clone())
    }
}

impl PropType for Orientation {
    fn value_type() -> ValueType {
        ValueType::Enum {
            variants: vec!["Horizontal".to_owned(), "Vertical".to_owned()],
        }
    }

    fn to_value(&self) -> Value {
        Value::Enum(format!("{self:?}"))
    }
}
