//! Demonstrates the portable [`CheckBox`]: toggling reports the new state and a
//! label echoes it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_checkbox
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Toggle(bool),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle(checked) => {
                self.result
                    .get()
                    .set_text(if checked { "Enabled" } else { "Disabled" });
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("CheckBox demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(column().padding(16).gap(16).children((
            label("Disabled").bind(&demo.result),
            checkbox("Enable feature").on_toggle(Msg::Toggle),
        )))?;
        Ok(demo)
    })
}
