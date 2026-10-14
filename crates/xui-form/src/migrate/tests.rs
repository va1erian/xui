use super::*;
use crate::model::{Length, Node};

const LOGIN: &str = r#"format = 1

[window]
name = "login_form"
title = "Sign in"
width = 320
height = 200

[[node]]
kind = "Edit"
name = "user_edit"
left = 100
top = 16
width = 204
height = 28
anchor = "stretch_horizontal"
cue = "your name"
tab_index = 1

[[node]]
kind = "GroupBox"
name = "options"
left = 16
top = 60
width = 288
height = 80
text = "Options"

[[node]]
kind = "CheckBox"
name = "remember"
parent = "options"
left = 8
top = 20
text = "Remember me"
"#;

#[test]
fn anchored_rectangles_become_an_absolute_layout() {
    let migration = from_v1(LOGIN).expect("the form converts");
    let form = migration.form;
    assert_eq!(
        (form.name.as_str(), form.title.as_str()),
        ("login_form", "Sign in")
    );
    let Node::Absolute(root) = &form.root else {
        panic!("anchored nodes stay absolute: {:?}", form.root);
    };
    assert_eq!(root.size, Some((Length(320.0), Length(200.0))));
    let Node::Edit(edit) = &root.children[0] else {
        panic!("the first node is the edit");
    };
    assert_eq!(edit.placeholder, "your name");
    assert_eq!(
        edit.at,
        Some((Length(100.0), Length(16.0), Length(204.0), Length(28.0)))
    );
    assert_eq!(edit.anchor, Some(crate::model::Anchor::StretchHorizontal));
    let Node::Group(group) = &root.children[1] else {
        panic!("a GroupBox becomes a Group");
    };
    let Node::Absolute(content) = &*group.content else {
        panic!("its content is absolute");
    };
    let Node::CheckBox(check) = &content.children[0] else {
        panic!("the check box is inside");
    };
    assert_eq!(
        check.at,
        Some((Length(8.0), Length(20.0), Length(120.0), Length(24.0)))
    );
    assert_eq!(migration.notes.len(), 1, "{:?}", migration.notes);
    assert!(form.validate(&crate::Catalog::xui()).is_empty());
}

#[test]
fn rectangles_in_rows_and_columns_become_a_grid() {
    let mut text =
        String::from("format = 1\n\n[window]\nname = \"pad\"\nwidth = 176\nheight = 104\n");
    text.push_str("\n[[node]]\nkind = \"Label\"\nname = \"display\"\nleft = 8\ntop = 8\nwidth = 160\nheight = 24\n");
    for (index, (left, top)) in [(8, 40), (64, 40), (120, 40), (8, 72), (64, 72)]
        .iter()
        .enumerate()
    {
        text.push_str(&format!(
            "\n[[node]]\nkind = \"Button\"\nname = \"key_{index}\"\nleft = {left}\ntop = {top}\nwidth = 48\nheight = 24\n"
        ));
    }
    let form = from_v1(&text).expect("the form converts").form;
    let ron = form.to_ron().expect("the migrated form writes");
    assert!(ron.contains("Grid("), "{ron}");
    let Node::Grid(grid) = &form.root else {
        panic!("aligned rectangles make a grid: {ron}");
    };
    assert_eq!(grid.columns, [crate::model::Track::Fill(1); 3]);
    assert_eq!((grid.gap, grid.padding), (Length(8.0), Length(8.0)));
    let Node::Label(display) = &grid.children[0] else {
        panic!("the display comes first");
    };
    assert_eq!(
        (display.span, display.height),
        (Some(3), Some(Length(24.0)))
    );
    assert_eq!(grid.children.len(), 6);
}

#[test]
fn a_gap_in_the_flow_keeps_the_layout_absolute() {
    let text = "format = 1\n[window]\nname = \"f\"\n\
        [[node]]\nkind = \"Button\"\nname = \"a\"\nleft = 0\ntop = 0\nwidth = 50\nheight = 20\n\
        [[node]]\nkind = \"Button\"\nname = \"b\"\nleft = 60\ntop = 30\nwidth = 50\nheight = 20\n";
    let form = from_v1(text).expect("the form converts").form;
    assert!(matches!(form.root, Node::Absolute(_)));
}

#[test]
fn bad_input_is_an_error() {
    assert!(matches!(from_v1("not toml ["), Err(MigrateError::Toml(_))));
    let unknown = "format = 1\n[window]\nname = \"f\"\n[[node]]\nkind = \"Gauge\"\nname = \"g\"\n";
    assert!(matches!(from_v1(unknown), Err(MigrateError::Node { .. })));
}

/// A format-1 form of `containers` panels, each inside the one before, with a
/// button in the innermost.
fn nested_panels(containers: usize) -> String {
    let mut text =
        String::from("format = 1\n[window]\nname = \"deep\"\nwidth = 200\nheight = 200\n");
    for level in 0..containers {
        text.push_str(&format!(
            "[[node]]\nkind = \"Panel\"\nname = \"panel_{level}\"\nleft = 4\ntop = 4\nwidth = 180\nheight = 180\n"
        ));
        if level > 0 {
            text.push_str(&format!("parent = \"panel_{}\"\n", level - 1));
        }
    }
    text.push_str(&format!(
        "[[node]]\nkind = \"Button\"\nname = \"ok\"\nparent = \"panel_{}\"\nleft = 4\ntop = 4\n",
        containers - 1
    ));
    text
}

#[test]
fn nesting_up_to_the_limit_converts_and_loads_back() {
    // Each container takes two levels (itself and its content's layout), the
    // window's own layout is the first: seven containers and a button fill
    // the sixteen levels a form may nest.
    let migration = from_v1(&nested_panels(7)).expect("seven nested panels convert");
    assert_eq!(migration.form.root.depth(), crate::MAX_DEPTH);
    let text = migration.form.to_ron().expect("it writes");
    assert_eq!(crate::load(&text).expect("it loads"), migration.form);
}

#[test]
fn nesting_past_the_limit_is_an_error_naming_the_node() {
    let error = from_v1(&nested_panels(8)).expect_err("eight nested panels are too deep");
    let MigrateError::Node { node, message } = &error else {
        panic!("a node is named: {error:?}");
    };
    assert_eq!(node, "panel_7");
    assert!(message.contains("too deeply"), "{message}");

    // A hostile chain is refused as early, without walking all of it.
    let error = from_v1(&nested_panels(3_000)).expect_err("3000 nested panels are too deep");
    assert!(matches!(error, MigrateError::Node { .. }), "{error:?}");
}

/// A format-1 form with the given `[[node]]` bodies, each `kind, name, extra`.
fn form_of(nodes: &[(&str, &str, &str)]) -> String {
    let mut text = String::from("format = 1\n[window]\nname = \"f\"\n");
    for (kind, name, extra) in nodes {
        text.push_str(&format!(
            "[[node]]\nkind = \"{kind}\"\nname = \"{name}\"\n{extra}\n"
        ));
    }
    text
}

/// The problems `from_v1` found in `text`, as `node: message` lines.
fn problems(text: &str) -> Vec<String> {
    match from_v1(text) {
        Err(MigrateError::Invalid(problems)) => problems
            .into_iter()
            .map(|problem| format!("{}: {}", problem.node, problem.message))
            .collect(),
        other => panic!("expected the form to be invalid, got {other:?}"),
    }
}

#[test]
fn a_name_used_twice_is_refused_not_a_stack_overflow() {
    // The second `x` is a child of `x`: converting the list walked it for
    // ever.
    let text = form_of(&[("Panel", "x", ""), ("Button", "x", "parent = \"x\"")]);
    assert_eq!(problems(&text), ["x: the name is used more than once"]);
}

#[test]
fn a_leaf_cannot_be_a_parent() {
    let text = form_of(&[("Button", "b", ""), ("Label", "l", "parent = \"b\"")]);
    let found = problems(&text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("l: its parent `b` is a Button"),
        "{found:?}"
    );
}

#[test]
fn a_missing_parent_is_reported_with_its_node() {
    let text = form_of(&[("Label", "l", "parent = \"nowhere\"")]);
    assert_eq!(problems(&text), ["l: its parent `nowhere` does not exist"]);
}

#[test]
fn a_parent_cycle_is_reported_for_each_node_on_it() {
    let text = form_of(&[
        ("Panel", "a", "parent = \"c\""),
        ("Panel", "b", "parent = \"a\""),
        ("Panel", "c", "parent = \"b\""),
        ("Panel", "outside", "parent = \"a\""),
        ("Panel", "own", "parent = \"own\""),
    ]);
    assert_eq!(
        problems(&text),
        [
            "a: it is part of a parent cycle",
            "b: it is part of a parent cycle",
            "c: it is part of a parent cycle",
            "own: it is part of a parent cycle",
        ]
    );
}

#[test]
fn every_violation_is_reported_at_once() {
    let text = form_of(&[
        ("Button", "b", ""),
        ("Panel", "p", "left = \"8\""),
        ("Label", "l", "parent = \"b\""),
        ("Label", "l", "parent = \"ghost\""),
    ]);
    let found = problems(&text);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found[0].starts_with("p: `left` must be an integer"));
    assert!(found[1].starts_with("l: its parent `b`"));
    assert!(found[2].starts_with("l: the name is used more than once"));
    assert!(found[3].starts_with("l: its parent `ghost` does not exist"));
}

#[test]
fn a_very_long_chain_of_panels_is_refused_without_recursing() {
    let mut text = String::from("format = 1\n[window]\nname = \"deep\"\n");
    for level in 0..20_000 {
        text.push_str(&format!(
            "[[node]]\nkind = \"Panel\"\nname = \"p{level}\"\n"
        ));
        if level > 0 {
            text.push_str(&format!("parent = \"p{}\"\n", level - 1));
        }
    }
    assert!(matches!(from_v1(&text), Err(MigrateError::Node { .. })));
}

#[test]
fn coordinates_that_overflow_are_not_a_grid_and_do_not_panic() {
    const FIRST: &str = "left = 0\ntop = 0\nwidth = 50\nheight = 20";
    let extremes = [
        // `left + width` overflows.
        (
            FIRST,
            "left = 9223372036854775807\ntop = 0\nwidth = 10\nheight = 20",
        ),
        // The gap between the columns, `0 - left`, overflows.
        (
            FIRST,
            "left = -9223372036854775808\ntop = 0\nwidth = 10\nheight = 20",
        ),
        // The gap between the rows overflows.
        (
            FIRST,
            "left = 0\ntop = -9223372036854775808\nwidth = 50\nheight = 20",
        ),
        // The room right of the last column, `width - right`, overflows.
        (
            "left = -9223372036854775808\ntop = -9223372036854775808\nwidth = 50\nheight = 20",
            "left = -9223372036854775748\ntop = -9223372036854775808\nwidth = 50\nheight = 20",
        ),
    ];
    for (first, second) in extremes {
        let text = form_of(&[("Button", "a", first), ("Button", "b", second)]);
        // Refused or kept absolute; never a grid, never an overflow panic.
        if let Ok(migration) = from_v1(&text) {
            assert!(
                matches!(migration.form.root, Node::Absolute(_)),
                "{second}: {:?}",
                migration.form.root
            );
        }
    }
}

#[test]
fn a_value_of_the_wrong_type_is_refused_not_defaulted() {
    let text = "format = 1\n[window]\nname = \"f\"\nwidth = 0\nresizable = \"yes\"\n[[node]]\nkind = \"Button\"\nname = \"a\"\nanchor = 5\n";
    let found = problems(text);
    assert_eq!(
        found,
        [
            "window: `width` must be an integer of at least 1",
            "window: `resizable` must be true or false",
            "a: `anchor` must be a string",
        ]
    );
}

#[test]
fn what_is_ignored_is_noted() {
    let text = "format = 1\n[window]\nname = \"f\"\ncolour = \"red\"\n[[nodes]]\nkind = \"Button\"\nname = \"lost\"\n";
    let migration = from_v1(text).expect("the form converts");
    assert_eq!(migration.notes.len(), 2, "{:?}", migration.notes);
    assert!(migration.notes[0].contains("`window`: dropped `colour`"));
    assert!(migration.notes[1].contains("top-level `nodes`"));
}

#[test]
fn a_form_that_converts_to_an_invalid_one_is_refused() {
    // Each is valid TOML and a tree; `check` would reject the result.
    let text = form_of(&[
        ("Button", "form", ""),
        ("Edit", "my-edit", ""),
        ("Slider", "level", "min = 50\nmax = 10"),
        ("ComboBox", "pick", "items = [\"a\"]\nselected = 3"),
    ]);
    let found = problems(&text);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(
        found[0].starts_with("form: `form` is reserved"),
        "{found:?}"
    );
    assert!(
        found[1].starts_with("my-edit: `my-edit` is not a valid name"),
        "{found:?}"
    );
    assert!(
        found[2].starts_with("level: `min` (50) is greater than `max` (10)"),
        "{found:?}"
    );
    assert!(found[3].starts_with("pick: `selected` is 3"), "{found:?}");
}
