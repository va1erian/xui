//! Demonstrates the portable [`Hyperlink`]: clicking the link raises a message
//! and a label reports it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_hyperlink
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Open,
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
    clicks: u32,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.clicks += 1;
                self.result
                    .get()
                    .set_text(&format!("Link opened {} times", self.clicks));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Hyperlink demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(column().padding(16).gap(16).children((
            label("Not opened yet").bind(&demo.result),
            hyperlink("Open the docs").on_click(Msg::Open),
        )))?;
        Ok(demo)
    })
}
