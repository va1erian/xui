//! Demonstrates the portable [`Tabs`] container: two pages hold labels created
//! through `tabs.ui()`, and changing tab updates a result label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_tabs
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, Tabs};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Tab(usize),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _general: Label<Msg>,
    _advanced: Label<Msg>,
    _tabs: Tabs<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Tab(index) => {
                let name = if index == 0 { "General" } else { "Advanced" };
                self.result.set_text(&format!("Tab {index}: {name}"));
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Tabs demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let tabs = Tabs::new(ui, l.rect(16.0, 16.0, 504.0, 240.0)).unwrap();
            let general =
                Label::new(tabs.ui(), l.rect(0.0, 0.0, 10.0, 10.0), "General page").unwrap();
            let advanced =
                Label::new(tabs.ui(), l.rect(0.0, 0.0, 10.0, 10.0), "Advanced page").unwrap();
            let (general_id, advanced_id) = (general.id(), advanced.id());
            let tabs = tabs
                .page("General", &[general_id])
                .page("Advanced", &[advanced_id])
                .on_change(|index| Some(Msg::Tab(index)));
            let result =
                Label::new(ui, l.rect(16.0, 256.0, 504.0, 288.0), "Tab 0: General").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _general: general,
                _advanced: advanced,
                _tabs: tabs,
            }
        },
    )
}
