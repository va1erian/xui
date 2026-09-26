//! Demonstrates the portable [`RadioGroup`]: choosing an option reports its
//! index and a label names it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_radiogroup
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, RadioGroup};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const OPTIONS: [&str; 3] = ["Small", "Medium", "Large"];

enum Msg {
    Select(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _radios: RadioGroup<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => {
                let name = OPTIONS.get(index).copied().unwrap_or("?");
                self.result.set_text(&format!("Selected {name}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("RadioGroup demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Selected Medium").unwrap();
            let radios = RadioGroup::new(ui, l.rect(16.0, 64.0, 320.0, 160.0), &OPTIONS)
                .unwrap()
                .on_select(|index| Some(Msg::Select(index)));
            radios.select(1);
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _radios: radios,
            }
        },
    )
}
