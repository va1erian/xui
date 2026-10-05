//! Demonstrates the portable [`Icon`] set: one button per icon, and a label
//! names the pressed one. `draw_icon` draws the same shapes on a `Canvas`.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_icon
//! ```

use xui::prelude::*;
use xui::widget::Icon;

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

#[derive(Clone)]
enum Msg {
    Pick(Icon),
}

#[derive(Default)]
struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Pick(icon) => self.result.get().set_text(&format!("Icon: {}", name(icon))),
        }
    }
}

fn main() -> Result<()> {
    xui::app("Icon demo").size(520, 360).run(|ui| {
        let demo = Demo::default();
        let buttons: Vec<_> = ICONS
            .into_iter()
            .map(|icon| button("").icon(icon).on_click(Msg::Pick(icon)).size(54, 56))
            .collect();
        ui.root(column().padding(16).gap(16).children((
            row().gap(8).children(buttons),
            label("No icon pressed").bind(&demo.result),
        )))?;
        Ok(demo)
    })
}
