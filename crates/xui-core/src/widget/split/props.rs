#![forbid(unsafe_code)]

//! [`Split`]'s property surface.

use super::Split;
use crate::property::{Properties, Property, PropertyInfo, PropertyType, Value, node};

impl<M: 'static> Properties for Split<M> {
    fn properties(&self) -> Vec<Property> {
        vec![
            Property {
                name: "position",
                value: Value::Dip(self.position()),
            },
            node::enabled(self.control.ui(), self.id()),
            node::visible(self.control.ui(), self.id()),
        ]
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        if node::set_visible(self.control.ui(), self.id(), name, &value) {
            return true;
        }
        match (name, value) {
            ("position", Value::Dip(position)) => {
                self.set_position(position);
                true
            }
            ("enabled", Value::Bool(enabled)) => {
                self.set_enabled(enabled);
                true
            }
            _ => false,
        }
    }

    fn schema(&self) -> Vec<PropertyInfo> {
        vec![
            PropertyInfo::new("position", PropertyType::Dip)
                .described("Layout", "The first pane's extent."),
            node::ENABLED,
            node::VISIBLE,
        ]
    }
}
