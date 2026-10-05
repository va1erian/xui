//! Shared harness for the integration tests.
//!
//! The only public way to obtain an `xui` [`Ui`](xui_core::Ui) is through
//! [`run_app`], so the tests build the form from inside its `make` closure
//! and run their assertions there. The [`OffscreenBackend`] renders
//! headlessly, so this works on CI.

// Each test binary uses its own part of the harness.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::OffscreenBackend;
use xui_core::app::{App, run_app};
use xui_core::backend::{Backend, Event, PlatformSpec};
use xui_core::message::{Modifiers, MouseButton};
use xui_core::units::Dip;
use xui_form::{
    Binder, BuildOptions, EventHandler, EventRef, Factories, Form, LiveForm, Value, build_with,
};

/// A message a test binder maps an event to.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// A click on the named widget, with its array index.
    Click(String, Option<usize>),
    /// A toggle with its new state.
    Toggle(bool),
    /// A selection with its index.
    Select(i64),
    /// A text change with its new text.
    Change(String),
}

/// A binder that maps every built-in event to a [`Msg`].
pub struct TestBinder;

impl Binder<Msg> for TestBinder {
    fn bind(&self, event: EventRef<'_>) -> Option<EventHandler<Msg>> {
        let (node, index) = (event.node.to_owned(), event.index);
        match event.event {
            "Click" => Some(Rc::new(move |_| Some(Msg::Click(node.clone(), index)))),
            "Toggle" => Some(Rc::new(|args| {
                args.first().and_then(Value::as_bool).map(Msg::Toggle)
            })),
            "Select" | "Activate" => Some(Rc::new(|args| {
                args.first().and_then(Value::as_int).map(Msg::Select)
            })),
            "Change" => Some(Rc::new(|args| {
                args.first()
                    .and_then(Value::as_str)
                    .map(|text| Msg::Change(text.to_owned()))
            })),
            _ => None,
        }
    }
}

/// An app that records the messages it receives.
pub struct TestApp {
    /// Every message, in order.
    pub messages: Rc<RefCell<Vec<Msg>>>,
}

impl App for TestApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut xui_core::Ui<Msg>) {
        self.messages.borrow_mut().push(msg);
    }
}

/// Loads `text`, panicking with the error when it does not load.
pub fn form(text: &str) -> Form {
    xui_form::load(text).unwrap_or_else(|error| panic!("the form loads: {error}"))
}

/// Builds `form` offscreen in a window of its own size and runs `check`
/// against the live form, returning its result.
pub fn with_form<R>(
    form: &Form,
    options: BuildOptions,
    check: impl FnOnce(&LiveForm<Msg>) -> R,
) -> R {
    with_form_sized(form, (form.size.0.0, form.size.1.0), options, check)
}

/// Builds `form` offscreen in a window of `size` design units.
pub fn with_form_sized<R>(
    form: &Form,
    size: (f32, f32),
    options: BuildOptions,
    check: impl FnOnce(&LiveForm<Msg>) -> R,
) -> R {
    let backend: Rc<dyn Backend> = Rc::new(OffscreenBackend::new());
    let slot: Rc<RefCell<Option<R>>> = Rc::new(RefCell::new(None));
    let out = Rc::clone(&slot);
    let spec = PlatformSpec::new("xui-form test").size(Dip(size.0), Dip(size.1));
    run_app(backend, spec, move |ui| {
        let live =
            build_with(ui, form, &Factories::xui(), &TestBinder, options).expect("the form builds");
        *out.borrow_mut() = Some(check(&live));
        TestApp {
            messages: Rc::new(RefCell::new(Vec::new())),
        }
    })
    .expect("run_app succeeds");
    slot.borrow_mut().take().expect("the closure ran")
}

/// Builds `form` offscreen, clicks the middle of each widget in `names` in
/// turn, and returns the messages the app received.
pub fn click(form: &Form, options: BuildOptions, names: &[&str]) -> Vec<Msg> {
    let backend = Rc::new(OffscreenBackend::new());
    let messages = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&messages);
    let clicker = Rc::clone(&backend);
    let spec = PlatformSpec::new("xui-form click").size(form.size.0.dip(), form.size.1.dip());
    run_app(backend as Rc<dyn Backend>, spec, move |ui| {
        let live =
            build_with(ui, form, &Factories::xui(), &TestBinder, options).expect("the form builds");
        let window = ui.window();
        for name in names {
            let bounds = live.bounds(name).expect("the widget exists");
            let (x, y) = (
                bounds.left + bounds.width() / 2,
                bounds.top + bounds.height() / 2,
            );
            let button = MouseButton::Left;
            let modifiers = Modifiers::NONE;
            clicker.inject(
                window,
                Event::MouseDown {
                    x,
                    y,
                    button,
                    modifiers,
                },
            );
            clicker.inject(
                window,
                Event::MouseUp {
                    x,
                    y,
                    button,
                    modifiers,
                },
            );
        }
        TestApp {
            messages: Rc::clone(&log),
        }
    })
    .expect("run_app succeeds");
    messages.borrow().clone()
}
