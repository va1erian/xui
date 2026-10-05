//! Demonstrates the portable [`Tabs`] container: each page holds a label laid
//! out by its own layout, and changing tab updates a result label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_tabs
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Tab(usize),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Tab(index) => {
                let name = if index == 0 { "General" } else { "Advanced" };
                self.result.get().set_text(&format!("Tab {index}: {name}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Tabs demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(8).children((
                tabs()
                    .page("General", column().child(label("General page")))
                    .page("Advanced", column().child(label("Advanced page")))
                    .on_change(Msg::Tab)
                    .height(224),
                label("Tab 0: General").bind(&demo.result),
            )),
        )?;
        Ok(demo)
    })
}
