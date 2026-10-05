//! Demonstrates the portable [`TaskDialog`]: a warning card with two command
//! buttons and a verification checkbox; the choice is reported by a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --example control_taskdialog
//! ```

use xui::prelude::*;
use xui::widget::{TaskDialog, TaskDialogAction, TaskDialogIcon};

#[derive(Clone)]
enum Msg {
    Open,
    Action(TaskDialogAction),
}

struct Demo {
    result: Handle<Label<Msg>>,
    dialog: TaskDialog<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.dialog.open();
                self.result.get().set_text("Task dialog opened");
            }
            Msg::Action(action) => {
                let text = match action {
                    TaskDialogAction::Command(0) => "Chose: Delete".to_string(),
                    TaskDialogAction::Command(index) => format!("Chose command {index}"),
                    TaskDialogAction::Cancel => "Cancelled".to_string(),
                };
                let remembered = if self.dialog.is_checked() {
                    " (remembered)"
                } else {
                    ""
                };
                self.result.get().set_text(&format!("{text}{remembered}"));
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("TaskDialog demo").size(560, 400).run(|ui| {
        let result = Handle::new();
        ui.root(column().padding(16).gap(16).children((
            button("Delete files").on_click(Msg::Open).width(240),
            label("No action yet").bind(&result),
        )))?;
        let dialog = TaskDialog::new(ui, "Delete 3 files?", "Deleted files cannot be recovered.")?
            .icon(TaskDialogIcon::Warning)
            .command("Delete")?
            .command("Keep")?
            .verification("Don't ask me again")?
            .on_action(|action| Some(Msg::Action(action)));
        // Open once up front so the card is visible without a click (and so
        // the demo is screenshot-able).
        dialog.open();
        Ok(Demo { result, dialog })
    })
}
