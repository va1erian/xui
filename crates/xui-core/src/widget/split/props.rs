#![forbid(unsafe_code)]

//! [`Split`]'s property surface.

use super::Split;
use crate::property::{Properties, Property, PropertyInfo, PropertyType, Value};

impl<M: 'static> Properties for Split<M> {
    fn properties(&self) -> Vec<Property> {
        vec![Property {
            name: "position",
            value: Value::Dip(self.position()),
        }]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        match (name, value) {
            ("position", Value::Dip(position)) => {
                self.set_position(position);
                true
            }
            _ => false,
        }
    }

    fn schema(&self) -> Vec<PropertyInfo> {
        vec![
            PropertyInfo::new("position", PropertyType::Dip)
                .described("Layout", "The first pane's extent."),
        ]
    }
}
