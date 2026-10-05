//! Demonstrates the portable [`GroupBox`] frame around a [`CheckBox`]: the
//! checkbox reports its state to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_groupbox
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
                    .set_text(if checked { "Option on" } else { "Option off" });
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("GroupBox demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Option off").bind(&demo.result),
                group(
                    "Settings",
                    column()
                        .padding(16)
                        .child(checkbox("Enable option").on_toggle(Msg::Toggle)),
                ),
            )),
        )?;
        Ok(demo)
    })
}
