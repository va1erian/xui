//! Demonstrates the portable [`Split`] container: two panes hold labels created
//! through `split.ui()`, and moving the divider updates a result label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_split
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, Split};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Moved(f32),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _left: Label<Msg>,
    _right: Label<Msg>,
    _split: Split<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Moved(position) => self.result.set_text(&format!("Divider at {position:.0}")),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Split demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let split = Split::row(ui, l.rect(16.0, 16.0, 504.0, 240.0)).unwrap();
            let left = Label::new(split.ui(), l.rect(0.0, 0.0, 10.0, 10.0), "Left pane").unwrap();
            let right = Label::new(split.ui(), l.rect(0.0, 0.0, 10.0, 10.0), "Right pane").unwrap();
            let (left_id, right_id) = (left.id(), right.id());
            split.pane_a(&[left_id]);
            split.pane_b(&[right_id]);
            split.set_min(Dip(60.0), Dip(60.0));
            split.set_position(Dip(244.0));
            let split = split.on_moved(|position| Some(Msg::Moved(position.value())));
            let result =
                Label::new(ui, l.rect(16.0, 256.0, 504.0, 288.0), "Divider at 244").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _left: left,
                _right: right,
                _split: split,
            }
        },
    )
}
