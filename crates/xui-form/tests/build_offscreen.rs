//! Offscreen build tests: clicking, reading and writing properties, design
//! mode, aliases and anchoring.

mod common;

use xui_core::geometry::Rect;
use xui_form::{BuildOptions, Catalog, FormDoc, Node, Value, ValueType};

use common::{Msg, aliased_catalog, click_node, with_form, with_form_sized};

/// A minimal form with one clickable button.
fn click_doc() -> FormDoc {
    let mut doc = FormDoc::new("main_form");
    let mut button = Node::new("Button", "cmdGo");
    button.set_prop("left", Value::Int(10));
    button.set_prop("top", Value::Int(10));
    button.set_prop("width", Value::Int(100));
    button.set_prop("height", Value::Int(30));
    button.set_prop("text", Value::Text("Go".to_owned()));
    doc.insert(button);
    doc
}

#[test]
fn clicking_a_button_delivers_the_binders_message() {
    let messages = click_node(
        &click_doc(),
        &Catalog::xui(),
        BuildOptions::default(),
        "cmdGo",
    );
    assert_eq!(messages, vec![Msg::Click("cmdGo".to_owned())]);
}

#[test]
fn design_mode_wires_no_events() {
    let messages = click_node(
        &click_doc(),
        &Catalog::xui(),
        BuildOptions {
            design_mode: true,
            ..BuildOptions::default()
        },
        "cmdGo",
    );
    assert!(messages.is_empty(), "the designer must not run event code");
}

#[test]
fn a_node_kind_may_use_an_alias() {
    let mut doc = FormDoc::new("main_form");
    let mut button = Node::new("CommandButton", "cmdGo");
    button.set_prop("left", Value::Int(0));
    button.set_prop("top", Value::Int(0));
    button.set_prop("width", Value::Int(80));
    button.set_prop("height", Value::Int(30));
    button.set_prop("text", Value::Text("Go".to_owned()));
    doc.insert(button);

    let messages = click_node(&doc, &aliased_catalog(), BuildOptions::default(), "cmdGo");
    assert_eq!(messages, vec![Msg::Click("cmdGo".to_owned())]);
}

/// A form with a filling panel and a bottom-right button inside it.
fn anchor_doc() -> FormDoc {
    let mut doc = FormDoc::new("main_form");
    let mut panel = Node::new("Panel", "panMain");
    panel.set_prop("left", Value::Int(0));
    panel.set_prop("top", Value::Int(0));
    panel.set_prop("width", Value::Int(320));
    panel.set_prop("height", Value::Int(200));
    panel.set_prop("anchor", Value::Enum("fill".to_owned()));
    doc.insert(panel);

    let mut button = Node::new("Button", "cmdGo");
    button.parent = Some("panMain".to_owned());
    button.set_prop("left", Value::Int(200));
    button.set_prop("top", Value::Int(150));
    button.set_prop("width", Value::Int(50));
    button.set_prop("height", Value::Int(40));
    button.set_prop("anchor", Value::Enum("bottom_right".to_owned()));
    doc.insert(button);
    doc
}

#[test]
fn the_form_anchors_to_a_window_larger_than_its_design() {
    let doc = anchor_doc();
    let catalog = Catalog::xui();
    let designed = with_form(&doc, &catalog, BuildOptions::default(), |form| {
        (form.bounds("panMain"), form.bounds("cmdGo"))
    });
    assert_eq!(designed.0, Some(Rect::new(0, 0, 320, 200)));
    assert_eq!(designed.1, Some(Rect::new(200, 150, 250, 190)));

    let grown = with_form_sized(&doc, &catalog, (500, 400), |form| {
        (form.bounds("panMain"), form.bounds("cmdGo"))
    });
    assert_eq!(grown.0, Some(Rect::new(0, 0, 500, 400)));
    assert_eq!(grown.1, Some(Rect::new(380, 350, 430, 390)));
}

#[test]
fn a_form_reports_a_node_kind_and_property_type() {
    let doc = click_doc();
    with_form(&doc, &Catalog::xui(), BuildOptions::default(), |form| {
        assert_eq!(form.kind("cmdGo"), Some("Button"));
        assert_eq!(form.kind("ghost"), None);
        assert_eq!(
            form.property_type("cmdGo", "text"),
            Some(ValueType::Text { multiline: false })
        );
        assert_eq!(
            form.property_type("cmdGo", "enabled"),
            Some(ValueType::Bool)
        );
        assert_eq!(form.property_type("cmdGo", "nope"), None);
    });
}

#[test]
fn a_form_with_a_bad_parent_fails_to_build() {
    let mut doc = FormDoc::new("main_form");
    let mut child = Node::new("Button", "cmdGo");
    child.parent = Some("ghost".to_owned());
    doc.insert(child);

    let backend: std::rc::Rc<dyn xui_core::backend::Backend> =
        std::rc::Rc::new(xui_canvas::OffscreenBackend::new());
    let catalog = Catalog::xui();
    let factories: xui_form::Factories<Msg> = xui_form::Factories::xui();
    let binder = common::TestBinder;
    let spec = xui_core::backend::PlatformSpec::new("bad parent")
        .size(xui_core::units::Dip(320.0), xui_core::units::Dip(200.0));
    let result = std::cell::RefCell::new(None);
    let result_ref = &result;
    xui_core::app::run_app(backend, spec, move |ui| {
        *result_ref.borrow_mut() = Some(xui_form::build(ui, &doc, &catalog, &factories, &binder));
        common::TestApp {
            messages: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    })
    .expect("run_app succeeds");
    let error = match result.borrow_mut().take().expect("build ran") {
        Err(error) => error,
        Ok(_) => panic!("a missing parent must fail the build"),
    };
    assert!(matches!(error, xui_form::BuildError::UnknownParent { .. }));
}

#[test]
fn geometry_and_anchor_edits_made_through_set_survive_a_relayout() {
    let doc = anchor_doc();
    let catalog = Catalog::xui();
    let button = with_form_sized(&doc, &catalog, (500, 400), |form| {
        // Move the button and pin it top-left instead of bottom-right.
        form.set("cmdGo", "left", &Value::Int(10))
            .expect("left is settable");
        form.set("cmdGo", "top", &Value::Int(20))
            .expect("top is settable");
        form.set("cmdGo", "anchor", &Value::Enum("top_left".to_owned()))
            .expect("anchor is settable");
        form.relayout();
        form.bounds("cmdGo")
    });
    assert_eq!(button, Some(Rect::new(10, 20, 60, 60)));
}

#[test]
fn a_child_listed_before_its_container_still_builds() {
    let ordered = anchor_doc();
    let mut doc = FormDoc::new("main_form");
    // The button (child) first, then its panel.
    doc.nodes = vec![ordered.nodes[1].clone(), ordered.nodes[0].clone()];
    let catalog = Catalog::xui();
    let bounds = with_form(&doc, &catalog, BuildOptions::default(), |form| {
        form.bounds("cmdGo")
    });
    assert_eq!(bounds, Some(Rect::new(200, 150, 250, 190)));
}

#[test]
fn float_properties_accept_integers_at_runtime() {
    let mut doc = FormDoc::new("main_form");
    doc.insert(Node::new("NumberField", "numOne"));
    doc.insert(Node::new("Slider", "sldOne"));
    let catalog = Catalog::xui();
    with_form(&doc, &catalog, BuildOptions::default(), |form| {
        form.set("numOne", "value", &Value::Int(3))
            .expect("an int sets a NumberField value");
        assert_eq!(form.get("numOne", "value"), Some(Value::Float(3.0)));
        form.set("sldOne", "value", &Value::Int(0))
            .expect("an int sets a Slider value");
    });
}

#[test]
fn a_list_view_without_selected_has_no_selection() {
    let mut doc = FormDoc::new("main_form");
    let mut list = Node::new("ListView", "items_list");
    list.set_prop("items", Value::List(vec!["a".to_owned(), "b".to_owned()]));
    doc.insert(list);
    let catalog = Catalog::xui();
    let selected = with_form(&doc, &catalog, BuildOptions::default(), |form| {
        form.get("items_list", "selected")
    });
    assert_eq!(selected, Some(Value::Int(-1)));
}

#[test]
fn a_list_view_accepts_items_at_runtime() {
    let mut doc = FormDoc::new("main_form");
    let mut list = Node::new("ListView", "items_list");
    list.set_prop("items", Value::List(vec!["a".to_owned()]));
    doc.insert(list);
    let catalog = Catalog::xui();
    let (before, after) = with_form(&doc, &catalog, BuildOptions::default(), |form| {
        let before = form.get("items_list", "items");
        form.set(
            "items_list",
            "items",
            &Value::List(vec!["a".to_owned(), "b".to_owned()]),
        )
        .expect("items is writable at runtime");
        (before, form.get("items_list", "items"))
    });
    assert_eq!(before, Some(Value::List(vec!["a".to_owned()])));
    assert_eq!(
        after,
        Some(Value::List(vec!["a".to_owned(), "b".to_owned()]))
    );
}

/// A form with a three-option radio group pinned to the bottom-right corner.
fn radio_doc() -> FormDoc {
    let mut doc = FormDoc::new("main_form");
    let mut group = Node::new("RadioGroup", "optSize");
    group.set_prop(
        "items",
        Value::List(vec!["S".to_owned(), "M".to_owned(), "L".to_owned()]),
    );
    group.set_prop("left", Value::Int(200));
    group.set_prop("top", Value::Int(100));
    group.set_prop("width", Value::Int(100));
    group.set_prop("anchor", Value::Enum("bottom_right".to_owned()));
    doc.insert(group);
    doc
}

#[test]
fn anchoring_and_edits_move_every_radio_option() {
    let doc = radio_doc();
    let catalog = Catalog::xui();
    let before = with_form(&doc, &catalog, BuildOptions::default(), |form| {
        form.node_bounds("optSize")
    });
    let (grown, edited) = with_form_sized(&doc, &catalog, (420, 300), |form| {
        let grown = form.node_bounds("optSize");
        form.set("optSize", "left", &Value::Int(10))
            .expect("left is settable");
        (grown, form.node_bounds("optSize"))
    });
    assert_eq!(before.len(), 3, "one node per option");
    for (old, new) in before.iter().zip(&grown) {
        // The window is larger by (100, 100): every option follows the corner.
        assert_eq!((new.left, new.top), (old.left + 100, old.top + 100));
    }
    for (option, moved) in grown.iter().zip(&edited) {
        assert_eq!(moved.left, 110, "an edited left moves every option");
        assert_eq!(moved.height(), option.height());
    }
}

#[test]
fn a_form_built_in_a_container_follows_the_container() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use xui_core::arrange::{Handle, LayoutExt, absolute, panel};
    use xui_core::widget::Panel;

    let doc = anchor_doc();
    let backend: Rc<dyn xui_core::backend::Backend> = Rc::new(xui_canvas::OffscreenBackend::new());
    let spec = xui_core::backend::PlatformSpec::new("container")
        .size(xui_core::units::Dip(640.0), xui_core::units::Dip(480.0));
    let bounds = Rc::new(RefCell::new(None));
    let out = Rc::clone(&bounds);
    xui_core::app::run_app(backend, spec, move |ui| {
        let host = Handle::<Panel<Msg>>::new();
        ui.root(absolute().child(panel(absolute()).plain().bind(&host).at(20, 10, 420, 300)))
            .expect("the host panel mounts");
        let options = BuildOptions {
            container: Some(host.get().id()),
            ..BuildOptions::default()
        };
        let factories = xui_form::Factories::xui();
        let form = xui_form::build_with(
            ui,
            &doc,
            &Catalog::xui(),
            &factories,
            &common::TestBinder,
            options,
        )
        .expect("the form builds in the panel");
        *out.borrow_mut() = Some((form.bounds("panMain"), form.bounds("cmdGo")));
        common::TestApp {
            messages: Rc::new(RefCell::new(Vec::new())),
        }
    })
    .expect("run_app succeeds");
    let (panel, button) = bounds.borrow_mut().take().expect("the app was built");
    assert_eq!(
        panel,
        Some(Rect::new(0, 0, 420, 300)),
        "relative to the host"
    );
    assert_eq!(button, Some(Rect::new(300, 250, 350, 290)));
}
