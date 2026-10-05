//! Demonstrates the portable [`Slider`]: dragging reports the value to a label
//! while the thumb moves.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_slider
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
            Msg::Value(value) => self.result.get().set_text(&format!("Slider: {value:.0}")),
        }
    }
}

fn main() -> Result<()> {
    xui::app("Slider demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Slider: 40").bind(&demo.result),
                slider(0.0, 100.0)
                    .then(|slider| {
                        slider.set_value(40.0);
                        slider
                    })
                    .on_change(Msg::Value),
            )),
        )?;
        Ok(demo)
    })
}
