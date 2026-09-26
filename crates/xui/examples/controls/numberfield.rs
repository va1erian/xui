//! Demonstrates the portable [`NumberField`]: stepping the spinner reports the
//! new value to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_numberfield
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, NumberField};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Value(f64),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _number: NumberField<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Value(value) => {
                self.result.set_text(&format!("Value: {value:.0}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("NumberField demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Value: 20").unwrap();
            let number = NumberField::new(ui, l.rect(16.0, 64.0, 256.0, 104.0), 0.0, 100.0, 5.0)
                .unwrap()
                .on_change(|value| Some(Msg::Value(value)));
            number.set_value(20.0);
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _number: number,
            }
        },
    )
}
