//! Demonstrates the portable [`Toolbar`]: clicking a tool reports its index and
//! a label names it. The tools are generated [`Lucide`] icons with tooltips,
//! packed from the left in groups divided by separators (which take no index),
//! with the last two carrying a text label after the icon. The last group are
//! checkable tools: clicking one latches it, and `update` unchecks the others
//! so they act as a radio group.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_toolbar
//! ```

use xui::Lucide;
use xui::prelude::*;

const TOOLS: [&str; 10] = [
    "New", "Open", "Save", "Cut", "Copy", "Run", "End", "Brush", "Bucket", "Grid",
];
/// The first checkable item and how many there are (the paint tools exclude
/// each other; the grid is an independent toggle).
const FIRST_TOOL: usize = 7;
const TOOL_COUNT: usize = 2;
const GRID: usize = 9;

#[derive(Clone)]
enum Msg {
    Click(usize),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
    bar: Handle<Toolbar<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Click(index) => {
                let name = TOOLS.get(index).copied().unwrap_or("?");
                let bar = self.bar.get();
                if (FIRST_TOOL..FIRST_TOOL + TOOL_COUNT).contains(&index) {
                    // A tool stays active: clicking it again does not unlatch it.
                    for tool in FIRST_TOOL..FIRST_TOOL + TOOL_COUNT {
                        bar.set_checked(tool, tool == index);
                    }
                }
                let state = match bar.is_checked(index) {
                    Some(true) => " (on)",
                    Some(false) => " (off)",
                    None => "",
                };
                self.result
                    .get()
                    .set_text(&format!("Tool {index}: {name}{state}"));
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
                    .separator()
                    .toggle_item(Lucide::Brush, "Brush")
                    .toggle_item(Lucide::PaintBucket, "Bucket")
                    .toggle_item(Lucide::Grid3x3, "Grid")
                    .checked(FIRST_TOOL, true)
                    .checked(GRID, true)
                    .bind(&demo.bar)
                    .on_click(Msg::Click),
            )),
        )?;
        Ok(demo)
    })
}
