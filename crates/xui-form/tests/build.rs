//! Building forms offscreen: layouts, events, control arrays and anchors.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::{Msg, TestApp, TestBinder, click, form, with_form, with_form_sized};
use xui_core::arrange::{LayoutExt, column, label};
use xui_core::geometry::Rect;
use xui_form::{BuildOptions, Factories, SetError, Value, describe};

const GREETER: &str = r#"Form(
    size: (360, 160),
    root: Column(padding: 16, gap: 8, children: [
        Row(gap: 8, children: [
            Edit(name: "name_edit", placeholder: "Your name", fill: 1),
            Button(name: "greet_button", text: "Greet", width: 80),
        ]),
        Label(name: "result_label", text: "Hi"),
    ]),
)"#;

#[test]
fn a_form_builds_the_same_layout_as_rust() {
    let bounds = with_form(&form(GREETER), BuildOptions::default(), |live| {
        ["name_edit", "greet_button", "result_label"].map(|name| live.bounds(name))
    });
    assert_eq!(bounds[0], Some(Rect::new(16, 16, 256, 44)));
    assert_eq!(bounds[1], Some(Rect::new(264, 16, 344, 44)));
    assert_eq!(bounds[2].map(|r| (r.left, r.top)), Some((16, 52)));
}

#[test]
fn clicking_a_button_delivers_the_binders_message() {
    let messages = click(&form(GREETER), BuildOptions::default(), &["greet_button"]);
    assert_eq!(messages, [Msg::Click("greet_button".to_owned(), None)]);
}

#[test]
fn design_mode_wires_no_events() {
    let options = BuildOptions {
        design_mode: true,
        ..BuildOptions::default()
    };
    assert!(click(&form(GREETER), options, &["greet_button"]).is_empty());
}

#[test]
fn a_control_array_names_its_elements_and_passes_their_index() {
    let keypad = form(
        r#"Form(root: Grid(columns: [Fill(1), Fill(1), Fill(1)], children: [
            Button(name: "digit", array: 10, text: "{index}"),
        ]))"#,
    );
    let texts = with_form(&keypad, BuildOptions::default(), |live| {
        ["digit[0]", "digit[7]"].map(|name| live.get(name, "text"))
    });
    assert_eq!(texts[0], Some(Value::Text("0".to_owned())));
    assert_eq!(texts[1], Some(Value::Text("7".to_owned())));
    let messages = click(&keypad, BuildOptions::default(), &["digit[7]", "digit[2]"]);
    assert_eq!(
        messages,
        [
            Msg::Click("digit".to_owned(), Some(7)),
            Msg::Click("digit".to_owned(), Some(2)),
        ]
    );
}

const PINNED: &str = r#"Form(
    size: (320, 200),
    root: Absolute(size: (320, 200), children: [
        Panel(name: "back", at: (0, 0, 320, 200), anchor: Fill),
        Button(name: "ok", text: "OK", at: (200, 150, 50, 40), anchor: BottomRight),
    ]),
)"#;

#[test]
fn an_absolute_layout_anchors_to_a_larger_window() {
    let grown = with_form_sized(
        &form(PINNED),
        (500.0, 400.0),
        BuildOptions::default(),
        |l| (l.bounds("back"), l.bounds("ok")),
    );
    assert_eq!(grown.0, Some(Rect::new(0, 0, 500, 400)));
    assert_eq!(grown.1, Some(Rect::new(380, 350, 430, 390)));
}

#[test]
fn geometry_writes_move_an_absolute_entry_for_good() {
    let moved = with_form(&form(PINNED), BuildOptions::default(), |live| {
        live.set("ok", "left", &Value::Int(10))
            .expect("left is settable");
        live.set("ok", "top", &Value::Int(20))
            .expect("top is settable");
        live.relayout();
        (live.bounds("ok"), live.get("ok", "left"))
    });
    assert_eq!(moved.0, Some(Rect::new(10, 20, 60, 60)));
    assert_eq!(moved.1, Some(Value::Int(10)));
}

#[test]
fn geometry_is_read_only_outside_an_absolute_layout() {
    let result = with_form(&form(GREETER), BuildOptions::default(), |live| {
        (
            live.set("greet_button", "left", &Value::Int(4)),
            live.get("greet_button", "left"),
        )
    });
    assert_eq!(result, (Err(SetError::ReadOnly), None));
}

#[test]
fn a_hidden_widget_starts_hidden_and_frees_its_space() {
    let hidden = form(
        r#"Form(root: Column(children: [
            Button(name: "first", visible: false), Button(name: "second"),
        ]))"#,
    );
    let (first, second) = with_form(&hidden, BuildOptions::default(), |live| {
        live.relayout();
        (live.get("first", "visible"), live.bounds("second"))
    });
    assert_eq!(first, Some(Value::Bool(false)));
    assert_eq!(second.map(|r| r.top), Some(0));
}

#[test]
fn containers_hold_their_content() {
    let nested = form(
        r#"Form(root: Column(children: [
            Group(name: "options", text: "Options", content: Column(children: [
                CheckBox(name: "wrap_check", text: "Wrap"),
            ])),
            Tabs(name: "pages", selected: 1, pages: [
                Page(title: "One", content: Label(name: "one")),
                Page(title: "Two", content: Label(name: "two")),
            ], fill: 1),
        ]))"#,
    );
    let (selected, two) = with_form(&nested, BuildOptions::default(), |live| {
        (live.get("pages", "selected"), live.get("two", "visible"))
    });
    assert_eq!(selected, Some(Value::Int(1)));
    assert_eq!(two, Some(Value::Bool(true)));
}

#[test]
fn a_described_form_mounts_inside_a_rust_layout() {
    let greeter = form(GREETER);
    let backend: Rc<dyn xui_core::backend::Backend> = Rc::new(xui_canvas::OffscreenBackend::new());
    let spec = xui_core::backend::PlatformSpec::new("describe")
        .size(xui_core::units::Dip(360.0), xui_core::units::Dip(200.0));
    let found = Rc::new(RefCell::new(None));
    let out = Rc::clone(&found);
    xui_core::app::run_app(backend, spec, move |ui| {
        let described = describe(
            ui,
            &greeter,
            &Factories::xui(),
            &TestBinder,
            BuildOptions::default(),
        )
        .expect("the form describes");
        ui.root(
            column()
                .child(label("Header").height(40))
                .child(described.layout.fill(1)),
        )
        .expect("the layout mounts");
        described.controls.ready();
        *out.borrow_mut() = described.controls.bounds("name_edit");
        TestApp {
            messages: Rc::new(RefCell::new(Vec::new())),
        }
    })
    .expect("run_app succeeds");
    assert_eq!(found.borrow().map(|r| r.top), Some(56), "below the header");
}

#[test]
fn a_migrated_grid_lays_out_as_it_was_designed() {
    let mut v1 = String::from("format = 1\n[window]\nname = \"pad\"\nwidth = 176\nheight = 72\n");
    v1.push_str("[[node]]\nkind = \"Label\"\nname = \"display\"\nleft = 8\ntop = 8\nwidth = 160\nheight = 24\n");
    for (index, left) in [8, 64, 120].iter().enumerate() {
        v1.push_str(&format!(
            "[[node]]\nkind = \"Button\"\nname = \"key_{index}\"\nleft = {left}\ntop = 40\nwidth = 48\nheight = 24\n"
        ));
    }
    let migrated = xui_form::migrate::from_v1(&v1)
        .expect("the form converts")
        .form;
    let bounds = with_form(&migrated, BuildOptions::default(), |live| {
        ["display", "key_0", "key_2"].map(|name| live.bounds(name))
    });
    assert_eq!(bounds[0], Some(Rect::new(8, 8, 168, 32)));
    assert_eq!(bounds[1], Some(Rect::new(8, 40, 56, 64)));
    assert_eq!(bounds[2], Some(Rect::new(120, 40, 168, 64)));
}
