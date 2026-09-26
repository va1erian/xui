//! Demonstrates the portable [`TopBar`]: icons, a toggle, a label, a spacer and
//! a slider report through one label.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_topbar
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Glyph, HasText, Label, TopBar, TopBarId};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    New,
    Star(bool),
    Volume(f64),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _bar: TopBar<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::New => self.result.set_text("New pressed"),
            Msg::Star(checked) => {
                self.result
                    .set_text(if checked { "Starred" } else { "Unstarred" });
            }
            Msg::Volume(value) => self.result.set_text(&format!("Volume: {value:.0}")),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("TopBar demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let new_id = TopBarId::new(1);
            let star_id = TopBarId::new(2);
            let volume_id = TopBarId::new(3);
            let bar = TopBar::new(ui, l.rect(16.0, 16.0, 504.0, 72.0))
                .unwrap()
                .icon(new_id, Glyph::Text("+"))
                .toggle(star_id, Glyph::Star)
                .label(TopBarId::new(4), "Volume")
                .spacer()
                .slider(volume_id, 0.0, 100.0)
                .on_click(move |id| if id == new_id { Some(Msg::New) } else { None })
                .on_toggle(move |id, checked| {
                    if id == star_id {
                        Some(Msg::Star(checked))
                    } else {
                        None
                    }
                })
                .on_change(move |id, value| {
                    if id == volume_id {
                        Some(Msg::Volume(value))
                    } else {
                        None
                    }
                });
            let result = Label::new(ui, l.rect(16.0, 88.0, 504.0, 120.0), "Top bar ready").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo { result, _bar: bar }
        },
    )
}
