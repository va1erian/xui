//! A scripted calculator: a `.lfm` form plus a `.rhai` script, with no Rust
//! event code.
//!
//! The digits and operators are control arrays, so the script has one
//! handler for each (`fn digit_click(index)`). [`ScriptForm::build`] builds
//! the widgets, wires every handler the script defines and runs
//! `form_load`; the app only routes widget events back into the script.
//!
//! Run it with `cargo run -p xui --features rhai --example script`.

use xui::BackendError;
use xui::form::load;
use xui::prelude::*;
use xui::script::Msg;
use xui::script::form::{ScriptForm, ScriptSource};

/// The form and its script, embedded so the example runs from any directory.
const FORM: &str = include_str!("forms/calculator.lfm");
const CODE: &str = include_str!("forms/calculator.rhai");

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
                    eprintln!("{error}");
                }
            }
            Msg::Quit => ui.quit(),
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let form =
        load(FORM).map_err(|error| BackendError::Other(format!("calculator.lfm:{error}")))?;
    xui::app(form.title.clone())
        .size(form.size.0.dip(), form.size.1.dip())
        .run(|ui| {
            let source = ScriptSource {
                name: &form.name,
                code: CODE,
                file: "calculator.rhai",
            };
            let form = ScriptForm::build(ui, &form, source, (), |_| Ok(()))
                .map_err(|error| BackendError::Other(error.to_string()))?;
            Ok(Scripted { form })
        })
}
