//! Demonstrates the portable [`FlowText`]: a line of normal, link and weak runs
//! that wraps; clicking the link updates a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_flowtext
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Link,
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Link => self.result.get().set_text("Documentation link clicked"),
        }
    }
}

fn main() -> Result<()> {
    xui::app("FlowText demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                flow_text()
                    .run(Run::normal("Read the "))
                    .run(Run::link("documentation").on_click(|| Some(Msg::Link)))
                    .separator(" · ")
                    .run(Run::weak("version 0.1")),
                label("No link clicked").bind(&demo.result),
            )),
        )?;
        Ok(demo)
    })
}
