#![forbid(unsafe_code)]

//! The generic property surface for the native widgets.
//!
//! Every native widget that carries text and a [`Control`](crate::Control)
//! reports the same small set — `text`, `enabled`, `visible` — so a form
//! designer can read and edit it without knowing the concrete type. The
//! setters go through the widget's own typed API.
//!
//! The impls are per widget rather than one blanket: `Properties` is a foreign
//! trait, and Rust's coherence rules reject `impl<T: LocalTrait> ForeignTrait
//! for T`.

use xui_core::property::{Properties, Property, Value};

use crate::{Button, CheckBox, ControlExt, Edit, GroupBox, HasText, Label, RadioOption};

/// The properties every text-bearing widget reports, in a stable order.
fn properties_of<T: ControlExt + HasText + ?Sized>(widget: &T) -> Vec<Property> {
    vec![
        Property {
            name: "text",
            value: Value::Text(widget.text()),
        },
        Property {
            name: "enabled",
            value: Value::Bool(widget.is_enabled()),
        },
        Property {
            name: "visible",
            value: Value::Bool(widget.is_visible()),
        },
    ]
}

/// Applies a property edit through the widget's typed API, reporting whether it
/// matched.
fn set_property_of<T: ControlExt + HasText + ?Sized>(widget: &T, name: &str, value: Value) -> bool {
    match (name, value) {
        ("text", Value::Text(text)) => {
            widget.set_text(&text);
            true
        }
        ("enabled", Value::Bool(enabled)) => {
            widget.set_enabled(enabled);
            true
        }
        ("visible", Value::Bool(visible)) => {
            widget.set_visible(visible);
            true
        }
        _ => false,
    }
}

macro_rules! impl_properties {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Properties for $ty {
                fn properties(&self) -> Vec<Property> {
                    properties_of(self)
                }

                fn set_property(&self, name: &str, value: Value) -> bool {
                    set_property_of(self, name, value)
                }
            }
        )+
    };
}

impl_properties!(Label, GroupBox, RadioOption);

macro_rules! impl_properties_generic {
    ($($ty:ident),+ $(,)?) => {
        $(
            impl<M> Properties for $ty<M> {
                fn properties(&self) -> Vec<Property> {
                    properties_of(self)
                }

                fn set_property(&self, name: &str, value: Value) -> bool {
                    set_property_of(self, name, value)
                }
            }
        )+
    };
}

impl_properties_generic!(Button, CheckBox, Edit);
