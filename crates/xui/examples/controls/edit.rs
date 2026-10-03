//! Demonstrates the portable [`Edit`] field: every keystroke raises a message
//! and a label reports the text length. A second field is a masked password
//! entry ([`Edit::password`]), pre-filled so it shows its bullets, with a cue
//! for when it is cleared; its label reports only the length.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_edit
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Edit, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Text(String),
    Password(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _edit: Edit<Msg>,
    _password: Edit<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Text(text) => {
                let chars = text.chars().count();
                self.result.set_text(&format!("{chars} characters"));
            }
            Msg::Password(chars) => {
                self.result
                    .set_text(&format!("password: {chars} characters"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Edit demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "0 characters").unwrap();
            let edit = Edit::new(ui, l.rect(16.0, 64.0, 504.0, 104.0), "type here")
                .unwrap()
                .on_change(|text| Some(Msg::Text(text.to_string())));
            let password = Edit::new(ui, l.rect(16.0, 120.0, 504.0, 160.0), "correct horse")
                .unwrap()
                .password(true)
                .cue("Password")
                .on_change(|text| Some(Msg::Password(text.chars().count())));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _edit: edit,
                _password: password,
            }
        },
    )
}
