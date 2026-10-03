//! Demonstrates the portable [`ToggleButton`]: pressing one reports the new
//! checked state to a label. One has an icon and a label, one an icon only.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_togglebutton
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::icon::Lucide;
use xui_core::widget::{HasText, Label, ToggleButton};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Bold(bool),
    Italic(bool),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _toggles: Vec<ToggleButton<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Bold(checked) => {
                self.result
                    .set_text(if checked { "Bold on" } else { "Bold off" });
            }
            Msg::Italic(checked) => {
                self.result
                    .set_text(if checked { "Italic on" } else { "Italic off" });
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
            let bold = ToggleButton::new(ui, l.rect(16.0, 64.0, 256.0, 104.0), "Bold")
                .unwrap()
                .icon(Lucide::Bold)
                .on_toggle(|checked| Some(Msg::Bold(checked)));
            let italic = ToggleButton::new(ui, l.rect(268.0, 64.0, 308.0, 104.0), "")
                .unwrap()
                .icon(Lucide::Italic)
                .on_toggle(|checked| Some(Msg::Italic(checked)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _toggles: vec![bold, italic],
            }
        },
    )
}
