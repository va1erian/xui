//! Demonstrates the portable [`ListView`] over a tiny in-memory model: picking
//! a row reports its index to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_listview
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, ListView};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const ROWS: [&str; 4] = ["Inbox", "Sent", "Drafts", "Archive"];

enum Msg {
    Select(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _list: ListView<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Select(row) => {
                let name = ROWS.get(row).copied().unwrap_or("?");
                self.result.set_text(&format!("Selected row {row}: {name}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("ListView demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Nothing selected").unwrap();
            let list = ListView::new(ui, l.rect(16.0, 64.0, 504.0, 240.0), &ROWS)
                .unwrap()
                .on_select(|row| Some(Msg::Select(row)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _list: list,
            }
        },
    )
}
