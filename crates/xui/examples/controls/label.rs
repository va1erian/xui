//! Demonstrates the portable [`Label`] and the `HasText` surface: a button
//! rewrites the label's text at run time.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_label
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Rewrite,
    Quit,
}

struct Demo {
    label: Label<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Rewrite => {
                let current = self.label.text();
                self.label.set_text(&format!("{current} *"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Label demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let label = Label::new(ui, l.rect(16.0, 16.0, 504.0, 56.0), "Original text").unwrap();
            let button = Button::new(ui, l.rect(16.0, 72.0, 256.0, 112.0), "Append a star")
                .unwrap()
                .on_click(|| Some(Msg::Rewrite));
            autoclose(ui, || Msg::Quit);
            Demo {
                label,
                _button: button,
            }
        },
    )
}
