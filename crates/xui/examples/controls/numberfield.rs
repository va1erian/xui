//! Demonstrates the portable [`NumberField`]: stepping the spinner reports the
//! new value to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_numberfield
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Value(f64),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Value(value) => self.result.get().set_text(&format!("Value: {value:.0}")),
        }
    }
}

fn main() -> Result<()> {
    xui::app("NumberField demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Value: 20").bind(&demo.result),
                number_field(0.0, 100.0, 5.0)
                    .then(|field| {
                        field.set_value(20.0);
                        field
                    })
                    .on_change(Msg::Value)
                    .width(240)
                    .align(Align::Start),
            )),
        )?;
        Ok(demo)
    })
}
