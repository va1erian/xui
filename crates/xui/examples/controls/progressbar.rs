//! Demonstrates the portable [`ProgressBar`] driven by a [`Slider`]: the bar
//! follows the slider and a label reports the percentage.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_progressbar
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Value(f64),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
    bar: Handle<ProgressBar<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Value(value) => {
                self.bar.get().set_value(value as i32);
                self.result
                    .get()
                    .set_text(&format!("Progress: {value:.0}%"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("ProgressBar demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Progress: 40%").bind(&demo.result),
                progress(100).value(40).bind(&demo.bar),
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
