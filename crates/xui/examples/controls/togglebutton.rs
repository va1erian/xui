//! Demonstrates the portable [`ToggleButton`]: pressing it reports the new
//! checked state to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_togglebutton
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, ToggleButton};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Toggle(bool),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _toggle: ToggleButton<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle(checked) => {
                self.result
                    .set_text(if checked { "Bold on" } else { "Bold off" });
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("ToggleButton demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Bold off").unwrap();
            let toggle = ToggleButton::new(ui, l.rect(16.0, 64.0, 256.0, 104.0), "Bold")
                .unwrap()
                .on_toggle(|checked| Some(Msg::Toggle(checked)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _toggle: toggle,
            }
        },
    )
}
