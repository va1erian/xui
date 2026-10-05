//! Demonstrates the portable [`ColorPicker`]: choosing a swatch reports the
//! colour and a label prints its hex value.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_colorpicker
//! ```

use xui::prelude::*;

const PALETTE: [Color; 4] = [
    Color::hex(0x00_78_D4),
    Color::hex(0x10_7C_10),
    Color::hex(0xE8_11_23),
    Color::hex(0x87_64_B8),
];

#[derive(Clone)]
enum Msg {
    Select(Color),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(color) => {
                self.result.get().set_text(&format!(
                    "Accent #{:02X}{:02X}{:02X}",
                    color.r, color.g, color.b
                ));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("ColorPicker demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Accent #000000").bind(&demo.result),
                color_picker(&PALETTE)
                    .columns(2)
                    .selected(PALETTE[0])
                    .on_select(Msg::Select)
                    .size(240, 96),
            )),
        )?;
        Ok(demo)
    })
}
