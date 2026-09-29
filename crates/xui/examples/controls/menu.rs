//! Demonstrates the portable [`Menu`]: a menu bar with a submenu and a check
//! item, plus a context menu shown from a button. A label reports the command.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example control_menu
//! ```

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::icon::Lucide;
use xui_core::widget::{Button, HasText, Label, Menu, MenuId};

#[path = "support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Command(&'static str),
    Toggled(&'static str, bool),
    Context,
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _menu: Menu<Msg>,
    _context: Menu<Msg>,
    _button: Button<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Command(name) => self.result.set_text(&format!("Command: {name}")),
            Msg::Toggled(name, checked) => {
                self.result.set_text(&format!("{name}: {checked}"));
            }
            Msg::Context => {
                let dpi = ui.dpi();
                let at = |value: f32| Dip(value).to_px(dpi).value();
                self._context.show_context(at(96.0), at(160.0));
                self.result.set_text("Context menu opened");
            }
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Menu demo").size(Dip(520.0), Dip(360.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let new_id = MenuId::new(1);
            let save_id = MenuId::new(2);
            let cut_id = MenuId::new(10);
            let copy_id = MenuId::new(11);
            let menu = Menu::bar(ui, l.rect(16.0, 16.0, 504.0, 48.0))
                .unwrap()
                .on_select(move |id| Some(Msg::Command(if id == new_id { "New" } else { "Help" })))
                .on_toggle(move |id, checked| {
                    if id == save_id {
                        Some(Msg::Toggled("Auto save", checked))
                    } else {
                        None
                    }
                })
                .build(|m| {
                    m.submenu(MenuId::new(0), "&File", |file| {
                        file.item(new_id, "&New").icon(Lucide::FilePlus);
                        file.separator();
                        file.check(save_id, "Auto &save", true);
                    });
                    m.item(MenuId::new(3), "&Help");
                });
            let context = Menu::context(ui)
                .on_select(move |id| {
                    Some(Msg::Command(if id == cut_id {
                        "Cut"
                    } else if id == copy_id {
                        "Copy"
                    } else {
                        "Paste"
                    }))
                })
                .build(|m| {
                    m.item(cut_id, "Cu&t").icon(Lucide::Scissors);
                    m.item(copy_id, "&Copy").icon(Lucide::Copy);
                    m.item(MenuId::new(12), "&Paste")
                        .icon(Lucide::ClipboardPaste);
                });
            let button = Button::new(ui, l.rect(16.0, 64.0, 256.0, 104.0), "Show context menu")
                .unwrap()
                .on_click(|| Some(Msg::Context));
            let result =
                Label::new(ui, l.rect(16.0, 120.0, 504.0, 152.0), "No command yet").unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _menu: menu,
                _context: context,
                _button: button,
            }
        },
    )
}
