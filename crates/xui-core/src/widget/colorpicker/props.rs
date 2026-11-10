#![forbid(unsafe_code)]

//! [`ColorPicker`]'s property surface.

use super::ColorPicker;
use crate::property::{Properties, Property, PropertyInfo, PropertyType, Value, node};

impl<M: 'static> Properties for ColorPicker<M> {
    /// The selected colour, or nothing while none is selected (the schema
    /// still reports the property, so a designer shows its editor).
    fn properties(&self) -> Vec<Property> {
        let color = self.color().map(|color| Property {
            name: "color",
            value: Value::Color(color),
        });
        color
            .into_iter()
            .chain([node::visible(self.control.ui(), self.id())])
            .collect()
    }

    fn set_property(&self, name: &str, value: Value) -> bool {
        if node::set_visible(self.control.ui(), self.id(), name, &value) {
            return true;
        }
        match (name, value) {
            ("color", Value::Color(color)) => {
                self.select(color);
                true
            }
            _ => false,
        }
    }

    fn schema(&self) -> Vec<PropertyInfo> {
        vec![
            PropertyInfo::new("color", PropertyType::Color)
                .described("Appearance", "The selected colour."),
            node::VISIBLE,
        ]
    }
}
