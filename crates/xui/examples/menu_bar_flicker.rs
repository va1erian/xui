//! Reproducer for the portable [`Menu`] bar's hover flicker.
//!
//! With a menu open, moving the pointer along the bar hides and re-shows the
//! drop-down on every mouse-move event, so it flickers continuously while the
//! pointer is over the bar. The menu opens up front here so the flicker shows
//! without a click.
//!
//! Run with:
//!
//! ```text
//! $env:XUI_DEMO_AUTOCLOSE_MS = "4000"
//! cargo run -p xui --features canvas --example menu_bar_flicker
//! ```
//!
//! Then slide the pointer a few pixels along the `File`/`Edit`/`View` titles:
//! the open drop-down vanishes and reappears for each move.

use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::PlatformSpec;
use xui_core::widget::{HasText, Label, Menu, MenuId};
use xui_core::{Properties, Value};

#[path = "controls/support.rs"]
mod support;
use support::{Layout, autoclose, backend};

enum Msg {
    Command(&'static str),
    Quit,
}

struct Demo {
    result: Label<Msg>,
    _menu: Menu<Msg>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Command(name) => self.result.set_text(&format!("Command: {name}")),
            Msg::Quit => ui.quit(),
        }
    }
}

fn main() -> xui_core::backend::Result<()> {
    run_app(
        backend(),
        PlatformSpec::new("Menu bar flicker repro").size(Dip(520.0), Dip(240.0)),
        |ui| {
            let l = Layout::new(ui.dpi());
            let new_id = MenuId::new(1);
            let open_id = MenuId::new(2);
            let undo_id = MenuId::new(10);
            let zoom_id = MenuId::new(20);
            let menu = Menu::bar(ui, l.rect(16.0, 16.0, 504.0, 48.0))
                .unwrap()
                .on_select(move |id| {
                    Some(Msg::Command(match id {
                        id if id == new_id => "New",
                        id if id == open_id => "Open",
                        id if id == undo_id => "Undo",
                        id if id == zoom_id => "Zoom",
                        _ => "Other",
                    }))
                })
                .build(|m| {
                    m.submenu(MenuId::new(0), "&File", |f| {
                        f.item(new_id, "&New");
                        f.item(open_id, "&Open");
                    });
                    m.submenu(MenuId::new(3), "&Edit", |e| {
                        e.item(undo_id, "&Undo");
                    });
                    m.submenu(MenuId::new(4), "&View", |v| {
                        v.item(zoom_id, "&Zoom");
                    });
                });
            // Open `File` up front: then moving the pointer along the bar
            // flickers the drop-down without a click.
            let _ = menu.set_property("open", Value::Bool(true));
            let result = Label::new(
                ui,
                l.rect(16.0, 72.0, 504.0, 200.0),
                "Move the pointer along the bar with a menu open: the popup flickers.",
            )
            .unwrap();
            autoclose(ui, || Msg::Quit);
            Demo {
                result,
                _menu: menu,
            }
        },
    )
}
