//! Demonstrates the portable [`Toolbar`]: clicking a tool reports its index and
//! a label names it. The tools are generated [`Lucide`] icons with tooltips,
//! packed from the left in groups divided by separators (which take no index),
//! with the last two carrying a text label after the icon.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_toolbar
//! ```

use xui::Lucide;
use xui::prelude::*;

const TOOLS: [&str; 7] = ["New", "Open", "Save", "Cut", "Copy", "Run", "End"];

#[derive(Clone)]
enum Msg {
    Click(usize),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Click(index) => {
                let name = TOOLS.get(index).copied().unwrap_or("?");
                self.result.get().set_text(&format!("Tool {index}: {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Toolbar demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("No tool yet").bind(&demo.result),
                toolbar()
                    .item(Lucide::FilePlus, "New")
                    .item(Lucide::FolderOpen, "Open")
                    .item(Lucide::Save, "Save")
                    .separator()
                    .item(Lucide::Scissors, "Cut")
                    .item(Lucide::Copy, "Copy")
                    .separator()
                    .item_with_text(Lucide::Play, "Run", "Run")
                    .item_with_text(Lucide::Square, "End", "End")
                    .on_click(Msg::Click),
            )),
        )?;
        Ok(demo)
    })
}
