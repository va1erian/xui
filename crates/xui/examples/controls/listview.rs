//! Demonstrates the portable [`ListView`] over a tiny in-memory model: picking
//! a row reports its index to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_listview
//! ```

use xui::prelude::*;

const ROWS: [&str; 4] = ["Inbox", "Sent", "Drafts", "Archive"];

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
            Msg::Select(row) => {
                let name = ROWS.get(row).copied().unwrap_or("?");
                self.result
                    .get()
                    .set_text(&format!("Selected row {row}: {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("ListView demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Nothing selected").bind(&demo.result),
                list()
                    .then(|list| {
                        list.set_items(&ROWS);
                        list.select(Some(0));
                        list
                    })
                    .on_select(Msg::Select)
                    .height(176),
            )),
        )?;
        Ok(demo)
    })
}
