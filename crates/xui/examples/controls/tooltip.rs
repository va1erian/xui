//! Demonstrates the portable [`Tooltip`]: attached to a button, it appears on
//! hover; clicking the button shows or hides it explicitly.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_tooltip
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Label, Tooltip};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Toggle,
    Quit,
}

struct Demo {
    result: Label<Msg>,
    tip: Tooltip<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle => {
                if self.tip.is_visible() {
                    self.tip.hide();
                    self.result.set_text("Tooltip hidden");
                } else {
                    self.tip.show();
                    self.result.set_text("Tooltip shown");
                }
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Tooltip demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let button = Button::new(ui, l.rect(16.0, 16.0, 256.0, 56.0), "Hover or click me")
                .unwrap()
                .on_click(|| Some(Msg::Toggle));
            let tip = Tooltip::attach(ui, button.id(), "This is a tooltip").unwrap();
            let result =
                Label::new(ui, l.rect(16.0, 72.0, 504.0, 104.0), "Tooltip hidden").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                tip,
                _button: button,
            }
        },
    )
}
