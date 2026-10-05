//! Demonstrates the portable [`Edit`] field: every keystroke raises a message
//! and a label reports the text length. A second field is a masked password
//! entry ([`Edit::password`]), pre-filled so it shows its bullets, with a cue
//! for when it is cleared; its label reports only the length.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_edit
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Text(String),
    Password(String),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let text = match msg {
            Msg::Text(text) => format!("{} characters", text.chars().count()),
            Msg::Password(text) => format!("password: {} characters", text.chars().count()),
        };
        self.result.get().set_text(&text);
    }
}

fn main() -> Result<()> {
    xui::app("Edit demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("0 characters").bind(&demo.result),
                edit().text("type here").on_change(Msg::Text),
                edit()
                    .text("correct horse")
                    .password()
                    .placeholder("Password")
                    .on_change(Msg::Password),
            )),
        )?;
        Ok(demo)
    })
}
