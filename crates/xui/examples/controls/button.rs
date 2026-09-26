//! Demonstrates the portable [`Button`]: a click raises a `Msg` and a label
//! reports how many times it was pressed.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_button
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Click,
    Quit,
}

struct Demo {
    result: Label<Msg>,
    clicks: u32,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Click => {
                self.clicks += 1;
                self.result
                    .set_text(&format!("Clicked {} times", self.clicks));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Button demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Not clicked yet").unwrap();
            let button = Button::new(ui, l.rect(16.0, 64.0, 256.0, 104.0), "Click me")
                .unwrap()
                .on_click(|| Some(Msg::Click));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                clicks: 0,
                _button: button,
            }
        },
    )
}
