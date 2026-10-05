//! Demonstrates the portable [`Panel`] container: its content is a layout of
//! its own, and a button inside it updates a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_panel
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Bump,
}

#[derive(Default)]
struct Demo {
    inside: Handle<Label<Msg>>,
    result: Handle<Label<Msg>>,
    clicks: u32,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Bump => {
                self.clicks += 1;
                self.inside
                    .get()
                    .set_text(&format!("{} clicks", self.clicks));
                self.result.get().set_text("Button inside panel pressed");
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Panel demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                panel(column().padding(12).gap(16).children((
                    label("0 clicks").bind(&demo.inside),
                    button("Count").on_click(Msg::Bump),
                )))
                .width(304)
                .align(Align::Start),
                label("Panel ready").bind(&demo.result),
            )),
        )?;
        Ok(demo)
    })
}
