//! Demonstrates the portable [`Hyperlink`]: clicking the link raises a message
//! and a label reports it.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_hyperlink
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Hyperlink, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Open,
    Quit,
}

struct Demo {
    result: Label<Msg>,
    clicks: u32,
    _link: Hyperlink<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Open => {
                self.clicks += 1;
                self.result
                    .set_text(&format!("Link opened {} times", self.clicks));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Hyperlink demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let result = Label::new(ui, l.rect(16.0, 16.0, 504.0, 48.0), "Not opened yet").unwrap();
            let link = Hyperlink::new(ui, l.rect(16.0, 64.0, 320.0, 96.0), "Open the docs")
                .unwrap()
                .on_click(|| Some(Msg::Open));
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                clicks: 0,
                _link: link,
            }
        },
    )
}
