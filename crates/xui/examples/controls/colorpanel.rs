//! Demonstrates the portable [`ColorPanel`]: a **Simple** basic-colour grid
//! and a **Full** HSV picker with editable HEX/RGB/CMYK/HSV/HSL boxes. Editing
//! either panel updates the label with the chosen colour.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_colorpanel
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Change(Color),
    Commit(Color),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let (verb, color) = match msg {
            Msg::Change(color) => ("Changing", color),
            Msg::Commit(color) => ("Committed", color),
        };
        self.result.get().set_text(&format!(
            "{verb} #{:02X}{:02X}{:02X}",
            color.r, color.g, color.b
        ));
    }
}

/// A colour panel starting at `color` that reports edits and commits.
fn panel_at(color: Color) -> Build<ColorPanel<Msg>, Msg> {
    color_panel()
        .color(color)
        .on_change(Msg::Change)
        .then(|panel| panel.on_commit(|color| Some(Msg::Commit(color))))
}

fn main() -> Result<()> {
    xui::app("ColorPanel demo").size(1200, 470).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(8).children((
                label("Pick a colour from either tab.").bind(&demo.result),
                row()
                    .gap(16)
                    .children((
                        panel_at(Color::hex(0x00_78_D4)).fill(1),
                        panel_at(Color::hex(0xEB_40_34))
                            .then(|panel| {
                                panel.select_tab(1);
                                panel
                            })
                            .fill(1),
                    ))
                    .fill(1),
            )),
        )?;
        Ok(demo)
    })
}
