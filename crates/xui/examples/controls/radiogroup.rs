//! Demonstrates the portable [`RadioGroup`]: choosing an option reports its
//! index and a label names it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_radiogroup
//! ```

use xui::prelude::*;

const OPTIONS: [&str; 3] = ["Small", "Medium", "Large"];

#[derive(Clone)]
enum Msg {
    Select(usize),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => {
                let name = OPTIONS.get(index).copied().unwrap_or("?");
                self.result.get().set_text(&format!("Selected {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("RadioGroup demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Selected Medium").bind(&demo.result),
                radio_group(&OPTIONS)
                    .selected(1)
                    .on_select(Msg::Select)
                    .width(304)
                    .align(Align::Start),
            )),
        )?;
        Ok(demo)
    })
}
