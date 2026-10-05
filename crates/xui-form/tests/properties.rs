//! Reading and writing every built-in kind's properties on a live form.

mod common;

use xui_form::{BuildOptions, Catalog, FormDoc, Node, Value};

use common::with_form;

/// Builds a form containing one node of every built-in kind.
fn all_kinds_doc() -> FormDoc {
    let mut doc = FormDoc::new("main_form");
    let mut push = |kind: &str, name: &str, props: &[(&str, Value)]| {
        let mut node = Node::new(kind, name);
        node.set_prop("width", Value::Int(120));
        node.set_prop("height", Value::Int(30));
        for (property, value) in props {
            node.set_prop(*property, value.clone());
        }
        doc.insert(node);
    };

    push(
        "Label",
        "lblOne",
        &[("text", Value::Text("one".to_owned()))],
    );
    push(
        "Button",
        "cmdOne",
        &[("text", Value::Text("go".to_owned()))],
    );
    push(
        "CheckBox",
        "chkOne",
        &[
            ("text", Value::Text("check".to_owned())),
            ("checked", Value::Bool(false)),
        ],
    );
    push(
        "ToggleButton",
        "tglOne",
        &[
            ("text", Value::Text("toggle".to_owned())),
            ("checked", Value::Bool(false)),
        ],
    );
    push(
        "RadioGroup",
        "radOne",
        &[
            ("items", Value::List(vec!["a".to_owned(), "b".to_owned()])),
            ("selected", Value::Int(0)),
        ],
    );
    push(
        "Edit",
        "txtOne",
        &[
            ("text", Value::Text("edit".to_owned())),
            ("cue", Value::Text("type".to_owned())),
        ],
    );
    push(
        "MultilineEdit",
        "mmoOne",
        &[("text", Value::Text("multi".to_owned()))],
    );
    push(
        "NumberField",
        "numOne",
        &[
            ("value", Value::Float(5.0)),
            ("min", Value::Float(0.0)),
            ("max", Value::Float(10.0)),
            ("step", Value::Float(1.0)),
        ],
    );
    push(
        "Slider",
        "sldOne",
        &[
            ("value", Value::Float(5.0)),
            ("min", Value::Float(0.0)),
            ("max", Value::Float(10.0)),
        ],
    );
    push(
        "ProgressBar",
        "prgOne",
        &[("value", Value::Int(5)), ("max", Value::Int(10))],
    );
    push(
        "ComboBox",
        "cboOne",
        &[
            ("items", Value::List(vec!["x".to_owned(), "y".to_owned()])),
            ("selected", Value::Int(1)),
        ],
    );
    push(
        "ListView",
        "lstOne",
        &[
            ("items", Value::List(vec!["r1".to_owned(), "r2".to_owned()])),
            ("selected", Value::Int(1)),
            ("multi_select", Value::Bool(true)),
        ],
    );
    push(
        "GroupBox",
        "fraOne",
        &[("text", Value::Text("frame".to_owned()))],
    );
    push("Panel", "panOne", &[]);
    push(
        "Separator",
        "sepOne",
        &[("orientation", Value::Enum("vertical".to_owned()))],
    );
    push(
        "Hyperlink",
        "lnkOne",
        &[("text", Value::Text("link".to_owned()))],
    );
    doc
}

#[test]
fn get_and_set_every_builtin_kind() {
    let doc = all_kinds_doc();
    let catalog = Catalog::xui();
    with_form(&doc, &catalog, BuildOptions::default(), |form| {
        assert_eq!(form.ids().len(), 16);

        // Text widgets round trip through their `text` property.
        for name in [
            "lblOne", "cmdOne", "chkOne", "tglOne", "txtOne", "mmoOne", "fraOne", "lnkOne",
        ] {
            form.set(name, "text", &Value::Text("changed".to_owned()))
                .expect("text is writable");
            assert_eq!(
                form.get(name, "text"),
                Some(Value::Text("changed".to_owned()))
            );
        }

        // Checked widgets.
        form.set("chkOne", "checked", &Value::Bool(true))
            .expect("checked is writable");
        assert_eq!(form.get("chkOne", "checked"), Some(Value::Bool(true)));
        form.set("tglOne", "checked", &Value::Bool(true))
            .expect("checked is writable");
        assert_eq!(form.get("tglOne", "checked"), Some(Value::Bool(true)));

        // Single-node integer selection.
        form.set("cboOne", "selected", &Value::Int(0))
            .expect("selected is writable");
        assert_eq!(form.get("cboOne", "selected"), Some(Value::Int(0)));

        // List selection, including clearing it.
        form.set("lstOne", "selected", &Value::Int(0))
            .expect("selected is writable");
        assert_eq!(form.get("lstOne", "selected"), Some(Value::Int(0)));
        form.set("lstOne", "selected", &Value::Int(-1))
            .expect("clearing is allowed");
        assert_eq!(form.get("lstOne", "selected"), Some(Value::Int(-1)));

        // Radio group selection.
        form.set("radOne", "selected", &Value::Int(1))
            .expect("selected is writable");
        assert_eq!(form.get("radOne", "selected"), Some(Value::Int(1)));

        // Numeric fields.
        form.set("numOne", "value", &Value::Float(3.0))
            .expect("value is writable");
        assert_eq!(form.get("numOne", "value"), Some(Value::Float(3.0)));
        form.set("numOne", "max", &Value::Float(20.0))
            .expect("max is writable");
        assert_eq!(form.get("numOne", "max"), Some(Value::Float(20.0)));
        form.set("sldOne", "value", &Value::Float(8.0))
            .expect("value is writable");
        assert_eq!(form.get("sldOne", "value"), Some(Value::Float(8.0)));

        // Progress bar.
        form.set("prgOne", "value", &Value::Int(7))
            .expect("value is writable");
        assert_eq!(form.get("prgOne", "value"), Some(Value::Int(7)));

        // Construction-only properties are read-only at runtime.
        assert!(form.set("radOne", "items", &Value::List(vec![])).is_err());
        assert!(
            form.set(
                "sepOne",
                "orientation",
                &Value::Enum("horizontal".to_owned())
            )
            .is_err()
        );
        assert!(
            form.set("txtOne", "cue", &Value::Text("x".to_owned()))
                .is_err()
        );

        // Common properties.
        form.set("cmdOne", "anchor", &Value::Enum("fill".to_owned()))
            .expect("anchor is writable");
        assert_eq!(
            form.get("cmdOne", "anchor"),
            Some(Value::Enum("fill".to_owned()))
        );
        form.set("cmdOne", "visible", &Value::Bool(false))
            .expect("visible is writable");
        assert_eq!(form.get("cmdOne", "visible"), Some(Value::Bool(false)));
        form.set("cmdOne", "tab_index", &Value::Int(3))
            .expect("tab_index is writable");
        assert_eq!(form.get("cmdOne", "tab_index"), Some(Value::Int(3)));

        // An unknown property is an error.
        assert!(form.set("cmdOne", "nope", &Value::Bool(true)).is_err());
    });
}

#[test]
fn construction_only_properties_are_readable_in_design_mode() {
    let doc = all_kinds_doc();
    let catalog = Catalog::xui();
    with_form(
        &doc,
        &catalog,
        BuildOptions {
            design_mode: true,
            ..BuildOptions::default()
        },
        |form| {
            assert_eq!(
                form.get("sepOne", "orientation"),
                Some(Value::Enum("vertical".to_owned()))
            );
            assert_eq!(
                form.get("txtOne", "cue"),
                Some(Value::Text("type".to_owned()))
            );
            assert_eq!(
                form.get("radOne", "items"),
                Some(Value::List(vec!["a".to_owned(), "b".to_owned()]))
            );
        },
    );
}
