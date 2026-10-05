//! Demonstrates the portable [`MaterialStatusBar`]: a button writes into its
//! second part, so the material band itself is the live result.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_materialstatusbar
//! ```

use xui::prelude::*;

#[derive(Clone)]
enum Msg {
    Notify,
}

#[derive(Default)]
struct Demo {
    status: Handle<MaterialStatusBar<Msg>>,
    notifications: u32,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Notify => {
                self.notifications += 1;
                self.status
                    .get()
                    .set_text(1, &format!("{} notifications", self.notifications));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("MaterialStatusBar demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        ui.root(
            column().padding(16).gap(16).children((
                material_status_bar(&["Native", "No notifications"])
                    .bind(&demo.status)
                    .height(44),
                button("Notify")
                    .on_click(Msg::Notify)
                    .width(240)
                    .align(Align::Start),
            )),
        )?;
        Ok(demo)
    })
}
