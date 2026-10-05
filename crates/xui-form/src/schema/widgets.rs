#![forbid(unsafe_code)]

//! The specs of the portable `xui-core` widgets.

use super::builtin::{
    arg, checked, design, enum_type, event, float, float_type, int_from, items, leaf, property,
    selected, text, value_events, widget,
};
use super::{CATEGORY_APPEARANCE, CATEGORY_BEHAVIOR, CATEGORY_DATA, Children, WidgetSpec};
use crate::value::{Value, ValueType};

/// The built-in widget specs, grounded in the `xui-core` widget APIs.
pub(super) fn widget_specs() -> Vec<WidgetSpec> {
    let index = int_from(None);
    let click = |description: &str| vec![event("Click", Vec::new(), true, description)];
    let toggle = || {
        vec![event(
            "Toggle",
            vec![arg("checked", ValueType::Bool)],
            true,
            "Raised with the new checked state.",
        )]
    };
    let select = |description: &str| {
        vec![event(
            "Select",
            vec![arg("index", index.clone())],
            true,
            description,
        )]
    };
    let change = |multiline: bool| {
        vec![event(
            "Change",
            vec![arg("text", ValueType::Text { multiline })],
            true,
            "Raised with the new text as the user types.",
        )]
    };
    let range = || {
        vec![
            float("value", 0.0, "The current value."),
            float("min", 0.0, "The smallest allowed value."),
            float("max", 100.0, "The largest allowed value."),
        ]
    };
    vec![
        leaf(
            "Label",
            "A static text label.",
            (120.0, 20.0),
            vec![text("text", CATEGORY_APPEARANCE, "The label's text.")],
            Vec::new(),
        ),
        leaf(
            "Button",
            "A push button.",
            (100.0, 28.0),
            vec![text("text", CATEGORY_APPEARANCE, "The button's caption.")],
            click("Raised when the button is pressed."),
        ),
        leaf(
            "CheckBox",
            "A labelled check box.",
            (120.0, 24.0),
            vec![
                text("text", CATEGORY_APPEARANCE, "The box's label."),
                checked("Whether the box is checked."),
            ],
            toggle(),
        ),
        leaf(
            "ToggleButton",
            "A button that latches a checked state.",
            (100.0, 28.0),
            vec![
                text("text", CATEGORY_APPEARANCE, "The button's label."),
                checked("Whether the button is latched down."),
            ],
            toggle(),
        ),
        leaf(
            "RadioGroup",
            "A vertical set of radio options.",
            (160.0, 96.0),
            vec![
                items("The option labels, top to bottom."),
                selected("The selected option index."),
            ],
            select("Raised with the newly selected index."),
        ),
        leaf(
            "Edit",
            "A single-line text field.",
            (160.0, 24.0),
            vec![
                text("text", CATEGORY_DATA, "The field's text."),
                text(
                    "cue",
                    CATEGORY_APPEARANCE,
                    "A placeholder shown while the field is empty.",
                ),
            ],
            change(false),
        ),
        leaf(
            "MultilineEdit",
            "A multi-line text area.",
            (200.0, 100.0),
            vec![property(
                "text",
                ValueType::Text { multiline: true },
                Value::Text(String::new()),
                CATEGORY_DATA,
                "The text area's text.",
            )],
            change(true),
        ),
        leaf(
            "NumberField",
            "A numeric field with steppers.",
            (120.0, 28.0),
            {
                let mut properties = range();
                properties.push(design(
                    "step",
                    float_type(),
                    Value::Float(1.0),
                    CATEGORY_BEHAVIOR,
                    "The step used by the steppers and arrow keys.",
                ));
                properties
            },
            value_events(
                "Raised with the new value as it changes.",
                "Raised with the value when it is committed.",
            ),
        ),
        leaf(
            "Slider",
            "A horizontal range control.",
            (160.0, 24.0),
            range(),
            value_events(
                "Raised with the new value while dragging.",
                "Raised with the value when the drag ends.",
            ),
        ),
        leaf(
            "ProgressBar",
            "A read-only progress indicator.",
            (160.0, 12.0),
            vec![
                property(
                    "value",
                    int_from(None),
                    Value::Int(0),
                    CATEGORY_DATA,
                    "The current value.",
                ),
                property(
                    "max",
                    int_from(Some(1)),
                    Value::Int(100),
                    CATEGORY_DATA,
                    "The upper bound of the range `0..=max`.",
                ),
            ],
            Vec::new(),
        ),
        leaf(
            "ComboBox",
            "A drop-down list of choices.",
            (160.0, 24.0),
            vec![
                items("The choices in the drop-down."),
                selected("The selected index."),
            ],
            select("Raised with the chosen index."),
        ),
        leaf(
            "ListView",
            "A list of rows with a selection.",
            (240.0, 140.0),
            vec![
                property(
                    "items",
                    ValueType::List,
                    Value::List(Vec::new()),
                    CATEGORY_DATA,
                    "The rows, one per line. A script may replace them at runtime.",
                ),
                property(
                    "selected",
                    int_from(None),
                    Value::Int(-1),
                    CATEGORY_DATA,
                    "The selected row, or -1 for none.",
                ),
                design(
                    "multi_select",
                    ValueType::Bool,
                    Value::Bool(false),
                    CATEGORY_BEHAVIOR,
                    "Whether more than one row may be selected.",
                ),
            ],
            {
                let mut events = select("Raised with the primary selected row.");
                events.push(event(
                    "Activate",
                    vec![arg("index", index.clone())],
                    false,
                    "Raised when a row is activated (Return or a double-click).",
                ));
                events
            },
        ),
        widget(
            "GroupBox",
            "A titled frame that groups widgets.",
            (200.0, 120.0),
            vec![text("text", CATEGORY_APPEARANCE, "The frame's title.")],
            Vec::new(),
            Children::Any,
        ),
        widget(
            "Panel",
            "A container that owns child widgets.",
            (200.0, 120.0),
            Vec::new(),
            Vec::new(),
            Children::Any,
        ),
        leaf(
            "Separator",
            "A divider line.",
            (120.0, 8.0),
            vec![design(
                "orientation",
                enum_type(&["horizontal", "vertical"]),
                Value::Enum("horizontal".to_owned()),
                CATEGORY_APPEARANCE,
                "The direction the divider runs.",
            )],
            Vec::new(),
        ),
        leaf(
            "Hyperlink",
            "A clickable link label.",
            (120.0, 20.0),
            vec![text("text", CATEGORY_APPEARANCE, "The link's text.")],
            click("Raised when the link is clicked."),
        ),
    ]
}
