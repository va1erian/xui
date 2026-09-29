//! Demonstrates the portable [`ColorPanel`]: a **Simple** basic-colour grid
//! and a **Full** HSV picker with editable HEX/RGB/CMYK/HSV/HSL boxes. Editing
//! either panel updates the label with the chosen colour.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_colorpanel
//! ```

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{ColorPanel, HasText, Label};
use xui_core::{Color, Dip};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Change(Color),
    Commit(Color),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _simple: ColorPanel<Msg>,
    _full: ColorPanel<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Change(color) => {
                self.result.set_text(&format!(
                    "Changing #{:02X}{:02X}{:02X}",
                    color.r, color.g, color.b
                ));
            }
            Msg::Commit(color) => {
                self.result.set_text(&format!(
                    "Committed #{:02X}{:02X}{:02X}",
                    color.r, color.g, color.b
                ));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("ColorPanel demo").size(Dip(1200.0), Dip(470.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(
                ui,
                l.rect(16.0, 16.0, 1184.0, 48.0),
                "Pick a colour from either tab.",
            )
            .unwrap();
            let simple = ColorPanel::new(ui, l.rect(16.0, 56.0, 590.0, 448.0))
                .unwrap()
                .with_color(Color::hex(0x00_78_D4))
                .on_change(|color| Some(Msg::Change(color)))
                .on_commit(|color| Some(Msg::Commit(color)));
            let full = ColorPanel::new(ui, l.rect(606.0, 56.0, 1184.0, 448.0))
                .unwrap()
                .with_color(Color::hex(0xEB_40_34))
                .on_change(|color| Some(Msg::Change(color)))
                .on_commit(|color| Some(Msg::Commit(color)));
            full.select_tab(1);
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _simple: simple,
                _full: full,
            }
        },
    )
}
