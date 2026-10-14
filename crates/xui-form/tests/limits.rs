//! The depth limit the reader, the writer, `validate` and `describe` share.

mod common;

use common::{build_failure, form, with_form};
use xui_form::model::{Absolute, Grid, Group, Label, Node, Page, Panel, Stack, Tabs};
use xui_form::{BuildOptions, Catalog, Form, MAX_ARRAY, MAX_DEPTH, WriteError, format, load};

/// The kinds that nest a node, one of each shape of nesting: a child list, a
/// container's content and a page's content.
const KINDS: [&str; 6] = ["Column", "Grid", "Absolute", "Panel", "Group", "Tabs"];

/// A form's text: `depth` levels of `kind` nested, ending in a label, with
/// each node starting on a line of its own (the node at level `k` is on line
/// `k + 1`).
fn nested_text(kind: &str, depth: usize) -> String {
    let (open, close) = match kind {
        "Column" | "Grid" | "Absolute" => (format!("{kind}(children: ["), "])"),
        "Panel" | "Group" => (format!("{kind}(content: "), ")"),
        "Tabs" => (
            "Tabs(pages: [Page(title: \"t\", content: ".to_owned(),
            ")])",
        ),
        other => panic!("no such kind {other}"),
    };
    let mut text = String::from("Form(\n");
    for level in 1..depth {
        let prefix = if level == 1 { "root: " } else { "" };
        text.push_str(&format!("{prefix}{open}\n"));
    }
    text.push_str(if depth == 1 {
        "root: Label()\n"
    } else {
        "Label()\n"
    });
    for _ in 1..depth {
        text.push_str(close);
        text.push('\n');
    }
    text.push_str(")\n");
    text
}

/// A tree of `depth` levels of `kind`, ending in a label.
fn nested_tree(kind: &str, depth: usize) -> Form {
    let mut node = Node::Label(Label::default());
    for _ in 1..depth {
        node = match kind {
            "Column" => Node::Column(Stack {
                children: vec![node],
                ..Stack::default()
            }),
            "Grid" => Node::Grid(Grid {
                children: vec![node],
                ..Grid::default()
            }),
            "Absolute" => Node::Absolute(Absolute {
                children: vec![node],
                ..Absolute::default()
            }),
            "Panel" => Node::Panel(Panel {
                content: Box::new(node),
                ..Panel::default()
            }),
            "Group" => Node::Group(Group {
                content: Box::new(node),
                ..Group::default()
            }),
            "Tabs" => Node::Tabs(Tabs {
                pages: vec![Page {
                    title: "t".to_owned(),
                    content: node,
                }],
                ..Tabs::default()
            }),
            other => panic!("no such kind {other}"),
        };
    }
    Form {
        root: node,
        ..Form::default()
    }
}

fn mentions_depth(form: &Form) -> bool {
    form.validate(&Catalog::xui())
        .iter()
        .any(|d| d.is_error() && d.message.contains("levels deep"))
}

#[test]
fn a_form_nested_to_the_limit_loads_and_writes_back() {
    for kind in KINDS {
        let form = load(&nested_text(kind, MAX_DEPTH))
            .unwrap_or_else(|error| panic!("{kind} at the limit: {error}"));
        assert_eq!(form.root.depth(), MAX_DEPTH, "{kind}");
        assert!(!mentions_depth(&form), "{kind}");

        let written = form.to_ron().expect("a form at the limit is written");
        assert_eq!(load(&written).expect("it reads back"), form, "{kind}");
        assert_eq!(format(&written).expect("it formats"), written, "{kind}");
    }
}

#[test]
fn one_level_past_the_limit_is_refused_where_it_is() {
    for kind in KINDS {
        let text = nested_text(kind, MAX_DEPTH + 1);
        let error = load(&text).expect_err("one past the limit does not load");
        assert!(
            error.message.contains("nests at most 16 levels deep"),
            "{kind}: {error}"
        );
        // Line 1 is `Form(`; the node at level `k` is on line `k + 1`, and
        // the error is at the node, not at the bracket before it.
        assert_eq!(
            (error.line, error.column),
            (MAX_DEPTH + 2, 1),
            "{kind}: {error}"
        );
        assert!(format(&text).is_err(), "{kind}: formatting loads first");
    }
}

#[test]
fn to_ron_refuses_what_load_would_refuse() {
    for kind in KINDS {
        let at_limit = nested_tree(kind, MAX_DEPTH);
        let text = at_limit.to_ron().expect("a tree at the limit is written");
        assert_eq!(load(&text).expect("it reads back"), at_limit, "{kind}");

        let past = nested_tree(kind, MAX_DEPTH + 1);
        assert_eq!(
            past.to_ron(),
            Err(WriteError::TooDeep {
                depth: MAX_DEPTH + 1
            }),
            "{kind}"
        );
    }
}

#[test]
fn a_very_deep_tree_is_reported_without_recursing() {
    // Neither measuring, validating nor writing walks the tree recursively.
    let deep = nested_tree("Column", 2_000);
    assert_eq!(deep.root.depth(), 2_000);
    assert!(mentions_depth(&deep));
    assert_eq!(deep.to_ron(), Err(WriteError::TooDeep { depth: 2_000 }));
    let message = build_failure(&deep).expect("a form that deep is not built");
    assert!(message.contains("2000 levels deep"), "{message}");
}

#[test]
fn hostile_nesting_is_an_error_not_a_stack_overflow() {
    let mut text = String::from("Form(root: ");
    text.push_str(&"Column(children: [".repeat(200_000));
    let error = load(&text).expect_err("200000 levels do not load");
    assert!(error.message.contains("levels deep"), "{error}");
    assert_eq!((error.line, error.column > 0), (1, true));

    let tabs = "Tabs(pages: [Page(content: ".repeat(200_000);
    assert!(load(&format!("Form(root: {tabs}")).is_err());
}

#[test]
fn a_form_at_the_limit_builds() {
    let at_limit = form(&nested_text("Column", MAX_DEPTH));
    with_form(&at_limit, BuildOptions::default(), |live| {
        assert!(live.controls().ids().is_empty(), "nothing is named");
    });
    let past = nested_tree("Column", MAX_DEPTH + 1);
    let message = build_failure(&past).expect("one past the limit is not built");
    assert!(message.contains("17 levels deep"), "{message}");
}

#[test]
fn a_form_past_the_array_cap_is_not_built() {
    for (text, needle) in [
        (
            r#"Form(root: Button(name: "b", array: 100000000))"#,
            "`array` is 100000000",
        ),
        (
            r#"Form(root: Button(name: "b", index: 20000000))"#,
            "`index` is 20000000",
        ),
        (
            r#"Form(root: Panel(name: "p", array: 30, content: Button(name: "b")))"#,
            "a Panel is a container",
        ),
    ] {
        let message = build_failure(&form(text)).expect("it is not built");
        assert!(message.contains(needle), "{message}");
    }
}

#[test]
fn an_array_at_the_cap_builds_every_element() {
    let at_cap = form(&format!(
        r#"Form(size: (640, 480), root: Wrap(children: [Button(name: "key", array: {MAX_ARRAY}, text: "{{index}}")]))"#
    ));
    with_form(&at_cap, BuildOptions::default(), |live| {
        assert_eq!(live.controls().ids().len(), MAX_ARRAY as usize);
        assert!(live.get("key[1023]", "text").is_some());
        assert!(live.get("key[1024]", "text").is_none());
    });
}

#[test]
fn a_length_that_cannot_be_read_back_is_not_written() {
    use xui_form::model::{Length, Node, Stack};
    for bad in [1.0e38, f32::NAN, f32::INFINITY, -32768.5] {
        let form = Form {
            root: Node::Column(Stack {
                gap: Length(bad),
                ..Stack::default()
            }),
            ..Form::default()
        };
        let Err(WriteError::Unwritable(message)) = form.to_ron() else {
            panic!("{bad} is not written");
        };
        assert!(message.contains("is not a length"), "{message}");
    }
    // The limit itself is a length, and reads back.
    let at_limit = Form {
        root: Node::Column(Stack {
            gap: Length(-32768.0),
            width: Some(Length(32768.0)),
            ..Stack::default()
        }),
        ..Form::default()
    };
    let text = at_limit.to_ron().expect("lengths at the limit are written");
    assert_eq!(load(&text).expect("they read back"), at_limit);
}

#[test]
fn huge_lengths_are_not_built_and_the_largest_ones_lay_out() {
    use xui_form::model::{Label, Length, Node, Stack};
    // Two `width: 1e38` made `render` panic in a debug build ("attempt to add
    // with overflow"): such a form is refused where it is read, and one that
    // was built in memory is refused by the build.
    let row = Form {
        root: Node::Row(Stack {
            children: vec![
                Node::Label(Label {
                    width: Some(Length(1.0e38)),
                    ..Label::default()
                }),
                Node::Label(Label {
                    width: Some(Length(1.0e38)),
                    ..Label::default()
                }),
            ],
            ..Stack::default()
        }),
        ..Form::default()
    };
    let message = build_failure(&row).expect("it is not built");
    assert!(message.contains("is not a length"), "{message}");

    // The largest lengths a file may hold lay out, and shrink to the room.
    let widest = form(
        r#"Form(root: Row(children: [
            Label(name: "a", width: 32768), Label(name: "b", width: 32768),
            Absolute(children: [Label(name: "c", at: (32768, 32768, 32768, 32768))]),
        ]))"#,
    );
    with_form(&widest, BuildOptions::default(), |live| {
        live.relayout();
        assert!(live.bounds("a").is_some() && live.bounds("c").is_some());
    });
}
