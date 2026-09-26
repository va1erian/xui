//! Demonstrates the portable [`GroupBox`] frame with a sibling [`CheckBox`]:
//! the checkbox reports its state to a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_groupbox
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{CheckBox, GroupBox, HasText, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Toggle(bool),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _group: GroupBox<Msg>,
    _check: CheckBox<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle(checked) => {
                self.result
                    .set_text(if checked { "Option on" } else { "Option off" });
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("GroupBox demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Option off").unwrap();
            let group = GroupBox::new(ui, l.rect(16.0, 64.0, 504.0, 168.0), "Settings").unwrap();
            let check = CheckBox::new(ui, l.rect(32.0, 104.0, 488.0, 144.0), "Enable option")
                .unwrap()
                .on_toggle(|checked| Some(Msg::Toggle(checked)));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _group: group,
                _check: check,
            }
        },
    )
}
