//! Demonstrates the portable [`Button`]: a click raises a `Msg` and a label
//! reports how many times it was pressed. The buttons carry generated
//! [`Lucide`] icons, one labelled and one icon-only.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_button
//! ```

use xui::icon::Lucide;
use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Click,
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
            Msg::Click => {
                self.clicks += 1;
                self.result
                    .get()
                    .set_text(&format!("Clicked {} times", self.clicks));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Button demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Not clicked yet").bind(&demo.result),
                row().gap(12).children((
                    button("Click me")
                        .icon(Lucide::Play)
                        .on_click(Msg::Click)
                        .width(284),
                    button("").icon(Lucide::Save).on_click(Msg::Click).width(48),
                )),
            )),
        )?;
        Ok(demo)
    })
}
