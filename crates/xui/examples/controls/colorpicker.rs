//! Demonstrates the portable [`ColorPicker`]: choosing a swatch reports the
//! colour and a label prints its hex value.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_colorpicker
//! ```

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{ColorPicker, HasText, Label};
use xui_core::{Color, Dip};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const PALETTE: [Color; 4] = [
    Color::hex(0x00_78_D4),
    Color::hex(0x10_7C_10),
    Color::hex(0xE8_11_23),
    Color::hex(0x87_64_B8),
];

enum Msg {
    Select(Color),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _swatches: ColorPicker<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(color) => {
                self.result.set_text(&format!(
                    "Accent #{:02X}{:02X}{:02X}",
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
        PlatformSpec::new("ColorPicker demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Accent #000000").unwrap();
            let swatches = ColorPicker::new(ui, l.rect(16.0, 64.0, 256.0, 160.0), &PALETTE)
                .unwrap()
                .columns(2)
                .selected(PALETTE[0])
                .on_select(|color| Some(Msg::Select(color)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _swatches: swatches,
            }
        },
    )
}
