//! Demonstrates the portable [`MultilineEdit`]: editing raises a message and a
//! label reports lines and characters.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_multilineedit
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Text(String),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Text(text) => {
                let lines = text.lines().count().max(1);
                self.result.get().set_text(&format!(
                    "{lines} lines, {} characters",
                    text.chars().count()
                ));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("MultilineEdit demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("1 line, 0 characters").bind(&demo.result),
                multiline_edit()
                    .then(|edit| {
                        edit.set_text("Write notes…");
                        edit
                    })
                    .on_change(Msg::Text)
                    .height(136),
            )),
        )?;
        Ok(demo)
    })
}
