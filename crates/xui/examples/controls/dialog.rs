//! Demonstrates the portable [`Dialog`]: a confirm dialog opened from a button;
//! the chosen action is reported by a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_dialog
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, Dialog, DialogAction, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Open,
    Action(DialogAction),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    dialog: Dialog<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.dialog.open();
                self.result.set_text("Dialog opened");
            }
            Msg::Action(action) => match action {
                DialogAction::Accept(text) if text.is_empty() => self.result.set_text("Accepted"),
                DialogAction::Accept(text) => self.result.set_text(&format!("Accepted: {text}")),
                DialogAction::Cancel => self.result.set_text("Cancelled"),
            },
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Dialog demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let button = Button::new(ui, l.rect(16.0, 16.0, 256.0, 56.0), "Show dialog")
                .unwrap()
                .on_click(|| Some(Msg::Open));
            let dialog = Dialog::confirm(ui, "Save changes?", "Your edits will be lost.")
                .unwrap()
                .accept_label("Save")
                .on_action(|action| Some(Msg::Action(action)));
            let result = Label::new(ui, l.rect(16.0, 72.0, 504.0, 104.0), "No action yet").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                dialog,
                _button: button,
            }
        },
    )
}
