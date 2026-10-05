//! Demonstrates the portable [`ComboBox`]: picking an item reports its index
//! and a label names it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_combobox
//! ```

use xui::icon::Lucide;
use xui::prelude::*;

const ITEMS: [&str; 3] = ["Alpha", "Beta", "Gamma"];

#[derive(Clone)]
enum Msg {
    Select(usize),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => {
                let name = ITEMS.get(index).copied().unwrap_or("?");
                self.result.get().set_text(&format!("Chose {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("ComboBox demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Nothing chosen").bind(&demo.result),
                combo_box(&ITEMS)
                    .then(|combo| {
                        combo
                            .item_icon(0, Lucide::CircleDot)
                            .item_icon(1, Lucide::Info)
                            .item_icon(2, Lucide::TriangleAlert)
                    })
                    .on_select(Msg::Select)
                    .width(304),
            )),
        )?;
        Ok(demo)
    })
}
