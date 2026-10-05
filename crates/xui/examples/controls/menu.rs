//! Demonstrates the portable [`Menu`]: a menu bar with a submenu and a check
//! item, plus a context menu shown from a button. A label reports the command.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_menu
//! ```

use xui::icon::Lucide;
use xui::prelude::*;

const NEW: MenuId = MenuId::new(1);
const AUTO_SAVE: MenuId = MenuId::new(2);
const CUT: MenuId = MenuId::new(10);
const COPY: MenuId = MenuId::new(11);

#[derive(Clone)]
enum Msg {
    Command(&'static str),
    Toggled(&'static str, bool),
    Context,
}

struct Demo {
    result: Handle<Label<Msg>>,
    context: Menu<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let result = self.result.get();
        match msg {
            Msg::Command(name) => result.set_text(&format!("Command: {name}")),
            Msg::Toggled(name, checked) => result.set_text(&format!("{name}: {checked}")),
            Msg::Context => {
                let dpi = ui.dpi();
                let at = |value: f32| Dip(value).to_px(dpi).value();
                self.context.show_context(at(96.0), at(160.0));
                result.set_text("Context menu opened");
            }
        }
    }
}

fn main() -> Result<()> {
    xui::app("Menu demo").size(520, 360).run(|ui| {
        let result = Handle::new();
        let menu = menu_bar(|m| {
            m.submenu(MenuId::new(0), "&File", |file| {
                file.item(NEW, "&New").icon(Lucide::FilePlus);
                file.separator();
                file.check(AUTO_SAVE, "Auto &save", true);
            });
            m.item(MenuId::new(3), "&Help");
        })
        .on_select(|id| Msg::Command(if id == NEW { "New" } else { "Help" }))
        .then(|menu| {
            menu.on_toggle(|id, checked| {
                (id == AUTO_SAVE).then_some(Msg::Toggled("Auto save", checked))
            })
        });
        ui.root(
            column().padding(16).gap(16).children((
                menu,
                button("Show context menu")
                    .on_click(Msg::Context)
                    .width(240)
                    .align(Align::Start),
                label("No command yet").bind(&result),
            )),
        )?;
        let context = Menu::context(ui)
            .on_select(|id| {
                Some(Msg::Command(if id == CUT {
                    "Cut"
                } else if id == COPY {
                    "Copy"
                } else {
                    "Paste"
                }))
            })
            .build(|m| {
                m.item(CUT, "Cu&t").icon(Lucide::Scissors);
                m.item(COPY, "&Copy").icon(Lucide::Copy);
                m.item(MenuId::new(12), "&Paste")
                    .icon(Lucide::ClipboardPaste);
            });
        Ok(Demo { result, context })
    })
}
