use super::*;

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
    let ron = form.to_ron();
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
