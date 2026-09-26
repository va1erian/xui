//! Demonstrates the portable [`FlowText`]: a line of normal, link and weak runs
//! that wraps; clicking the link updates a label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_flowtext
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{FlowText, HasText, Label, Run};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Link,
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _flow: FlowText<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Link => self.result.set_text("Documentation link clicked"),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("FlowText demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let flow = FlowText::new(ui, l.rect(16.0, 16.0, 504.0, 80.0))
                .unwrap()
                .run(Run::normal("Read the "))
                .run(Run::link("documentation").on_click(|| Some(Msg::Link)))
                .separator(" · ")
                .run(Run::weak("version 0.1"));
            let result =
                Label::new(ui, l.rect(16.0, 96.0, 504.0, 128.0), "No link clicked").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _flow: flow,
            }
        },
    )
}
