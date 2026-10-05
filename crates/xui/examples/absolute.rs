//! `absolute()`: the one layout for free positions, as a form designer or a
//! form imported from coordinates uses it. Each widget sits where `at` puts
//! it in a 400x240 design; anchors decide how it follows a resize: the notes
//! stretch, the buttons stay in the bottom-right corner.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example absolute
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself; `XUI_SNAPSHOT=<dir>` saves a
//! light and a dark screenshot instead of opening a window.

use xui::prelude::*;

struct Form;

impl App for Form {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn main() -> Result<()> {
    xui::app("Absolute layout").size(400, 240).run(|ui| {
        ui.root(
            absolute().design_size(400, 240).children((
                label("Name").at(16, 20, 80, 28),
                edit()
                    .placeholder("Your name")
                    .at(100, 20, 284, 28)
                    .anchor(Anchor::StretchHorizontal),
                label("Notes").at(16, 60, 80, 28),
                multiline_edit().at(100, 60, 284, 120).anchor(Anchor::Fill),
                button("Cancel")
                    .at(212, 196, 80, 28)
                    .anchor(Anchor::BottomRight),
                button("OK")
                    .primary()
                    .at(304, 196, 80, 28)
                    .anchor(Anchor::BottomRight),
            )),
        )?;
        Ok(Form)
    })
}
