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
    assert_eq!(form.to_ron().expect("it writes"), CANONICAL);
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
    let text = form.to_ron().expect("it writes");
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

#[test]
fn a_byte_order_mark_is_ignored_and_not_counted() {
    let marked = format!("\u{feff}{CANONICAL}");
    assert_eq!(load(&marked), load(CANONICAL));
    assert_eq!(format(&marked).expect("it formats"), CANONICAL);
    let error = load("\u{feff}Form(root: Button(txt: \"OK\"))").expect_err("txt is not a field");
    assert_eq!((error.line, error.column), (1, 19), "{error}");
}

#[test]
fn crlf_line_endings_load_and_format_to_lf() {
    let crlf = CANONICAL.replace('\n', "\r\n");
    assert_eq!(load(&crlf), load(CANONICAL));
    assert_eq!(format(&crlf).expect("it formats"), CANONICAL);
}

#[test]
fn a_commented_format_one_file_points_at_migrate_too() {
    for text in [
        "# my form\n\nformat = 1\n[window]\nname = \"main\"\n",
        "   # indented comment\n#\n[window]\nname = \"main\"\n",
        "\u{feff}# generated\n[[node]]\nkind = \"Button\"\nname = \"ok\"\n",
    ] {
        let error = load(text).expect_err("TOML is format 1");
        assert!(error.message.contains("xui-form migrate"), "{error}");
    }
    let error = load("# my form\n\nformat = 1\n").expect_err("TOML is format 1");
    assert_eq!(
        (error.line, error.column),
        (3, 1),
        "at the first line of it"
    );

    // Comments and attributes of a format-2 file are not mistaken for it.
    assert!(load("// my form\nForm()").is_ok());
    assert!(load("#![enable(implicit_some)]\nForm()").is_ok());
}

#[test]
fn a_negative_number_is_located_at_its_sign() {
    let error = load("Form(root: Button(fill: -1))").expect_err("fill is unsigned");
    assert_eq!((error.line, error.column), (1, 25), "{error}");
    let error = load("Form(\n  root: Button(array: -3),\n)").expect_err("array is unsigned");
    assert_eq!((error.line, error.column), (2, 23), "{error}");
}

#[test]
fn a_length_that_is_not_finite_or_is_huge_is_refused_where_it_is() {
    for (text, column) in [
        ("Form(root: Column(gap: 1e38))", 24),
        ("Form(root: Column(gap: NaN))", 24),
        ("Form(root: Column(gap: inf))", 24),
        ("Form(root: Column(gap: -inf))", 24),
        ("Form(root: Column(gap: 32768.5))", 24),
        ("Form(root: Column(padding: 9223372036854775807))", 28),
        ("Form(size: (1e38, 100), root: Column())", 13),
        ("Form(root: Button(width: 1e38))", 26),
        (
            "Form(root: Absolute(children: [Button(at: (1e38, 0, 10, 10))]))",
            44,
        ),
        ("Form(root: Grid(columns: [Fixed(1e30)]))", 33),
    ] {
        let error = load(text).expect_err(text);
        assert!(error.message.contains("is not a length"), "{text}: {error}");
        assert_eq!((error.line, error.column), (1, column), "{text}: {error}");
    }
    // The limits themselves are lengths.
    let form = load("Form(size: (32768, 100), root: Column(gap: -32768))").expect("at the limit");
    assert_eq!(form.size.0, Length(32768.0));
}
