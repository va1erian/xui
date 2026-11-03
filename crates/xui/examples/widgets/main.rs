//! A gallery of the portable `xui-core` widgets on the software (tiny-skia)
//! backend.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example widgets
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself, for headless smoke runs.
//! `XUI_SNAPSHOT=<dir>` saves a light and a dark screenshot instead of
//! opening a window. `XUI_GALLERY_THEME=dark`, `XUI_GALLERY_MENU`,
//! `XUI_GALLERY_CONTEXT`, `XUI_GALLERY_DIALOG` and `XUI_GALLERY_TOOLTIP`
//! start dark or show the menu, context menu, dialog or a tooltip up front.

mod menus;
mod page;

use xui::prelude::*;
use xui_core::widget::{Dialog, DialogAction, Tooltip};
use xui_core::{Color, Properties, Value};

#[derive(Clone)]
enum Msg {
    Edit(String),
    Number(f64),
    Check(bool),
    Toggle(bool),
    Radio(usize),
    Combo(usize),
    Slide(f64),
    Scroll(i32),
    List(usize),
    Icon(usize),
    Tree(usize),
    Tool(usize),
    TopBarNew,
    TopBarSearch,
    TopBarPlay,
    TopBarStar(bool),
    TopBarVolume(f64),
    BarMenu(&'static str),
    BarToggle(&'static str, bool),
    ContextMenu,
    Link,
    Theme(usize),
    Tab(usize),
    SplitPane(f32),
    Swatch(Color),
    Panel(Color),
    DialogOpen,
    DialogAction(DialogAction),
}

struct Gallery {
    parts: page::Parts,
    context: Menu<Msg>,
    dialog: Dialog<Msg>,
    _tips: [Tooltip<Msg>; 2],
}

impl App for Gallery {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let note = match msg {
            Msg::Edit(text) => {
                self.parts.echo.get().set_text(&format!("Edit: {text}"));
                format!("edited: {text}")
            }
            Msg::Number(value) => format!("number: {value}"),
            Msg::Check(checked) => format!("checkbox: {checked}"),
            Msg::Toggle(checked) => format!("toggle: {checked}"),
            Msg::Radio(index) => format!("radio #{index}"),
            Msg::Combo(index) => format!("combo #{index}"),
            Msg::Slide(value) => {
                self.parts.bar.get().set_value(value as i32);
                format!("slider: {value:.0}")
            }
            Msg::Scroll(offset) => format!("scroll: {offset}px"),
            Msg::List(index) => format!("list row #{index}"),
            Msg::Icon(index) => format!("icon tile #{index}"),
            Msg::Tree(index) => format!("tree row #{index}"),
            Msg::Tool(index) => format!("toolbar #{index}"),
            Msg::TopBarNew => "topbar: new".to_string(),
            Msg::TopBarSearch => "topbar: search".to_string(),
            Msg::TopBarPlay => "topbar: play".to_string(),
            Msg::TopBarStar(checked) => format!("topbar star: {checked}"),
            Msg::TopBarVolume(value) => format!("topbar volume: {value:.0}"),
            Msg::BarMenu(name) => format!("menu: {name}"),
            Msg::BarToggle(name, checked) => format!("menu {name}: {checked}"),
            Msg::ContextMenu => {
                show_context(&self.context, ui, 400.0, 60.0);
                "context menu".to_string()
            }
            Msg::Link => "link clicked".to_string(),
            Msg::Tab(index) => format!("tab #{index}"),
            Msg::SplitPane(position) => format!("split: {position:.0}"),
            Msg::Swatch(color) => format!("accent #{:02X}{:02X}{:02X}", color.r, color.g, color.b),
            Msg::Panel(color) => format!("panel #{:02X}{:02X}{:02X}", color.r, color.g, color.b),
            Msg::DialogOpen => {
                self.dialog.open();
                "dialog: open".to_string()
            }
            Msg::DialogAction(action) => match action {
                DialogAction::Accept(text) if text.is_empty() => "dialog: accepted".to_string(),
                DialogAction::Accept(text) => format!("dialog: accepted {text}"),
                DialogAction::Cancel => "dialog: cancelled".to_string(),
            },
            Msg::Theme(choice) => {
                let dark = choice == 1;
                ui.set_theme(if dark { Theme::dark() } else { Theme::light() });
                format!("theme: {}", if dark { "dark" } else { "light" })
            }
        };
        let status = self.parts.status.get();
        status.set_text(0, "Ready");
        status.set_text(1, &note);
    }
}

/// Pops `menu` up at (`x`, `y`) design units in the window.
fn show_context(menu: &Menu<Msg>, ui: &Ui<Msg>, x: f32, y: f32) {
    let at = |value: f32| Dip(value).to_px(ui.dpi()).value();
    menu.show_context(at(x), at(y));
}

fn main() -> Result<()> {
    xui::app("xui widgets").size(1296, 664).run(move |ui| {
        let gallery_env = |name: &str| std::env::var(name).is_ok();
        let dark = std::env::var("XUI_GALLERY_THEME").as_deref() == Ok("dark");
        let parts = page::Parts::default();
        ui.root(page::page(&parts, dark))?;
        if dark {
            ui.set_theme(Theme::dark());
        }

        let menu = parts.menu.get();
        menu.set_enabled(menus::UNDO, false);
        if gallery_env("XUI_GALLERY_MENU") {
            menu.set_property("open", Value::Bool(true));
        }
        let context = menus::context(ui);
        if gallery_env("XUI_GALLERY_CONTEXT") {
            show_context(&context, ui, 60.0, 250.0);
        }
        let dialog = Dialog::confirm(ui, "Save changes?", "Your edits will be lost otherwise.")?
            .accept_label("Save")
            .on_action(|action| Some(Msg::DialogAction(action)));
        // A tooltip observes the widget's hover without taking over its own
        // click handling; the bar gets one too.
        let tips = [
            Tooltip::attach(ui, parts.link.get().id(), "Open the documentation")?,
            Tooltip::attach(ui, parts.bar.get().id(), "Scan progress")?,
        ];

        // A screenshot cannot press a button: open the dialog from a timer
        // once the window is live.
        let dialog_at = gallery_env("XUI_GALLERY_DIALOG").then(|| ui.set_timer(500));
        ui.on_timer(move |fired| {
            if dialog_at == Some(fired) {
                Some(Msg::DialogOpen)
            } else {
                None
            }
        });

        // A screenshot run cannot hover, so show one tip outright. Show it
        // last, after every sibling exists, so `raise` puts it on top.
        if gallery_env("XUI_GALLERY_TOOLTIP") {
            tips[0].show();
        }

        Ok(Gallery {
            parts,
            context,
            dialog,
            _tips: tips,
        })
    })
}
