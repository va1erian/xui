//! Demonstrates the portable [`TaskDialog`]: a warning card with two command
//! buttons and a verification checkbox; the choice is reported by a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_taskdialog
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Label, TaskDialog, TaskDialogAction, TaskDialogIcon};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Open,
    Action(TaskDialogAction),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    dialog: TaskDialog<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.dialog.open();
                self.result.set_text("Task dialog opened");
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
                self.result.set_text(&format!("{text}{remembered}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("TaskDialog demo").size(Dip(560.0), Dip(400.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let button = Button::new(ui, l.rect(16.0, 16.0, 256.0, 56.0), "Delete files")
                .unwrap()
                .on_click(|| Some(Msg::Open));
            let dialog =
                TaskDialog::new(ui, "Delete 3 files?", "Deleted files cannot be recovered.")
                    .unwrap()
                    .icon(TaskDialogIcon::Warning)
                    .command("Delete")
                    .unwrap()
                    .command("Keep")
                    .unwrap()
                    .verification("Don't ask me again")
                    .unwrap()
                    .on_action(|action| Some(Msg::Action(action)));
            let result = Label::new(ui, l.rect(16.0, 72.0, 544.0, 104.0), "No action yet").unwrap();
            autoclose(ui, || Msg::Quit);
            let demo = Demo {
                result,
                dialog,
                _button: button,
            };
            // Open once up front so the card is visible without a click (and so
            // the demo is screenshot-able).
            demo.dialog.open();
            demo
        },
    )
}
