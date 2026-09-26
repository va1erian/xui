//! Demonstrates the portable [`MaterialStatusBar`]: a button writes into its
//! second part, so the material band itself is the live result.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_materialstatusbar
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, MaterialStatusBar};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Notify,
    Quit,
}

struct Demo {
    status: MaterialStatusBar<Msg>,
    notifications: u32,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Notify => {
                self.notifications += 1;
                self.status
                    .set_text(1, &format!("{} notifications", self.notifications));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("MaterialStatusBar demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let status = MaterialStatusBar::new(
                ui,
                l.rect(16.0, 16.0, 504.0, 60.0),
                &["Native", "No notifications"],
            )
            .unwrap();
            let button = Button::new(ui, l.rect(16.0, 80.0, 256.0, 120.0), "Notify")
                .unwrap()
                .on_click(|| Some(Msg::Notify));
            autoclose(ui, || Msg::Quit);
            Demo {
                status,
                notifications: 0,
                _button: button,
            }
        },
    )
}
