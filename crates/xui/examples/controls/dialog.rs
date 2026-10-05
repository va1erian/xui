//! Demonstrates the portable [`Dialog`]: a confirm dialog opened from a button;
//! the chosen action is reported by a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_dialog
//! ```

use xui::prelude::*;
use xui::widget::{Dialog, DialogAction};

#[derive(Clone)]
enum Msg {
    Open,
    Action(DialogAction),
}

struct Demo {
    result: Handle<Label<Msg>>,
    dialog: Dialog<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        let result = self.result.get();
        match msg {
            Msg::Open => {
                self.dialog.open();
                result.set_text("Dialog opened");
            }
            Msg::Action(action) => match action {
                DialogAction::Accept(text) if text.is_empty() => result.set_text("Accepted"),
                DialogAction::Accept(text) => result.set_text(&format!("Accepted: {text}")),
                DialogAction::Cancel => result.set_text("Cancelled"),
            },
        }
    }
}

fn main() -> Result<()> {
    xui::app("Dialog demo").size(520, 360).run(|ui| {
        let result = Handle::new();
        ui.root(column().padding(16).gap(16).children((
            button("Show dialog").on_click(Msg::Open).width(240),
            label("No action yet").bind(&result),
        )))?;
        let dialog = Dialog::confirm(ui, "Save changes?", "Your edits will be lost.")?
            .accept_label("Save")
            .on_action(|action| Some(Msg::Action(action)));
        Ok(Demo { result, dialog })
    })
}
