//! Demonstrates the portable [`Tooltip`]: attached to a button, it appears on
//! hover; clicking the button shows or hides it explicitly.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_tooltip
//! ```

use xui::prelude::*;
use xui::widget::Tooltip;

#[derive(Clone)]
enum Msg {
    Toggle,
}

struct Demo {
    result: Handle<Label<Msg>>,
    tip: Tooltip<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle => {
                if self.tip.is_visible() {
                    self.tip.hide();
                    self.result.get().set_text("Tooltip hidden");
                } else {
                    self.tip.show();
                    self.result.get().set_text("Tooltip shown");
                }
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Tooltip demo").size(520, 360).run(|ui| {
        let (button_handle, result) = (Handle::<Button<Msg>>::new(), Handle::new());
        ui.root(
            column().padding(16).gap(16).children((
                button("Hover or click me")
                    .on_click(Msg::Toggle)
                    .bind(&button_handle)
                    .width(240),
                label("Tooltip hidden").bind(&result),
            )),
        )?;
        let tip = Tooltip::attach(ui, button_handle.get().id(), "This is a tooltip")?;
        Ok(Demo { result, tip })
    })
}
