//! Integration tests for [`ScriptForm`]: building a form on the offscreen
//! backend, running its `.rhai` script and routing events.
//!
//! These exercise the reusable path with no IDE project: a `Form`, a
//! script string and `xui-script` alone.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::app::{App, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::units::Dip;
use xui_form::{Form, Value};
use xui_script::form::{FormError, ScriptForm, ScriptSource};
use xui_script::{EngineHost, Msg, ScriptError};

/// An application with no messages of its own; the test runs the script inline.
struct TestApp;

impl App for TestApp {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, _ui: &mut xui_core::Ui<Msg>) {}
}

/// A form with an `Edit` named `name_edit` (holding "Ada"), a `Label` named
/// `result_label` and a control array of three buttons named `pick`.
fn greeting_doc() -> Form {
    xui_form::load(
        r#"Form(name: "frmMain", root: Column(padding: 10, gap: 6, children: [
            Edit(name: "name_edit", text: "Ada"),
            Label(name: "result_label", text: "before"),
            Row(children: [Button(name: "pick", array: 3, text: "{index}")]),
        ]))"#,
    )
    .expect("the form loads")
}

/// Builds `doc` with `code`, runs `configure` on the engine, then runs `check`
/// against the built [`ScriptForm`] and returns its result.
fn run<R>(
    doc: &Form,
    code: &str,
    configure: impl FnOnce(&mut EngineHost) -> Result<(), ScriptError> + 'static,
    check: impl FnOnce(&ScriptForm) -> R + 'static,
) -> Result<R, FormError> {
    let backend: Rc<dyn Backend> = Rc::new(OffscreenBackend::new());
    let spec = PlatformSpec::new("xui-script test").size(Dip(320.0), Dip(200.0));
    let slot: Rc<RefCell<Option<Result<R, FormError>>>> = Rc::new(RefCell::new(None));
    let slot_inner = Rc::clone(&slot);
    let doc = doc.clone();
    let code = code.to_owned();

    run_app(backend, spec, move |ui| {
        let result = ScriptForm::build(
            ui,
            &doc,
            ScriptSource {
                name: "frmMain",
                code: &code,
                file: "frmMain.rhai",
            },
            (),
            configure,
        )
        .map(|script| check(&script));
        *slot_inner.borrow_mut() = Some(result);
        TestApp
    })
    .expect("run_app succeeds");

    slot.borrow_mut().take().expect("the build ran")
}

#[test]
fn form_load_runs_and_a_click_handler_updates_a_label() {
    let doc = greeting_doc();
    let form = run(
        &doc,
        "fn form_load() { result_label.text = name_edit.text; }\n\
         fn hello_button_click() { result_label.text = `Hi, ${name_edit.text}`; }",
        |_| Ok(()),
        |script| {
            script
                .run("hello_button", "Click", &[])
                .expect("the click handler runs");
            script.live_form().get("result_label", "text")
        },
    )
    .expect("the form builds");

    assert_eq!(form, Some(Value::Text("Hi, Ada".to_owned())));
}

#[test]
fn a_missing_handler_is_not_an_error() {
    let doc = greeting_doc();
    run(
        &doc,
        "",
        |_| Ok(()),
        |script| {
            script
                .run("hello_button", "Click", &[])
                .expect("a missing handler is ignored");
            assert_eq!(
                script.live_form().get("result_label", "text"),
                Some(Value::Text("before".to_owned())),
                "nothing ran"
            );
        },
    )
    .expect("the form builds");
}

#[test]
fn a_handler_with_an_extra_parameter_receives_unit() {
    let doc = greeting_doc();
    let value = run(
        &doc,
        "fn go_button_click(sender) { \
             result_label.text = if sender == () { \"unit\" } else { \"other\" }; \
         }",
        |_| Ok(()),
        |script| {
            script
                .run("go_button", "Click", &[])
                .expect("the click handler runs");
            script.live_form().get("result_label", "text")
        },
    )
    .expect("the form builds");

    assert_eq!(value, Some(Value::Text("unit".to_owned())));
}

#[test]
fn configure_registers_a_module_the_script_imports() {
    let doc = greeting_doc();
    let label = run(
        &doc,
        "fn form_load() { result_label.text = greeting(\"Grace\"); }",
        |host| {
            host.register_module(
                "util",
                "util.rhai",
                "fn greeting(name) { `Hello, ${name}!` }",
            )
        },
        |script| script.live_form().get("result_label", "text"),
    )
    .expect("the form builds");

    assert_eq!(label, Some(Value::Text("Hello, Grace!".to_owned())));
}

#[test]
fn a_form_load_error_is_located_and_nothing_runs() {
    let doc = greeting_doc();
    let error = run(
        &doc,
        "fn form_load() {\n    result_label.nope = 1;\n}",
        |_| Ok(()),
        |_script| unreachable!("the form must not build"),
    )
    .expect_err("form_load fails");

    let FormError::Script(error) = error else {
        panic!("a located script error is expected, got {error:?}");
    };
    assert_eq!(error.file, "frmMain.rhai");
    assert_eq!(error.line, 2);
    assert!(error.column > 0);
}

#[test]
fn a_parse_error_is_located_before_the_form_is_built() {
    let doc = greeting_doc();
    let error = run(
        &doc,
        "fn form_load() {\n    let x = ;\n}",
        |_| Ok(()),
        |_script| unreachable!("the form must not build"),
    )
    .expect_err("the script does not parse");

    let FormError::Script(error) = error else {
        panic!("a located script error is expected, got {error:?}");
    };
    assert_eq!((error.file.as_str(), error.line), ("frmMain.rhai", 2));
    assert!(error.column > 0);
}

#[test]
fn a_control_array_handler_gets_the_index_and_elements_index_in_scripts() {
    let doc = greeting_doc();
    let text = run(
        &doc,
        "fn pick_click(index) { result_label.text = `${pick[index].text}!`; }",
        |_| Ok(()),
        |form| {
            form.run("pick", "Click", &[Value::Int(2)])
                .expect("the handler runs");
            form.live_form().get("result_label", "text")
        },
    )
    .expect("the form builds");
    assert_eq!(text, Some(Value::Text("2!".to_owned())));
}

#[test]
fn the_calculator_example_adds() {
    let doc = xui_form::load(include_str!("../../xui/examples/forms/calculator.lfm"))
        .expect("the calculator loads");
    let code = include_str!("../../xui/examples/forms/calculator.rhai");
    let display = run(
        &doc,
        code,
        |_| Ok(()),
        |form| {
            let press = |control: &str, index: Option<i64>| {
                let args: Vec<Value> = index.map(Value::Int).into_iter().collect();
                form.run(control, "Click", &args).expect("the handler runs");
            };
            press("digit", Some(7));
            press("operator", Some(3));
            press("digit", Some(3));
            press("equals", None);
            form.live_form().get("display", "text")
        },
    )
    .expect("the form builds");
    assert_eq!(display, Some(Value::Text("10.0".to_owned())));
}
