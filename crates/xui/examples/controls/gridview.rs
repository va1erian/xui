//! Demonstrates the portable [`GridView`] of plain tiles: picking a tile
//! reports its index to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_gridview
//! ```

use xui::prelude::*;

const TILES: [&str; 5] = ["One", "Two", "Three", "Four", "Five"];

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
                let name = TILES.get(index).copied().unwrap_or("?");
                self.result.get().set_text(&format!("Tile {index}: {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("GridView demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(column().padding(16).gap(16).children((
            label("Nothing selected").bind(&demo.result),
            grid_view(&TILES).on_select(Msg::Select).height(176),
        )))?;
        Ok(demo)
    })
}
