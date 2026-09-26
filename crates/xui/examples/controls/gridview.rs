//! Demonstrates the portable [`GridView`] of plain tiles: picking a tile
//! reports its index to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_gridview
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{GridView, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const TILES: [&str; 5] = ["One", "Two", "Three", "Four", "Five"];

enum Msg {
    Select(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _grid: GridView<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => {
                let name = TILES.get(index).copied().unwrap_or("?");
                self.result.set_text(&format!("Tile {index}: {name}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("GridView demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Nothing selected").unwrap();
            let grid = GridView::new(ui, l.rect(16.0, 64.0, 504.0, 240.0), &TILES)
                .unwrap()
                .on_select(|index| Some(Msg::Select(index)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _grid: grid,
            }
        },
    )
}
