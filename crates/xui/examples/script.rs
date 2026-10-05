//! A scripted form: a `.lfm` document plus a `.rhai` script, with no Rust
//! event code.
//!
//! [`ScriptForm::build`] builds the form's widgets, wires every
//! `<control>_<event>` handler the script defines and runs `form_load`; the
//! app only routes each widget event back into the script.
//!
//! Run it with `cargo run -p xui --features rhai --example script`.

use xui::BackendError;
use xui::form::{Catalog, FormDoc, Value};
use xui::prelude::*;
use xui::script::Msg;
use xui::script::form::{ScriptForm, ScriptSource};

/// The form and its script, embedded so the example runs from any directory.
const FORM: &str = include_str!("forms/hello.lfm");
const CODE: &str = include_str!("forms/hello.rhai");

/// Owns the scripted form and routes its widget events into the script.
struct Scripted {
    form: ScriptForm,
}

impl App for Scripted {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Event {
                control,
                event,
                args,
                ..
            } => {
                if let Err(error) = self.form.run(&control, &event, &args) {
                    eprintln!("hello.rhai: {error}");
                }
            }
            Msg::Quit => ui.quit(),
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let catalog = Catalog::xui();
    let doc = FormDoc::from_toml(FORM, &catalog)
        .map_err(|error| BackendError::Other(format!("hello.lfm: {error}")))?;
    let extent = |name| doc.window.prop(name).and_then(Value::as_int).unwrap_or(200) as f32;
    let title = doc
        .window
        .prop("title")
        .and_then(Value::as_str)
        .unwrap_or("Hello");
    xui::app(title)
        .size(extent("width"), extent("height"))
        .run(|ui| {
            let source = ScriptSource {
                name: "main_form",
                code: CODE,
                file: "hello.rhai",
            };
            let form = ScriptForm::build(ui, &doc, &catalog, source, (), |_| Ok(()))
                .map_err(|error| BackendError::Other(error.to_string()))?;
            Ok(Scripted { form })
        })
}
