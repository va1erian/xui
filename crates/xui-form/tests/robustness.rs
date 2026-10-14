//! Inputs that must never panic: empty forms, every prefix of a form, names
//! used twice, and the reader's state after it has failed.

mod common;

use common::{form, with_form};
use xui_form::migrate::from_v1;
use xui_form::model::Node;
use xui_form::{BuildOptions, Catalog, Form, MAX_DEPTH, format, load};

/// A form with every kind of node, strings with multi-byte characters, a
/// comment and a number of each shape.
const SAMPLE: &str = r#"// héllo
Form(name: "main_form", title: "Grüße \"quoted\"", size: (360, 200), root: Column(gap: 8.5, children: [
    Row(children: [
        Edit(name: "name_edit", placeholder: "Né", fill: 1),
        Button(name: "digit", array: 3, text: "{index} é"),
    ]),
    Grid(columns: [Auto, Fixed(80), Fill(2)], children: [Label(text: "a", span: 2)]),
    Absolute(size: (100, 40), children: [Slider(name: "s", at: (4, 4, 80, 20), anchor: StretchHorizontal)]),
    Tabs(name: "tabs", selected: 1, pages: [
        Page(title: "One", content: Panel(card: true, content: Label(name: "l"))),
        Page(title: "Two", content: Group(text: "G", content: ListView(items: ["x", "ÿ"], selected: 1))),
    ]),
]))
"#;

/// A format-1 form with a group, a panel, anchors and a tab order.
const SAMPLE_V1: &str = "format = 1\n[window]\nname = \"main\"\ntitle = \"Grüße\"\nwidth = 320\nheight = 200\n\
    [[node]]\nkind = \"GroupBox\"\nname = \"box\"\nleft = 8\ntop = 8\nwidth = 200\nheight = 100\ntext = \"é\"\n\
    [[node]]\nkind = \"Edit\"\nname = \"in_box\"\nparent = \"box\"\nleft = 8\ntop = 20\ncue = \"ü\"\ntab_index = 1\n\
    [[node]]\nkind = \"Button\"\nname = \"ok\"\nleft = 220\ntop = 160\nanchor = \"bottom_right\"\n";

#[test]
fn the_sample_forms_are_valid() {
    let form = load(SAMPLE).expect("the sample loads");
    assert!(form.validate(&Catalog::xui()).iter().all(|d| !d.is_error()));
    assert!(from_v1(SAMPLE_V1).is_ok());
}

#[test]
fn no_prefix_of_a_form_panics_the_reader() {
    // Cut at every character: whatever it is, it is a form or an error.
    for (end, _) in SAMPLE.char_indices() {
        let text = &SAMPLE[..end];
        let _ = load(text);
        let _ = format(text);
    }
}

#[test]
fn no_prefix_of_a_v1_form_panics_the_conversion() {
    for (end, _) in SAMPLE_V1.char_indices() {
        let _ = from_v1(&SAMPLE_V1[..end]);
    }
}

#[test]
fn an_empty_form_round_trips_validates_and_builds() {
    let empty = Form::default();
    let text = empty.to_ron().expect("an empty form is written");
    assert_eq!(text, "Form(size: (320, 200), root: Column())\n");
    assert_eq!(load(&text).expect("it loads"), empty);
    assert_eq!(load("Form()").expect("defaults load"), empty);
    assert!(empty.validate(&Catalog::xui()).is_empty());
    with_form(&empty, BuildOptions::default(), |live| {
        assert_eq!(live.controls().names().count(), 0);
    });

    // Every container and layout with nothing in it.
    for root in [
        "Row()",
        "Wrap()",
        "Absolute()",
        "Panel()",
        "Group()",
        "Tabs()",
    ] {
        let text = format!("Form(root: {root})");
        let form = load(&text).unwrap_or_else(|error| panic!("{root}: {error}"));
        let diagnostics = form.validate(&Catalog::xui());
        assert!(diagnostics.is_empty(), "{root}: {diagnostics:?}");
        let written = form.to_ron().expect("it is written");
        assert_eq!(load(&written).expect("it loads again"), form, "{root}");
        with_form(&form, BuildOptions::default(), |_| ());
    }
}

#[test]
fn an_empty_format_one_form_converts_to_an_empty_layout() {
    let migration = from_v1("format = 1\n[window]\nname = \"empty\"\n").expect("it converts");
    let Node::Absolute(root) = &migration.form.root else {
        panic!("no nodes, no grid: {:?}", migration.form.root);
    };
    assert!(root.children.is_empty());
    assert!(migration.notes.is_empty());
    let text = migration.form.to_ron().expect("it is written");
    assert_eq!(load(&text).expect("it loads"), migration.form);
}

#[test]
fn a_name_used_twice_builds_with_the_first_one() {
    let twice = form(
        r#"Form(root: Column(children: [
            Label(name: "a", text: "one"), Label(name: "a", text: "two"),
        ]))"#,
    );
    assert!(twice.validate(&Catalog::xui()).iter().any(|d| d.is_error()));
    with_form(&twice, BuildOptions::default(), |live| {
        assert_eq!(live.controls().names().count(), 1);
        assert_eq!(
            live.get("a", "text"),
            Some(xui_form::Value::Text("one".to_owned()))
        );
    });
}

#[test]
fn a_failed_load_leaves_the_reader_as_it_was() {
    // The reader counts the nodes it is inside; a failure at any depth must
    // leave that count at zero, or valid forms would soon be refused.
    let mut deep = String::from("Form(root: ");
    deep.push_str(&"Column(children: [".repeat(MAX_DEPTH + 4));
    deep.push_str(&"])".repeat(MAX_DEPTH + 4));
    deep.push(')');
    let mut misspelt = String::from("Form(root: ");
    misspelt.push_str(&"Column(children: [".repeat(MAX_DEPTH - 1));
    misspelt.push_str("Button(txt: \"x\")");
    misspelt.push_str(&"])".repeat(MAX_DEPTH - 1));
    misspelt.push(')');

    let mut at_limit = String::from("Form(root: ");
    at_limit.push_str(&"Column(children: [".repeat(MAX_DEPTH - 1));
    at_limit.push_str("Label()");
    at_limit.push_str(&"])".repeat(MAX_DEPTH - 1));
    at_limit.push(')');

    for _ in 0..(MAX_DEPTH * 3) {
        assert!(load(&deep).is_err());
        assert!(load(&misspelt).is_err());
        assert_eq!(
            load(&at_limit)
                .expect("a form at the limit loads")
                .root
                .depth(),
            MAX_DEPTH
        );
    }
}
