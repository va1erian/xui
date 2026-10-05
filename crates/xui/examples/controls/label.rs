//! Demonstrates the portable [`Label`] and the `HasText` surface: a button
//! rewrites the label's text at run time.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_label
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Rewrite,
}

#[derive(Default)]
struct Demo {
    label: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Rewrite => {
                let label = self.label.get();
                let current = label.text();
                label.set_text(&format!("{current} *"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Label demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                label("Original text").bind(&demo.label),
                button("Append a star")
                    .on_click(Msg::Rewrite)
                    .width(240)
                    .align(Align::Start),
            )),
        )?;
        Ok(demo)
    })
}
