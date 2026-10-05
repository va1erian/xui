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
//! `XUI_SNAPSHOT=<dir>` saves a light and a dark screenshot instead.

use xui::prelude::*;
use xui_core::{Properties, Value};

const NEW: MenuId = MenuId::new(1);
const OPEN: MenuId = MenuId::new(2);
const UNDO: MenuId = MenuId::new(10);
const ZOOM: MenuId = MenuId::new(20);

#[derive(Clone)]
enum Msg {
    Command(&'static str),
}

struct Demo {
    result: Handle<Label<Msg>>,
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Command(name) => self.result.get().set_text(&format!("Command: {name}")),
        }
    }
}

fn main() -> Result<()> {
    xui::app("Menu bar flicker repro").size(520, 240).run(|ui| {
        let demo = Demo {
            result: Handle::new(),
        };
        let menu: Handle<Menu<Msg>> = Handle::new();
        ui.root(
            column().padding(16).gap(24).children((
                menu_bar(|m| {
                    m.submenu(MenuId::new(0), "&File", |f| {
                        f.item(NEW, "&New");
                        f.item(OPEN, "&Open");
                    });
                    m.submenu(MenuId::new(3), "&Edit", |e| {
                        e.item(UNDO, "&Undo");
                    });
                    m.submenu(MenuId::new(4), "&View", |v| {
                        v.item(ZOOM, "&Zoom");
                    });
                })
                .on_select(|id| {
                    Msg::Command(match id {
                        NEW => "New",
                        OPEN => "Open",
                        UNDO => "Undo",
                        ZOOM => "Zoom",
                        _ => "Other",
                    })
                })
                .bind(&menu)
                .height(32),
                label("Move the pointer along the bar with a menu open: the popup flickers.")
                    .bind(&demo.result),
            )),
        )?;
        // Open `File` up front: then moving the pointer along the bar
        // flickers the drop-down without a click.
        menu.get().set_property("open", Value::Bool(true));
        Ok(demo)
    })
}
