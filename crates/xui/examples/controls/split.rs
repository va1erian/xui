//! Demonstrates the portable [`Split`] container: each pane holds a label laid
//! out by its own layout, and moving the divider updates a result label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_split
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Moved(f32),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Moved(position) => self
                .result
                .get()
                .set_text(&format!("Divider at {position:.0}")),
        }
    }
}

fn main() -> Result<()> {
    xui::app("Split demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(8).children((
                split(
                    column().child(label("Left pane")),
                    column().child(label("Right pane")),
                )
                .min(60, 60)
                .position(244)
                .on_moved(|position| Msg::Moved(position.value()))
                .height(224),
                label("Divider at 244").bind(&demo.result),
            )),
        )?;
        Ok(demo)
    })
}
