//! Demonstrates the portable [`Icon`] set: one button per icon, and a label
//! names the pressed one. `draw_icon` draws the same shapes on a `Canvas`.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_icon
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{Button, HasText, Icon, Label};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

const ICONS: [Icon; 8] = [
    Icon::Plus,
    Icon::Minus,
    Icon::Close,
    Icon::Check,
    Icon::ChevronDown,
    Icon::ChevronUp,
    Icon::Search,
    Icon::More,
];

fn name(icon: Icon) -> &'static str {
    match icon {
        Icon::Plus => "Plus",
        Icon::Minus => "Minus",
        Icon::Close => "Close",
        Icon::Check => "Check",
        Icon::ChevronDown => "ChevronDown",
        Icon::ChevronUp => "ChevronUp",
        Icon::Search => "Search",
        Icon::More => "More",
    }
}

enum Msg {
    Pick(Icon),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _buttons: Vec<Button<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Pick(icon) => self.result.set_text(&format!("Icon: {}", name(icon))),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Icon demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let mut buttons = Vec::new();
            for (index, icon) in ICONS.into_iter().enumerate() {
                let left = 16.0 + index as f32 * 62.0;
                let button = Button::new(ui, l.rect(left, 16.0, left + 54.0, 72.0), "")
                    .unwrap()
                    .icon(icon)
                    .on_click(move || Some(Msg::Pick(icon)));
                buttons.push(button);
            }
            let result =
                Label::new(ui, l.rect(16.0, 88.0, 504.0, 120.0), "No icon pressed").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _buttons: buttons,
            }
        },
    )
}
