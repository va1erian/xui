#![forbid(unsafe_code)]

//! The widget nodes: one declaration per portable `xui-core` widget, which is
//! both its file format and its schema.

use super::Orientation;
use super::macros::widget;
use crate::value::ValueType;

/// An int type from `min` up.
fn at_least(min: i64) -> ValueType {
    ValueType::Int {
        min: Some(min),
        max: None,
    }
}

widget! {
    /// A static text label.
    Label {
        /// The label's text.
        text: String = String::new(); Appearance,
    }
    events {}
}

widget! {
    /// A push button.
    Button {
        /// The button's caption.
        text: String = String::new(); Appearance,
    }
    events {
        /// Raised when the button is pressed.
        Click(),
    }
}

widget! {
    /// A clickable link label.
    Hyperlink {
        /// The link's text.
        text: String = String::new(); Appearance,
    }
    events {
        /// Raised when the link is clicked.
        Click(),
    }
}

widget! {
    /// A labelled check box.
    CheckBox {
        /// The box's label.
        text: String = String::new(); Appearance,
        /// Whether the box is checked.
        checked: bool = false; Data,
    }
    events {
        /// Raised with the new checked state.
        Toggle(checked: bool),
    }
}

widget! {
    /// A button that latches a checked state.
    ToggleButton {
        /// The button's label.
        text: String = String::new(); Appearance,
        /// Whether the button is latched down.
        checked: bool = false; Data,
    }
    events {
        /// Raised with the new checked state.
        Toggle(checked: bool),
    }
}

widget! {
    /// A single-line text field.
    Edit {
        /// The field's text.
        text: String = String::new(); Data,
        /// A placeholder shown while the field is empty.
        placeholder: String = String::new(); Appearance (design),
        /// Masks the text, for a password.
        password: bool = false; Behavior (design),
    }
    events {
        /// Raised with the new text as the user types.
        Change(text: String),
    }
}

widget! {
    /// A multi-line text area.
    MultilineEdit {
        /// The text area's text.
        text: String = String::new(); Data => ValueType::Text { multiline: true },
    }
    events {
        /// Raised with the new text as the user types.
        Change(text: String),
    }
}

widget! {
    /// A numeric field with steppers.
    NumberField {
        /// The current value.
        value: f64 = 0.0; Data,
        /// The smallest allowed value.
        min: f64 = 0.0; Data,
        /// The largest allowed value.
        max: f64 = 100.0; Data,
        /// The step used by the steppers and arrow keys.
        step: f64 = 1.0; Behavior (design),
    }
    events {
        /// Raised with the new value as it changes.
        Change(value: f64),
        /// Raised with the value when it is committed.
        Commit(value: f64),
    }
}

widget! {
    /// A horizontal range control.
    Slider {
        /// The current value.
        value: f64 = 0.0; Data,
        /// The smallest allowed value.
        min: f64 = 0.0; Data,
        /// The largest allowed value.
        max: f64 = 100.0; Data,
    }
    events {
        /// Raised with the new value while dragging.
        Change(value: f64),
        /// Raised with the value when the drag ends.
        Commit(value: f64),
    }
}

widget! {
    /// A read-only progress indicator.
    ProgressBar {
        /// The current value.
        value: i64 = 0; Data,
        /// The upper bound of the range `0..=max`.
        max: i64 = 100; Data => at_least(1),
    }
    events {}
}

widget! {
    /// A vertical set of radio options.
    RadioGroup {
        /// The option labels, top to bottom.
        items: Vec<String> = Vec::new(); Data (design),
        /// The selected option index.
        selected: i64 = 0; Data => at_least(0),
    }
    events {
        /// Raised with the newly selected index.
        Select(index: i64),
    }
}

widget! {
    /// A drop-down list of choices.
    ComboBox {
        /// The choices in the drop-down.
        items: Vec<String> = Vec::new(); Data (design),
        /// The selected index.
        selected: i64 = 0; Data => at_least(0),
    }
    events {
        /// Raised with the chosen index.
        Select(index: i64),
    }
}

widget! {
    /// A list of rows with a selection.
    ListView {
        /// The rows, one per line. A script may replace them at runtime.
        items: Vec<String> = Vec::new(); Data,
        /// The selected row, or -1 for none.
        selected: i64 = -1; Data,
        /// Whether more than one row may be selected.
        multi_select: bool = false; Behavior (design),
    }
    events {
        /// Raised with the primary selected row.
        Select(index: i64),
        /// Raised when a row is activated (Return or a double-click).
        Activate(index: i64),
    }
}

widget! {
    /// A divider line.
    Separator {
        /// The direction the divider runs.
        orientation: Orientation = Orientation::Horizontal; Appearance (design),
    }
    events {}
}
