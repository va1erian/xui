//! Demonstrates the portable [`ScrollView`] container: ten rows laid out in
//! `scroll(..)` scroll as one, and scrolling updates a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_scrollview
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Scroll(i32),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Scroll(offset) => self
                .result
                .get()
                .set_text(&format!("Scroll offset: {offset}px")),
        }
    }
}

fn main() -> Result<()> {
    xui::app("ScrollView demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        let rows: Vec<_> = (0..10)
            .map(|row| label(format!("Row {row}")).height(32))
            .collect();
        ui.root(
            column().padding(16).gap(16).children((
                scroll(column().children(rows))
                    .on_scroll(|offset| Msg::Scroll(offset.value()))
                    .height(224),
                label("Scroll offset: 0px").bind(&demo.result),
            )),
        )?;
        Ok(demo)
    })
}
