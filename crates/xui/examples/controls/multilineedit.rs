//! Demonstrates the portable [`MultilineEdit`]: editing raises a message and a
//! label reports lines and characters.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_multilineedit
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, MultilineEdit};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Text(String),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _edit: MultilineEdit<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Text(text) => {
                let lines = text.lines().count().max(1);
                self.result.set_text(&format!(
                    "{lines} lines, {} characters",
                    text.chars().count()
                ));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("MultilineEdit demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "1 line, 0 characters").unwrap();
            let edit = MultilineEdit::new(ui, l.rect(16.0, 64.0, 504.0, 200.0), "Write notes…")
                .unwrap()
                .on_change(|text| Some(Msg::Text(text.to_string())));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _edit: edit,
            }
        },
    )
}
