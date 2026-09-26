//! Demonstrates the portable [`ProgressBar`] driven by a [`Slider`]: the bar
//! follows the slider and a label reports the percentage.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_progressbar
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, ProgressBar, Slider};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Value(f64),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    bar: ProgressBar<Msg>,
    _slider: Slider<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Value(value) => {
                self.bar.set_value(value as i32);
                self.result.set_text(&format!("Progress: {value:.0}%"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("ProgressBar demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Progress: 40%").unwrap();
            let bar = ProgressBar::new(ui, l.rect(16.0, 64.0, 504.0, 80.0), 100).unwrap();
            bar.set_value(40);
            let slider = Slider::new(ui, l.rect(16.0, 96.0, 504.0, 136.0), 0.0, 100.0)
                .unwrap()
                .on_change(|value| Some(Msg::Value(value)));
            slider.set_value(40.0);
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                bar,
                _slider: slider,
            }
        },
    )
}
