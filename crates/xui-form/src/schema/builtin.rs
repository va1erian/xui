#![forbid(unsafe_code)]

//! The common properties, the window's spec and the helpers the built-in
//! widget specs are written with.

use xui_core::units::Dip;

use super::{
    ANCHOR_NAMES, Access, ArgSpec, CATEGORY_APPEARANCE, CATEGORY_BEHAVIOR, CATEGORY_DATA,
    CATEGORY_LAYOUT, Children, EventSpec, PropertySpec, WidgetSpec,
};
use crate::value::{Value, ValueType};

/// Builds a read-write property spec.
pub(super) fn property(
    name: &str,
    ty: ValueType,
    default: Value,
    category: &str,
    description: &str,
) -> PropertySpec {
    PropertySpec {
        name: name.to_owned(),
        ty,
        default,
        category: category.to_owned(),
        description: description.to_owned(),
        access: Access::ReadWrite,
    }
}

/// Builds a design-only property spec.
pub(super) fn design(
    name: &str,
    ty: ValueType,
    default: Value,
    category: &str,
    desc: &str,
) -> PropertySpec {
    PropertySpec {
        access: Access::DesignOnly,
        ..property(name, ty, default, category, desc)
    }
}

/// An int type bounded below by `min`, if any.
pub(super) fn int_from(min: Option<i64>) -> ValueType {
    ValueType::Int { min, max: None }
}

/// An unbounded float type.
pub(super) fn float_type() -> ValueType {
    ValueType::Float {
        min: None,
        max: None,
    }
}

/// A single-line text type.
pub(super) fn text_type() -> ValueType {
    ValueType::Text { multiline: false }
}

/// An enum type from variant names.
pub(super) fn enum_type(variants: &[&str]) -> ValueType {
    ValueType::Enum {
        variants: variants.iter().map(|name| (*name).to_owned()).collect(),
    }
}

/// A text property with an empty default.
pub(super) fn text(name: &str, category: &str, description: &str) -> PropertySpec {
    property(
        name,
        text_type(),
        Value::Text(String::new()),
        category,
        description,
    )
}

/// A float property.
pub(super) fn float(name: &str, default: f64, description: &str) -> PropertySpec {
    property(
        name,
        float_type(),
        Value::Float(default),
        CATEGORY_DATA,
        description,
    )
}

/// A bool property in the data category.
pub(super) fn checked(description: &str) -> PropertySpec {
    property(
        "checked",
        ValueType::Bool,
        Value::Bool(false),
        CATEGORY_DATA,
        description,
    )
}

/// A design-only list of item labels.
pub(super) fn items(description: &str) -> PropertySpec {
    design(
        "items",
        ValueType::List,
        Value::List(Vec::new()),
        CATEGORY_DATA,
        description,
    )
}

/// The selected index, from zero.
pub(super) fn selected(description: &str) -> PropertySpec {
    property(
        "selected",
        int_from(Some(0)),
        Value::Int(0),
        CATEGORY_DATA,
        description,
    )
}

/// Builds an event spec.
pub(super) fn event(
    name: &str,
    args: Vec<ArgSpec>,
    is_default: bool,
    description: &str,
) -> EventSpec {
    EventSpec {
        name: name.to_owned(),
        args,
        is_default,
        description: description.to_owned(),
    }
}

/// Builds an event argument spec.
pub(super) fn arg(name: &str, ty: ValueType) -> ArgSpec {
    ArgSpec {
        name: name.to_owned(),
        ty,
    }
}

/// A `Change`/`Commit` pair carrying a float `value`.
pub(super) fn value_events(change: &str, commit: &str) -> Vec<EventSpec> {
    vec![
        event("Change", vec![arg("value", float_type())], true, change),
        event("Commit", vec![arg("value", float_type())], false, commit),
    ]
}

/// Builds a widget spec.
pub(super) fn widget(
    kind: &str,
    description: &str,
    size: (f32, f32),
    properties: Vec<PropertySpec>,
    events: Vec<EventSpec>,
    children: Children,
) -> WidgetSpec {
    WidgetSpec {
        kind: kind.to_owned(),
        description: description.to_owned(),
        properties,
        events,
        children,
        default_size: (Dip(size.0), Dip(size.1)),
    }
}

/// A widget with no children.
pub(super) fn leaf(
    kind: &str,
    description: &str,
    size: (f32, f32),
    properties: Vec<PropertySpec>,
    events: Vec<EventSpec>,
) -> WidgetSpec {
    widget(kind, description, size, properties, events, Children::None)
}

/// The common properties, in save order.
pub(super) fn common_properties() -> Vec<PropertySpec> {
    let geometry = |name: &str, description: &str| {
        property(
            name,
            int_from(None),
            Value::Int(0),
            CATEGORY_LAYOUT,
            description,
        )
    };
    let flag = |name: &str, description: &str| {
        property(
            name,
            ValueType::Bool,
            Value::Bool(true),
            CATEGORY_BEHAVIOR,
            description,
        )
    };
    vec![
        geometry(
            "left",
            "The x offset from the parent's left edge, in design units.",
        ),
        geometry(
            "top",
            "The y offset from the parent's top edge, in design units.",
        ),
        geometry("width", "The width, in design units."),
        geometry("height", "The height, in design units."),
        property(
            "anchor",
            enum_type(&ANCHOR_NAMES),
            Value::Enum("top_left".to_owned()),
            CATEGORY_LAYOUT,
            "How the node follows its parent when it is resized.",
        ),
        flag("visible", "Whether the node is shown."),
        flag("enabled", "Whether the node accepts input."),
        property(
            "tab_index",
            int_from(Some(0)),
            Value::Int(0),
            CATEGORY_BEHAVIOR,
            "The node's place in the tab order among its siblings.",
        ),
    ]
}

/// The window's spec; the window is not a widget kind.
pub(super) fn window_spec() -> WidgetSpec {
    let int = ValueType::Int {
        min: None,
        max: None,
    };
    let extent = |name: &str, default: i64, description: &str| {
        property(
            name,
            int_from(Some(1)),
            Value::Int(default),
            CATEGORY_LAYOUT,
            description,
        )
    };
    widget(
        "Window",
        "The top-level window that hosts the form.",
        (320.0, 200.0),
        vec![
            text(
                "title",
                CATEGORY_APPEARANCE,
                "The title shown in the window's title bar.",
            ),
            extent("width", 320, "The client width, in design units."),
            extent("height", 200, "The client height, in design units."),
            property(
                "resizable",
                ValueType::Bool,
                Value::Bool(true),
                CATEGORY_BEHAVIOR,
                "Whether the window can be resized.",
            ),
        ],
        vec![
            event("Load", Vec::new(), true, "Raised once the window is built."),
            event(
                "Close",
                vec![arg("cancel", ValueType::Bool)],
                false,
                "Raised when the window is asked to close; `cancel` stops it.",
            ),
            event(
                "Resize",
                vec![arg("width", int.clone()), arg("height", int)],
                false,
                "Raised when the client area changes size.",
            ),
            event(
                "Activate",
                Vec::new(),
                false,
                "Raised when the window is focused.",
            ),
        ],
        Children::None,
    )
}
