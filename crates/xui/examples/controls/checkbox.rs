//! Demonstrates the portable [`CheckBox`]: toggling reports the new state and a
//! label echoes it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_checkbox
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{CheckBox, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Toggle(bool),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _check: CheckBox<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle(checked) => {
                self.result
                    .set_text(if checked { "Enabled" } else { "Disabled" });
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("CheckBox demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Disabled").unwrap();
            let check = CheckBox::new(ui, l.rect(16.0, 64.0, 320.0, 104.0), "Enable feature")
                .unwrap()
                .on_toggle(|checked| Some(Msg::Toggle(checked)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _check: check,
            }
        },
    )
}
