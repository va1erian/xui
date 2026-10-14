//! Validating a well-typed form.

use xui_form::{Catalog, Diagnostic, Severity, load};

fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let form = load(text).unwrap_or_else(|error| panic!("the form loads: {error}"));
    form.validate(&Catalog::xui())
}

fn has(diagnostics: &[Diagnostic], severity: Severity, needle: &str) -> bool {
    diagnostics
        .iter()
        .any(|d| d.severity == severity && d.message.contains(needle))
}

#[test]
fn a_valid_form_has_no_diagnostics() {
    let found = diagnostics(
        r#"Form(name: "main_form", root: Column(children: [
            Edit(name: "name_edit"),
            Grid(columns: [Auto, Fill(1)], children: [
                Label(text: "Size"), Slider(name: "size_slider", span: 1),
            ]),
            Absolute(children: [Button(name: "ok", at: (8, 8, 80, 28), anchor: BottomRight)]),
            Row(children: [Button(name: "digit", index: 0), Button(name: "digit", index: 1)]),
        ]))"#,
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn names_must_be_identifiers_and_unique() {
    let found = diagnostics(
        r#"Form(name: "main form", root: Column(children: [
            Label(name: "1st"), Button(name: "go"), Button(name: "go"),
        ]))"#,
    );
    assert!(has(&found, Severity::Error, "`1st` is not a valid name"));
    assert!(has(&found, Severity::Error, "`go` is used more than once"));
    assert!(has(&found, Severity::Error, "not a valid form name"));
}

#[test]
fn control_arrays_must_not_clash() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            Button(name: "digit", array: 3), Button(name: "digit", index: 2),
            Button(name: "other", array: 2, index: 1), Button(array: 2),
        ]))"#,
    );
    assert!(
        has(&found, Severity::Error, "`digit[2]` is used more than once"),
        "{found:?}"
    );
    assert!(has(&found, Severity::Error, "either `array` or `index`"));
    assert!(has(&found, Severity::Error, "needs a name"));
}

#[test]
fn values_out_of_range_are_errors() {
    let found = diagnostics(r#"Form(root: ComboBox(name: "pick", selected: -1))"#);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].node.as_deref(), Some("pick"));
    assert_eq!(found[0].property.as_deref(), Some("selected"));
}

#[test]
fn layout_fields_the_parent_ignores_are_warnings() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            Button(name: "a", at: (0, 0, 10, 10), anchor: Fill, span: 2),
            Absolute(children: [Label(name: "loose")]),
            Grid(),
        ]))"#,
    );
    assert!(has(&found, Severity::Warning, "`at` only places"));
    assert!(has(&found, Severity::Warning, "`anchor` only anchors"));
    assert!(has(&found, Severity::Warning, "`span` only spans"));
    assert!(has(&found, Severity::Warning, "`at` is missing"));
    assert!(has(&found, Severity::Error, "needs at least one column"));
}

#[test]
fn a_control_array_is_bounded() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            Button(name: "ok_array", array: 1024), Button(name: "ok_index", index: 1023),
            Button(name: "big", array: 1025), Button(name: "high", index: 1024),
        ]))"#,
    );
    assert!(has(&found, Severity::Error, "`array` is 1025"), "{found:?}");
    assert!(has(&found, Severity::Error, "`index` is 1024"), "{found:?}");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].node.as_deref(), Some("big"));
    assert_eq!(found[0].property.as_deref(), Some("array"));
}

#[test]
fn a_huge_array_or_index_is_reported_without_being_built_or_enumerated() {
    // 100 million elements took 770 MB to check, and u32::MAX would have
    // been 32 GB: the count is a range, never a list.
    let found = diagnostics(
        r#"Form(root: Column(children: [
            Button(name: "a", array: 100000000),
            Button(name: "b", array: 4294967295),
            Button(name: "c", index: 20000000),
            Button(name: "d", index: 4294967295),
            Button(name: "a", index: 99999999),
            Button(name: "b", array: 4294967295),
        ]))"#,
    );
    let breaches = found
        .iter()
        .filter(|d| d.message.contains("at most 1024") || d.message.contains("below 1024"))
        .count();
    assert_eq!(breaches, 6, "{found:?}");
    // The ranges are still compared: `b` is used twice, and `a`'s index
    // lies inside its array.
    assert!(
        has(
            &found,
            Severity::Error,
            "`b[0]` to `b[4294967294]` are used more than once"
        ),
        "{found:?}"
    );
    assert!(
        has(
            &found,
            Severity::Error,
            "`a[99999999]` is used more than once"
        ),
        "{found:?}"
    );
}

#[test]
fn an_array_of_zero_elements_is_an_error() {
    let found = diagnostics(r#"Form(root: Button(name: "none", array: 0))"#);
    assert!(has(&found, Severity::Error, "`array` is 0"), "{found:?}");
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_container_cannot_be_a_control_array() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            Panel(name: "panel", array: 2, content: Button(name: "b")),
            Group(name: "group", index: 1),
            Tabs(name: "tabs", array: 3),
            Panel(name: "plain"),
        ]))"#,
    );
    for kind in ["Panel", "Group", "Tabs"] {
        assert!(
            has(&found, Severity::Error, &format!("a {kind} is a container")),
            "{kind}: {found:?}"
        );
    }
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn nested_container_arrays_are_refused_not_multiplied() {
    // Three nested `Panel(array: 30)` built 27000 widgets and ran past 150 s.
    let found = diagnostics(
        r#"Form(root: Panel(array: 30, name: "a", content: Panel(array: 30, name: "b",
            content: Panel(array: 30, name: "c", content: Button(name: "leaf")))))"#,
    );
    assert_eq!(
        found.iter().filter(|d| d.is_error()).count(),
        3,
        "{found:?}"
    );
}

#[test]
fn a_widget_cannot_be_named_form() {
    // A script reaches the form by `form`: a control of that name made
    // `form.title` fail with "unknown property 'title' on form (Button)".
    let found = diagnostics(r#"Form(root: Column(children: [Button(name: "form")]))"#);
    assert!(
        has(&found, Severity::Error, "`form` is reserved"),
        "{found:?}"
    );
    assert_eq!(found[0].node.as_deref(), Some("form"));
    assert!(xui_form::validate::RESERVED_NAMES.contains(&"form"));
}

#[test]
fn a_selection_must_exist() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            ComboBox(name: "pick", items: ["a", "b"], selected: 2),
            RadioGroup(name: "radio", items: ["a"], selected: 1),
            ListView(name: "rows", items: ["a"], selected: 4),
            Tabs(name: "tabs", selected: 2, pages: [Page(title: "One")]),
            ComboBox(name: "ok_combo", items: ["a", "b"], selected: 1),
            ListView(name: "ok_list", items: ["a"]),
            RadioGroup(name: "empty_radio"),
            ComboBox(name: "empty_combo"),
            Tabs(name: "no_pages"),
        ]))"#,
    );
    let on = |name: &str| {
        found
            .iter()
            .filter(|d| d.node.as_deref() == Some(name))
            .count()
    };
    assert_eq!(found.len(), 4, "{found:?}");
    for name in ["pick", "radio", "rows", "tabs"] {
        assert_eq!(on(name), 1, "{name}: {found:?}");
    }
    // A combo box's items are fixed at construction: an error. A list's can
    // be replaced by a script, so its selection may yet be valid: a warning.
    let severity = |name: &str| {
        found
            .iter()
            .find(|d| d.node.as_deref() == Some(name))
            .map(|d| d.severity)
    };
    assert_eq!(severity("pick"), Some(Severity::Error));
    assert_eq!(severity("rows"), Some(Severity::Warning));
    assert!(has(
        &found,
        Severity::Error,
        "`selected` is 2 but there are 2 items"
    ));
    assert!(has(
        &found,
        Severity::Error,
        "`selected` is 1 but there is 1 item"
    ));
    assert!(has(
        &found,
        Severity::Error,
        "`selected` is 2 but there is 1 page"
    ));
}

#[test]
fn a_range_must_not_be_backwards_or_not_a_number() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            Slider(name: "backwards", min: 10, max: 5),
            NumberField(name: "also_backwards", min: 1, max: 0),
            Slider(name: "fine", min: 5, max: 5),
            Slider(name: "nan_value", value: NaN),
            NumberField(name: "nan_min", min: NaN),
            NumberField(name: "infinite", max: inf),
        ]))"#,
    );
    assert!(
        has(
            &found,
            Severity::Error,
            "`min` (10) is greater than `max` (5)"
        ),
        "{found:?}"
    );
    assert!(
        has(
            &found,
            Severity::Error,
            "`min` (1) is greater than `max` (0)"
        ),
        "{found:?}"
    );
    for name in ["nan_value", "nan_min", "infinite"] {
        assert!(
            found
                .iter()
                .any(|d| d.node.as_deref() == Some(name) && d.is_error()),
            "{name}: {found:?}"
        );
    }
    assert!(found.iter().all(|d| d.node.as_deref() != Some("fine")));
}

#[test]
fn a_progress_bar_holds_what_an_i32_holds() {
    let found = diagnostics(
        r#"Form(root: Column(children: [
            ProgressBar(name: "ok", value: -2147483648, max: 2147483647),
            ProgressBar(name: "big_max", max: 4294967296),
            ProgressBar(name: "big_value", value: 2147483648),
            ProgressBar(name: "zero_max", max: 0),
        ]))"#,
    );
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(has(
        &found,
        Severity::Error,
        "`max` must be an int from 1 to 2147483647"
    ));
    assert!(has(
        &found,
        Severity::Error,
        "`value` must be an int from -2147483648 to 2147483647"
    ));
}

#[test]
fn a_length_out_of_range_is_an_error_on_a_form_that_was_never_read() {
    use xui_form::model::{Length, Node, Stack};
    let form = xui_form::Form {
        size: (Length(f32::NAN), Length(100.0)),
        root: Node::Column(Stack {
            gap: Length(1.0e38),
            width: Some(Length(f32::INFINITY)),
            ..Stack::default()
        }),
        ..xui_form::Form::default()
    };
    let found = form.validate(&Catalog::xui());
    assert_eq!(
        found.iter().filter(|d| d.is_error()).count(),
        3,
        "{found:?}"
    );
    assert!(has(&found, Severity::Error, "`gap`: 1e38 is not a length"));
    assert!(has(
        &found,
        Severity::Error,
        "the window's size: NaN is not a length"
    ));
}

#[test]
fn an_unnamed_container_array_is_only_a_container_error() {
    // It used to say as well that "a control array needs a name", which
    // sends the author to name something that cannot be an array at all.
    let found = diagnostics(r#"Form(root: Column(children: [Panel(array: 2), Group(index: 1)]))"#);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(has(&found, Severity::Error, "a Panel is a container"));
    assert!(has(&found, Severity::Error, "a Group is a container"));
    assert!(!has(&found, Severity::Error, "needs a name"));
}
