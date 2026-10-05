//! A sign-in form loaded from a `.lfm` file and built into widgets.
//!
//! The form is data (`forms/login.lfm`): [`load`] reads it, [`build`] turns
//! it into the same builders Rust code writes, and [`Handlers`] attach the
//! app's messages to events by name.
//!
//! Run it with `cargo run -p xui --features form --example form`.

use xui::BackendError;
use xui::form::{Factories, Handlers, LiveForm, Value, build, load};
use xui::prelude::*;

/// The form, embedded so the example runs from any directory.
const LOGIN: &str = include_str!("forms/login.lfm");

#[derive(Clone, Debug)]
enum Msg {
    SignIn,
    Cancel,
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
    let form = load(LOGIN).map_err(|error| BackendError::Other(format!("login.lfm:{error}")))?;
    let handlers = Handlers::new()
        .on("sign_in_button", "Click", Msg::SignIn)
        .on("cancel_button", "Click", Msg::Cancel);
    xui::app(form.title.clone())
        .size(form.size.0.dip(), form.size.1.dip())
        .run(|ui| {
            let form = build(ui, &form, &Factories::xui(), &handlers)
                .map_err(|error| BackendError::Other(error.to_string()))?;
            Ok(Login { form })
        })
}
