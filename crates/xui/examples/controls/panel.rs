//! Demonstrates the portable [`Panel`] container: children are created through
//! `panel.ui()` and a button inside it updates a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_panel
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Label, Panel};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Bump,
    Quit,
}

struct Demo {
    inside: Label<Msg>,
    result: Label<Msg>,
    clicks: u32,
    _panel: Panel<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Bump => {
                self.clicks += 1;
                self.inside.set_text(&format!("{} clicks", self.clicks));
                self.result.set_text("Button inside panel pressed");
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Panel demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let panel = Panel::new(ui, l.rect(16.0, 16.0, 320.0, 180.0)).unwrap();
            let inside =
                Label::new(panel.ui(), l.rect(12.0, 12.0, 288.0, 48.0), "0 clicks").unwrap();
            let button = Button::new(panel.ui(), l.rect(12.0, 64.0, 288.0, 104.0), "Count")
                .unwrap()
                .on_click(|| Some(Msg::Bump));
            let result = Label::new(ui, l.rect(16.0, 196.0, 504.0, 228.0), "Panel ready").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                inside,
                result,
                clicks: 0,
                _panel: panel,
                _button: button,
            }
        },
    )
}
