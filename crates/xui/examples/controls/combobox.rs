//! Demonstrates the portable [`ComboBox`]: picking an item reports its index
//! and a label names it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_combobox
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::icon::Lucide;
use xui_core::widget::{ComboBox, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const ITEMS: [&str; 3] = ["Alpha", "Beta", "Gamma"];

enum Msg {
    Select(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _combo: ComboBox<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(index) => {
                let name = ITEMS.get(index).copied().unwrap_or("?");
                self.result.set_text(&format!("Chose {name}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("ComboBox demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Nothing chosen").unwrap();
            let combo = ComboBox::new(ui, l.rect(16.0, 64.0, 320.0, 104.0), &ITEMS)
                .unwrap()
                .item_icon(0, Lucide::CircleDot)
                .item_icon(1, Lucide::Info)
                .item_icon(2, Lucide::TriangleAlert)
                .on_select(|index| Some(Msg::Select(index)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _combo: combo,
            }
        },
    )
}
