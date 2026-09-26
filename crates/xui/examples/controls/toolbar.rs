//! Demonstrates the portable [`Toolbar`]: clicking a tool reports its index and
//! a label names it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_toolbar
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, Toolbar};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const TOOLS: [&str; 3] = ["New", "Open", "Save"];

enum Msg {
    Click(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _toolbar: Toolbar<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Click(index) => {
                let name = TOOLS.get(index).copied().unwrap_or("?");
                self.result.set_text(&format!("Tool {index}: {name}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Toolbar demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "No tool yet").unwrap();
            let toolbar = Toolbar::new(ui, l.rect(16.0, 64.0, 504.0, 112.0), &TOOLS)
                .unwrap()
                .on_click(|index| Some(Msg::Click(index)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _toolbar: toolbar,
            }
        },
    )
}
