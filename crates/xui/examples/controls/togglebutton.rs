//! Demonstrates the portable [`ToggleButton`]: pressing one reports the new
//! checked state to a label. One has an icon and a label, one an icon only.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_togglebutton
//! ```

use xui::Lucide;
use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Bold(bool),
    Italic(bool),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let text = match msg {
            Msg::Bold(checked) => {
                if checked {
                    "Bold on"
                } else {
                    "Bold off"
                }
            }
            Msg::Italic(checked) => {
                if checked {
                    "Italic on"
                } else {
                    "Italic off"
                }
            }
        };
        self.result.get().set_text(text);
    }
}

fn main() -> Result<()> {
    xui::app("ToggleButton demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Bold off").bind(&demo.result),
                row().gap(12).children((
                    toggle_button("Bold")
                        .icon(Lucide::Bold)
                        .on_toggle(Msg::Bold)
                        .width(240),
                    toggle_button("")
                        .icon(Lucide::Italic)
                        .on_toggle(Msg::Italic)
                        .width(40),
                )),
            )),
        )?;
        Ok(demo)
    })
}
