#![forbid(unsafe_code)]

//! The properties every widget's node has, whatever the widget: whether it is
//! enabled and whether it is shown. A widget reports them after its own, and
//! sets them through its own setters so a change repaints as the widget would.

use super::{Property, PropertyInfo, PropertyType, Value};
use crate::app::Ui;
use crate::backend::WidgetId;

/// The schema of the `enabled` property.
pub(crate) const ENABLED: PropertyInfo = PropertyInfo::new("enabled", PropertyType::Bool)
    .described(
        "Behavior",
        "Whether the widget itself was enabled; a disabled one is dimmed and takes no input.",
    );

/// The schema of the `visible` property.
pub(crate) const VISIBLE: PropertyInfo = PropertyInfo::new("visible", PropertyType::Bool)
    .described(
        "Behavior",
        "Whether the widget itself was shown; a hidden one takes no space.",
    );

/// The `enabled` property of node `id`.
pub(crate) fn enabled<M: 'static>(ui: &Ui<M>, id: WidgetId) -> Property {
    Property {
        name: ENABLED.name,
        value: Value::Bool(ui.is_enabled(id)),
    }
}

/// The `visible` property of node `id`.
pub(crate) fn visible<M: 'static>(ui: &Ui<M>, id: WidgetId) -> Property {
    Property {
        name: VISIBLE.name,
        value: Value::Bool(ui.is_visible(id)),
    }
}

/// Sets node `id`'s `visible` property, returning whether `name` and `value`
/// were that property.
pub(crate) fn set_visible<M: 'static>(ui: &Ui<M>, id: WidgetId, name: &str, value: &Value) -> bool {
    match (name, value) {
        ("visible", Value::Bool(visible)) => {
            ui.set_visible(id, *visible);
            true
        }
        _ => false,
    }
}
