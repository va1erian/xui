//! The generic property surface on the native widgets, as a form designer would
//! use it: read a widget's properties and edit them through the same typed API.
//!
//! Requires a real desktop; run it in Windows Sandbox like the rest of the UI
//! tests.

#![cfg(windows)]

mod common;

use common::run_app_with_watchdog;
use xui_win32::prelude::*;
use xui_win32::{Properties, Value};

enum Msg {}

struct PropsApp {
    // Kept alive for the run.
    _label: Label,
    _button: Button<Msg>,
}

impl App for PropsApp {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, _ui: &mut Ui<Msg>) {}
}

#[test]
fn native_widgets_report_and_edit_properties() {
    let Some(run) = run_app_with_watchdog("win32ui.props", |ui| {
        let label = Label::new(ui, Rect::new(0, 0, 120, 24), "hi").unwrap();
        assert_eq!(label.property("text"), Some(Value::Text("hi".to_string())));
        assert_eq!(label.property("visible"), Some(Value::Bool(true)));
        assert!(label.set_property("text", Value::Text("bye".to_string())));
        assert_eq!(label.text(), "bye");
        assert!(!label.set_property("text", Value::Bool(true)), "wrong type");
        assert!(!label.set_property("nope", Value::Text("x".to_string())));

        assert!(label.set_property("visible", Value::Bool(false)));
        assert_eq!(label.property("visible"), Some(Value::Bool(false)));
        assert!(!label.is_visible());

        let button = Button::new(ui, "Ok").unwrap();
        assert_eq!(button.property("enabled"), Some(Value::Bool(true)));
        assert!(button.set_property("enabled", Value::Bool(false)));
        assert!(!button.is_enabled());
        assert!(button.set_property("enabled", Value::Bool(true)));
        assert!(button.is_enabled());

        // The test is the assertion; quit at once.
        ui.quit();
        PropsApp {
            _label: label,
            _button: button,
        }
    }) else {
        return;
    };

    assert!(!run.timed_out);
}
