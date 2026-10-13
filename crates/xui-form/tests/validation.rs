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
    assert!(has(
        &found,
        Severity::Error,
        "`digit` is used more than once"
    ));
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
