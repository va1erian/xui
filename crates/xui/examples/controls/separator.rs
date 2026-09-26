//! Demonstrates the portable [`Separator`] in both orientations; a button
//! toggles its selected outline and a label reports the state.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_separator
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Label, Separator};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Toggle,
    Quit,
}

struct Demo {
    result: Label<Msg>,
    selected: bool,
    _above: Label<Msg>,
    _left: Label<Msg>,
    _right: Label<Msg>,
    _horizontal: Separator<Msg>,
    _vertical: Separator<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Toggle => {
                self.selected = !self.selected;
                self._horizontal.set_selected(self.selected);
                self._vertical.set_selected(self.selected);
                self.result.set_text(if self.selected {
                    "Separators selected"
                } else {
                    "Separators idle"
                });
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Separator demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result =
                Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Separators idle").unwrap();
            let above = Label::new(ui, l.rect(16.0, 72.0, 320.0, 96.0), "Above").unwrap();
            let horizontal = Separator::new(ui, l.rect(16.0, 104.0, 504.0, 106.0)).unwrap();
            let vertical = Separator::vertical(ui, l.rect(360.0, 120.0, 362.0, 240.0)).unwrap();
            let left = Label::new(ui, l.rect(16.0, 128.0, 340.0, 152.0), "Left").unwrap();
            let right = Label::new(ui, l.rect(380.0, 128.0, 504.0, 152.0), "Right").unwrap();
            let button = Button::new(ui, l.rect(16.0, 200.0, 256.0, 240.0), "Toggle selected")
                .unwrap()
                .on_click(|| Some(Msg::Toggle));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                selected: false,
                _above: above,
                _left: left,
                _right: right,
                _horizontal: horizontal,
                _vertical: vertical,
                _button: button,
            }
        },
    )
}
