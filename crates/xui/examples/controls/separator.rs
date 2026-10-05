//! Demonstrates the portable [`Separator`] in both orientations; a button
//! toggles its selected outline and a label reports the state.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_separator
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Toggle,
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
    horizontal: Handle<Separator<Msg>>,
    vertical: Handle<Separator<Msg>>,
    selected: bool,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle => {
                self.selected = !self.selected;
                self.horizontal.get().set_selected(self.selected);
                self.vertical.get().set_selected(self.selected);
                self.result.get().set_text(if self.selected {
                    "Separators selected"
                } else {
                    "Separators idle"
                });
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Separator demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(8).children((
                label("Separators idle").bind(&demo.result),
                label("Above"),
                separator().bind(&demo.horizontal),
                row()
                    .gap(16)
                    .children((
                        label("Left").fill(1),
                        vertical_separator().bind(&demo.vertical),
                        label("Right").width(124),
                    ))
                    .height(120),
                button("Toggle selected").on_click(Msg::Toggle).width(240),
            )),
        )?;
        Ok(demo)
    })
}
