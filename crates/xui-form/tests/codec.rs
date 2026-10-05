//! Reading and writing `.lfm` format 2.

use xui_form::model::{Length, Node};
use xui_form::{format, load};

/// A canonical form: what `to_ron` writes for it.
const CANONICAL: &str = r#"Form(title: "Hello", size: (360, 160), root: Column(gap: 8, padding: 16, children: [
    Row(gap: 8, children: [
        Edit(name: "name_edit", placeholder: "Your name", fill: 1),
        Button(name: "greet_button", text: "Greet"),
    ]),
    Label(name: "result_label"),
    Grid(columns: [
        Auto,
        Fill(1),
    ], children: [
        Button(name: "digit", array: 10, text: "{index}"),
    ]),
]))
"#;

#[test]
fn a_canonical_form_round_trips_byte_for_byte() {
    let form = load(CANONICAL).expect("the form loads");
    assert_eq!(form.to_ron(), CANONICAL);
    assert_eq!(format(CANONICAL).expect("it formats"), CANONICAL);
}

#[test]
fn a_hand_written_form_formats_canonically() {
    let hand = r#"Form(size: (360, 160), title: "Hello", root: Column(padding: 16, gap: 8,
        children: [Row(gap: 8, children: [Edit(name: "name_edit", placeholder: "Your name",
        fill: 1, text: ""), Button(name: "greet_button", text: "Greet", enabled: true)]),
        Label(name: "result_label"), Grid(columns: [Auto, Fill(1)], children: [
        Button(name: "digit", array: 10, text: "{index}")])]))"#;
    assert_eq!(format(hand).expect("it formats"), CANONICAL);
}

#[test]
fn defaults_are_omitted_and_lengths_keep_their_fraction() {
    let form = load(r#"Form(root: Column(gap: 2.5, children: [Label(text: "")]))"#)
        .expect("the form loads");
    let Node::Column(column) = &form.root else {
        panic!("the root is a column");
    };
    assert_eq!(column.gap, Length(2.5));
    let text = form.to_ron();
    assert!(text.contains("gap: 2.5,"), "{text}");
    assert!(text.contains("Label()"), "an all-default label: {text}");
    assert!(text.contains("size: (320, 200)"), "{text}");
}

#[test]
fn an_unknown_field_suggests_the_closest() {
    let error = load(r#"Form(root: Button(txt: "OK"))"#).expect_err("txt is not a field");
    assert_eq!(error.suggestion.as_deref(), Some("text"));
    assert_eq!((error.line, error.column), (1, 19));
    assert!(
        error.to_string().contains("did you mean `text`?"),
        "{error}"
    );
}

#[test]
fn an_unknown_kind_suggests_the_closest() {
    let error = load("Form(root: Colum(children: []))").expect_err("Colum is not a kind");
    assert_eq!(error.suggestion.as_deref(), Some("Column"));
}

#[test]
fn a_mistyped_value_is_located() {
    let error = load("Form(\n    root: CheckBox(checked: 1),\n)").expect_err("1 is not a bool");
    assert_eq!(error.line, 2);
    assert_eq!(error.suggestion, None);
}

#[test]
fn a_format_one_file_points_at_migrate() {
    let error = load("format = 1\n\n[window]\nname = \"main\"\n").expect_err("TOML is format 1");
    assert!(error.message.contains("xui-form migrate"), "{error}");
}
