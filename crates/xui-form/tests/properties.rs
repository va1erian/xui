//! Reading and writing every built-in kind's properties on a live form.

mod common;

use common::{form, with_form};
use xui_form::{BuildOptions, SetError, Value};

/// One widget of every kind.
const ALL_KINDS: &str = r#"Form(size: (640, 900), root: Column(children: [
    Label(name: "label", text: "one"),
    Button(name: "button", text: "go"),
    Hyperlink(name: "link", text: "link"),
    CheckBox(name: "check", text: "check"),
    ToggleButton(name: "toggle", text: "toggle"),
    Edit(name: "edit", text: "edit", placeholder: "type"),
    MultilineEdit(name: "memo", text: "multi"),
    NumberField(name: "number", value: 5, max: 10),
    Slider(name: "slider", value: 5, max: 10),
    ProgressBar(name: "progress", value: 5, max: 10),
    RadioGroup(name: "radio", items: ["a", "b"]),
    ComboBox(name: "combo", items: ["x", "y"], selected: 1),
    ListView(name: "list", items: ["r1", "r2"], selected: 1, multi_select: true),
    Group(name: "group", text: "frame"),
    Panel(name: "panel"),
    Separator(name: "rule", orientation: Vertical),
]))"#;

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

#[test]
fn get_and_set_every_builtin_kind() {
    with_form(&form(ALL_KINDS), BuildOptions::default(), |live| {
        assert_eq!(live.controls().ids().len(), 16);
        for name in [
            "label", "button", "link", "check", "toggle", "edit", "memo", "group",
        ] {
            live.set(name, "text", &text("changed"))
                .expect("text is writable");
            assert_eq!(live.get(name, "text"), Some(text("changed")), "{name}");
        }
        for name in ["check", "toggle"] {
            live.set(name, "checked", &Value::Bool(true))
                .expect("checked is writable");
            assert_eq!(live.get(name, "checked"), Some(Value::Bool(true)));
        }
        for (name, index) in [("combo", 0), ("radio", 1), ("list", 0), ("list", -1)] {
            live.set(name, "selected", &Value::Int(index))
                .expect("selected is writable");
            assert_eq!(live.get(name, "selected"), Some(Value::Int(index)));
        }
        live.set("number", "value", &Value::Int(3))
            .expect("an int sets a float");
        assert_eq!(live.get("number", "value"), Some(Value::Float(3.0)));
        live.set("number", "max", &Value::Float(20.0))
            .expect("max is writable");
        assert_eq!(live.get("number", "max"), Some(Value::Float(20.0)));
        live.set("slider", "value", &Value::Float(8.0))
            .expect("value is writable");
        assert_eq!(live.get("slider", "value"), Some(Value::Float(8.0)));
        live.set("progress", "value", &Value::Int(7))
            .expect("value is writable");
        assert_eq!(live.get("progress", "value"), Some(Value::Int(7)));
        let rows = Value::List(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        live.set("list", "items", &rows).expect("items is writable");
        assert_eq!(live.get("list", "items"), Some(rows));

        // Construction-only properties are read-only at runtime.
        let readonly = Err(SetError::ReadOnly);
        assert_eq!(live.set("radio", "items", &Value::List(vec![])), readonly);
        assert_eq!(live.set("edit", "placeholder", &text("x")), readonly);
        let horizontal = Value::Enum("Horizontal".to_owned());
        assert_eq!(live.set("rule", "orientation", &horizontal), readonly);

        live.set("button", "visible", &Value::Bool(false))
            .expect("visible is writable");
        assert_eq!(live.get("button", "visible"), Some(Value::Bool(false)));
        assert_eq!(
            live.set("button", "nope", &Value::Bool(true)),
            Err(SetError::UnknownProperty)
        );
        assert_eq!(
            live.set("button", "text", &Value::Int(1)),
            Err(SetError::TypeMismatch)
        );
    });
}

#[test]
fn construction_only_properties_are_readable_in_design_mode() {
    let options = BuildOptions {
        design_mode: true,
        ..BuildOptions::default()
    };
    with_form(&form(ALL_KINDS), options, |live| {
        assert_eq!(
            live.get("rule", "orientation"),
            Some(Value::Enum("Vertical".to_owned()))
        );
        assert_eq!(live.get("edit", "placeholder"), Some(text("type")));
        assert_eq!(
            live.get("radio", "items"),
            Some(Value::List(vec!["a".to_owned(), "b".to_owned()]))
        );
        assert_eq!(live.controls().kind("list"), Some("ListView"));
    });
}
