//! A sign-in form loaded from a `.lfm` file and built into widgets.
//!
//! The form is data (`forms/login.lfm`): [`FormDoc::from_toml`] loads it
//! against the schema, [`build`] turns it into live widgets, and a
//! [`Binder`] maps the events the app cares about to its own `Msg`. The
//! fields stretch and the buttons stay in the corner as the window resizes.
//!
//! Run it with `cargo run -p xui --features form --example form`.

use std::rc::Rc;

use xui::BackendError;
use xui::form::{
    Binder, Catalog, EventHandler, EventRef, Factories, FormDoc, LiveForm, Value, build,
};
use xui::prelude::*;

/// The form, embedded so the example runs from any directory.
const LOGIN: &str = include_str!("forms/login.lfm");

#[derive(Clone, Debug)]
enum Msg {
    SignIn,
    Cancel,
}

/// Maps the buttons' clicks to [`Msg`]; every other event stays unwired.
struct LoginBinder;

impl Binder<Msg> for LoginBinder {
    fn bind(&self, event: EventRef<'_>) -> Option<EventHandler<Msg>> {
        let msg = match (event.node, event.event) {
            ("sign_in_button", "Click") => Msg::SignIn,
            ("cancel_button", "Click") => Msg::Cancel,
            _ => return None,
        };
        Some(Rc::new(move |_| Some(msg.clone())))
    }
}

/// Owns the form, which owns every widget.
struct Login {
    form: LiveForm<Msg>,
}

impl App for Login {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::SignIn => {
                let user = self.form.get("user_edit", "text");
                let user = user.as_ref().and_then(Value::as_str).unwrap_or_default();
                let status = if user.is_empty() {
                    "Enter a user name".to_owned()
                } else {
                    format!("Signed in as {user}")
                };
                let _ = self.form.set("status_label", "text", &Value::Text(status));
            }
            Msg::Cancel => ui.quit(),
        }
    }
}

fn main() -> Result<()> {
    let catalog = Catalog::xui();
    let doc = FormDoc::from_toml(LOGIN, &catalog)
        .map_err(|error| BackendError::Other(format!("login.lfm: {error}")))?;
    let extent = |name| doc.window.prop(name).and_then(Value::as_int).unwrap_or(200) as f32;
    let title = doc
        .window
        .prop("title")
        .and_then(Value::as_str)
        .unwrap_or("Sign in");
    xui::app(title)
        .size(extent("width"), extent("height"))
        .run(|ui| {
            let form = build(ui, &doc, &catalog, &Factories::xui(), &LoginBinder)
                .map_err(|error| BackendError::Other(error.to_string()))?;
            Ok(Login { form })
        })
}
