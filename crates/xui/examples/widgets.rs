//! A gallery of the portable `xui-core` widgets that runs on either backend,
//! so you can compare the native controls with the software (tiny-skia) path.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui --features canvas --example widgets
//! ```
//!
//! `XUI_BACKEND=canvas` starts on the software backend instead of the native
//! one; the **Renderer** button starts a fresh process on the other backend
//! (`winit` allows one event loop per process, so it cannot be restarted in
//! place). `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself, and
//! `XUI_AUTOSWITCH_MS` clicks the button for it, both for headless smoke runs.

use std::cell::Cell;
use std::rc::Rc;

use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::widget::{
    Button, CheckBox, ComboBox, Edit, Glyph, GroupBox, HasText, Hyperlink, Label, ListView,
    MaterialStatusBar, Menu, MenuId, MultilineEdit, NumberField, Panel, ProgressBar, RadioGroup,
    Separator, Slider, StatusBar, ToggleButton, Toolbar, TopBar, TopBarId, TreeRow, TreeView,
};
use xui_core::{Dip, Properties, Rect, Theme, Value};

/// Which backend the gallery runs on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Renderer {
    /// The native Win32 backend (where available).
    Native,
    /// The cross-platform software backend (winit + tiny-skia).
    Canvas,
}

impl Renderer {
    fn label(self) -> &'static str {
        match self {
            Renderer::Native => "native",
            Renderer::Canvas => "skia",
        }
    }

    fn other(self) -> Renderer {
        match self {
            Renderer::Native => Renderer::Canvas,
            Renderer::Canvas => Renderer::Native,
        }
    }

    fn env_value(self) -> &'static str {
        match self {
            Renderer::Native => "native",
            Renderer::Canvas => "canvas",
        }
    }
}

#[cfg(all(feature = "win32", windows))]
fn backend_for(renderer: Renderer) -> Rc<dyn Backend> {
    match renderer {
        Renderer::Native => Rc::new(xui_win32::Win32Backend::new()),
        Renderer::Canvas => Rc::new(xui_canvas::WinitBackend::new()),
    }
}

#[cfg(not(all(feature = "win32", windows)))]
fn backend_for(renderer: Renderer) -> Rc<dyn Backend> {
    match renderer {
        Renderer::Native | Renderer::Canvas => Rc::new(xui_canvas::WinitBackend::new()),
    }
}

/// Whether the gallery can switch backends (it needs both to be compiled in).
const CAN_SWITCH: bool = cfg!(all(feature = "win32", windows));

enum Msg {
    Edit(String),
    Number(f64),
    Check(bool),
    Toggle(bool),
    Radio(usize),
    Combo(usize),
    Slide(f64),
    List(usize),
    Tree(usize),
    Tool(usize),
    TopBarNew,
    TopBarSearch,
    TopBarStar(bool),
    TopBarVolume(f64),
    BarMenu(&'static str),
    BarToggle(&'static str, bool),
    ContextMenu,
    Link,
    Theme(usize),
    Switch,
    Autoclose,
}

struct Gallery {
    echo: Label<Msg>,
    status: StatusBar<Msg>,
    bar: ProgressBar<Msg>,
    renderer: Renderer,
    switch: Rc<Cell<Option<Renderer>>>,
    _title: Label<Msg>,
    _edit: Edit<Msg>,
    _number: NumberField<Msg>,
    _check: CheckBox<Msg>,
    _toggle: ToggleButton<Msg>,
    _radios: RadioGroup<Msg>,
    _combo: ComboBox<Msg>,
    _slider: Slider<Msg>,
    _link: Hyperlink<Msg>,
    _list: ListView<Msg>,
    _tree: TreeView<Msg>,
    _group: GroupBox<Msg>,
    _grouped: CheckBox<Msg>,
    _multi: MultilineEdit<Msg>,
    _panel: Panel<Msg>,
    _inside: Label<Msg>,
    _toolbar: Toolbar<Msg>,
    _topbar: TopBar<Msg>,
    _menu: Menu<Msg>,
    _context: Menu<Msg>,
    _material: MaterialStatusBar<Msg>,
    _sep: Separator<Msg>,
    _theme: RadioGroup<Msg>,
    _button: Button<Msg>,
    _switch: Option<Button<Msg>>,
}

impl App for Gallery {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        let note = match msg {
            Msg::Edit(text) => {
                self.echo.set_text(&format!("Edit: {text}"));
                format!("edited: {text}")
            }
            Msg::Number(value) => format!("number: {value}"),
            Msg::Check(checked) => format!("checkbox: {checked}"),
            Msg::Toggle(checked) => format!("toggle: {checked}"),
            Msg::Radio(index) => format!("radio #{index}"),
            Msg::Combo(index) => format!("combo #{index}"),
            Msg::Slide(value) => {
                self.bar.set_value(value as i32);
                format!("slider: {value:.0}")
            }
            Msg::List(index) => format!("list row #{index}"),
            Msg::Tree(index) => format!("tree row #{index}"),
            Msg::Tool(index) => format!("toolbar #{index}"),
            Msg::TopBarNew => "topbar: new".to_string(),
            Msg::TopBarSearch => "topbar: search".to_string(),
            Msg::TopBarStar(checked) => format!("topbar star: {checked}"),
            Msg::TopBarVolume(value) => format!("topbar volume: {value:.0}"),
            Msg::BarMenu(name) => format!("menu: {name}"),
            Msg::BarToggle(name, checked) => format!("menu {name}: {checked}"),
            Msg::ContextMenu => {
                let at = |value: f32| Dip(value).to_px(ui.dpi()).value();
                self._context.show_context(at(400.0), at(60.0));
                "context menu".to_string()
            }
            Msg::Link => "link clicked".to_string(),
            Msg::Theme(choice) => match choice {
                1 => {
                    ui.set_theme(Theme::dark());
                    "theme: dark".to_string()
                }
                _ => {
                    ui.set_theme(Theme::light());
                    "theme: light".to_string()
                }
            },
            Msg::Switch => {
                self.switch.set(Some(self.renderer.other()));
                ui.quit();
                return;
            }
            Msg::Autoclose => {
                ui.quit();
                return;
            }
        };
        self.status.set_text(0, "Ready");
        self.status.set_text(1, &note);
    }
}

fn run(renderer: Renderer, switch: Rc<Cell<Option<Renderer>>>) {
    let title = format!("xui widgets ({})", renderer.label());
    let _ = run_app(
        backend_for(renderer),
        PlatformSpec::new(&title).size(Dip(780.0), Dip(640.0)),
        move |ui| {
            let dpi = ui.dpi();
            let p = move |value: f32| Dip(value).to_px(dpi).value();
            let rect = move |l: f32, t: f32, r: f32, b: f32| Rect::new(p(l), p(t), p(r), p(b));

            let title = Label::new(ui, rect(16.0, 12.0, 380.0, 40.0), "xui widgets").unwrap();
            let edit = Edit::new(ui, rect(16.0, 48.0, 380.0, 76.0), "type here")
                .unwrap()
                .on_change(|text| Some(Msg::Edit(text.to_string())));
            let number = NumberField::new(ui, rect(16.0, 84.0, 380.0, 112.0), 0.0, 100.0, 5.0)
                .unwrap()
                .on_commit(|value| Some(Msg::Number(value)));
            let check = CheckBox::new(ui, rect(16.0, 120.0, 150.0, 148.0), "Enabled")
                .unwrap()
                .on_toggle(|checked| Some(Msg::Check(checked)));
            let toggle = ToggleButton::new(ui, rect(160.0, 120.0, 380.0, 148.0), "Bold")
                .unwrap()
                .on_toggle(|checked| Some(Msg::Toggle(checked)));
            let radios = RadioGroup::new(
                ui,
                rect(16.0, 156.0, 380.0, 240.0),
                &["Small", "Medium", "Large"],
            )
            .unwrap()
            .on_select(|index| Some(Msg::Radio(index)));
            let combo = ComboBox::new(
                ui,
                rect(16.0, 248.0, 380.0, 276.0),
                &["Alpha", "Beta", "Gamma"],
            )
            .unwrap()
            .on_select(|index| Some(Msg::Combo(index)));
            let slider = Slider::new(ui, rect(16.0, 284.0, 380.0, 312.0), 0.0, 100.0)
                .unwrap()
                .on_change(|value| Some(Msg::Slide(value)));
            slider.set_value(40.0);
            let bar = ProgressBar::new(ui, rect(16.0, 320.0, 380.0, 328.0), 100).unwrap();
            bar.set_value(40);
            let link = Hyperlink::new(ui, rect(16.0, 336.0, 380.0, 364.0), "Open docs")
                .unwrap()
                .on_click(|| Some(Msg::Link));
            let button = Button::new(ui, rect(16.0, 458.0, 190.0, 486.0), "Context menu")
                .unwrap()
                .on_click(|| Some(Msg::ContextMenu));
            let theme = RadioGroup::new(ui, rect(16.0, 366.0, 190.0, 420.0), &["Light", "Dark"])
                .unwrap()
                .on_select(|index| Some(Msg::Theme(index)));
            if std::env::var("XUI_GALLERY_THEME").as_deref() == Ok("dark") {
                ui.set_theme(Theme::dark());
                theme.select(1);
            }
            let switch_button = CAN_SWITCH.then(|| {
                let label = format!(
                    "Renderer: {} → {}",
                    renderer.label(),
                    renderer.other().label()
                );
                Button::new(ui, rect(198.0, 458.0, 380.0, 486.0), &label)
                    .unwrap()
                    .on_click(|| Some(Msg::Switch))
            });

            let list = ListView::new(
                ui,
                rect(400.0, 48.0, 764.0, 160.0),
                &["Inbox", "Sent", "Drafts", "Archive"],
            )
            .unwrap()
            .on_select(|index| Some(Msg::List(index)));
            let tree = TreeView::new(
                ui,
                rect(400.0, 168.0, 764.0, 264.0),
                &[
                    TreeRow::new("Inbox", 0).expandable(true).expanded(true),
                    TreeRow::new("Work", 1),
                    TreeRow::new("Home", 1),
                    TreeRow::new("Archive", 0).expandable(true),
                ],
            )
            .unwrap()
            .on_select(|index| Some(Msg::Tree(index)));

            let group = GroupBox::new(ui, rect(400.0, 276.0, 764.0, 356.0), "Group").unwrap();
            let grouped = CheckBox::new(ui, rect(416.0, 312.0, 748.0, 340.0), "Inside the group")
                .unwrap()
                .on_toggle(|checked| Some(Msg::Check(checked)));

            let multi = MultilineEdit::new(ui, rect(400.0, 364.0, 764.0, 428.0), "Notes…").unwrap();

            let panel = Panel::new(ui, rect(400.0, 436.0, 764.0, 504.0)).unwrap();
            let inside =
                Label::new(panel.ui(), rect(12.0, 12.0, 300.0, 40.0), "In a panel").unwrap();

            let toolbar = Toolbar::new(
                ui,
                rect(400.0, 512.0, 764.0, 544.0),
                &["New", "Open", "Save"],
            )
            .unwrap()
            .on_click(|index| Some(Msg::Tool(index)));

            let new_id = TopBarId::new(1);
            let star_id = TopBarId::new(2);
            let volume_id = TopBarId::new(3);
            let search_id = TopBarId::new(4);
            let topbar = TopBar::new(ui, rect(16.0, 424.0, 380.0, 452.0))
                .unwrap()
                .icon(new_id, Glyph::Menu)
                .toggle(star_id, Glyph::Star)
                .label(TopBarId::new(5), "xui")
                .spacer()
                .slider(volume_id, 0.0, 100.0)
                .icon(search_id, Glyph::Search)
                .on_click(move |id| match id {
                    id if id == new_id => Some(Msg::TopBarNew),
                    id if id == search_id => Some(Msg::TopBarSearch),
                    _ => None,
                })
                .on_toggle(move |id, checked| (id == star_id).then_some(Msg::TopBarStar(checked)))
                .on_change(move |id, value| (id == volume_id).then_some(Msg::TopBarVolume(value)));
            let new_id = MenuId::new(1);
            let open_id = MenuId::new(2);
            let save_id = MenuId::new(3);
            let recent_id = MenuId::new(4);
            let undo_id = MenuId::new(10);
            let redo_id = MenuId::new(11);
            let left_id = MenuId::new(12);
            let right_id = MenuId::new(13);
            let menu = Menu::bar(ui, rect(400.0, 12.0, 764.0, 40.0))
                .unwrap()
                .on_select(move |id| {
                    let name = if id == new_id {
                        "new"
                    } else if id == open_id {
                        "open"
                    } else if id == recent_id {
                        "recent"
                    } else if id == undo_id {
                        "undo"
                    } else if id == redo_id {
                        "redo"
                    } else {
                        "command"
                    };
                    Some(Msg::BarMenu(name))
                })
                .on_toggle(move |id, checked| {
                    let name = if id == save_id {
                        "auto save"
                    } else if id == left_id {
                        "left"
                    } else if id == right_id {
                        "right"
                    } else {
                        "toggle"
                    };
                    Some(Msg::BarToggle(name, checked))
                })
                .build(|m| {
                    m.submenu(MenuId::new(0), "&File", |f| {
                        f.item(new_id, "&New");
                        f.item(open_id, "&Open");
                        f.separator();
                        f.check(save_id, "Auto &Save", true);
                        f.separator();
                        f.submenu(recent_id, "&Recent", |r| {
                            r.item(MenuId::new(5), "Report 1");
                            r.item(MenuId::new(6), "Report 2");
                        });
                    });
                    m.submenu(MenuId::new(7), "&Edit", |e| {
                        e.item(undo_id, "&Undo");
                        e.item(redo_id, "&Redo");
                        e.separator();
                        e.radio(left_id, "Align &Left", true);
                        e.radio(right_id, "Align &Right", false);
                    });
                });
            menu.set_enabled(undo_id, false);

            let cut_id = MenuId::new(20);
            let copy_id = MenuId::new(21);
            let paste_id = MenuId::new(22);
            let wrap_id = MenuId::new(23);
            let more_id = MenuId::new(24);
            let context = Menu::context(ui)
                .on_select(move |id| {
                    let name = if id == cut_id {
                        "cut"
                    } else if id == copy_id {
                        "copy"
                    } else if id == paste_id {
                        "paste"
                    } else if id == more_id {
                        "more"
                    } else {
                        "command"
                    };
                    Some(Msg::BarMenu(name))
                })
                .on_toggle(move |id, checked| {
                    (id == wrap_id).then_some(Msg::BarToggle("word wrap", checked))
                })
                .build(|m| {
                    m.item(cut_id, "Cu&t");
                    m.item(copy_id, "&Copy");
                    m.item(paste_id, "&Paste");
                    m.separator();
                    m.check(wrap_id, "&Word wrap", false);
                    m.separator();
                    m.submenu(more_id, "&More", |s| {
                        s.item(MenuId::new(25), "Item &A");
                        s.item(MenuId::new(26), "Item &B");
                    });
                });
            if std::env::var("XUI_GALLERY_MENU").is_ok() {
                let _ = menu.set_property("open", Value::Bool(true));
            }
            if std::env::var("XUI_GALLERY_CONTEXT").is_ok() {
                let at = |value: f32| Dip(value).to_px(ui.dpi()).value();
                context.show_context(at(60.0), at(250.0));
            }

            let sep = Separator::new(ui, rect(16.0, 552.0, 764.0, 554.0)).unwrap();
            let status =
                StatusBar::new(ui, rect(16.0, 560.0, 764.0, 584.0), &["Ready", ""]).unwrap();
            let material =
                MaterialStatusBar::new(ui, rect(16.0, 588.0, 764.0, 610.0), &["Native", "light"])
                    .unwrap();
            let echo = Label::new(ui, rect(16.0, 612.0, 764.0, 634.0), "Edit: ").unwrap();

            // Both smoke hooks fire through one timer mapper, told apart by id.
            let switch_at = Rc::new(Cell::new(None));
            let autoclose_at = Rc::new(Cell::new(None));
            if CAN_SWITCH && let Ok(millis) = std::env::var("XUI_AUTOSWITCH_MS") {
                let _ = millis
                    .parse::<u32>()
                    .map(|ms| switch_at.set(Some(ui.set_timer(ms))));
            }
            if let Ok(millis) = std::env::var("XUI_DEMO_AUTOCLOSE_MS") {
                let _ = millis
                    .parse::<u32>()
                    .map(|ms| autoclose_at.set(Some(ui.set_timer(ms))));
            }
            let switch_at_for_timer = Rc::clone(&switch_at);
            let autoclose_at_for_timer = Rc::clone(&autoclose_at);
            ui.on_timer(move |fired| {
                if switch_at_for_timer.get() == Some(fired) {
                    Some(Msg::Switch)
                } else if autoclose_at_for_timer.get() == Some(fired) {
                    Some(Msg::Autoclose)
                } else {
                    None
                }
            });

            Gallery {
                echo,
                status,
                bar,
                renderer,
                switch,
                _title: title,
                _edit: edit,
                _number: number,
                _check: check,
                _toggle: toggle,
                _radios: radios,
                _combo: combo,
                _slider: slider,
                _link: link,
                _list: list,
                _tree: tree,
                _group: group,
                _grouped: grouped,
                _multi: multi,
                _panel: panel,
                _inside: inside,
                _toolbar: toolbar,
                _topbar: topbar,
                _menu: menu,
                _context: context,
                _material: material,
                _sep: sep,
                _theme: theme,
                _button: button,
                _switch: switch_button,
            }
        },
    );
}

fn main() {
    let renderer = match std::env::var("XUI_BACKEND").as_deref() {
        Ok("canvas") => Renderer::Canvas,
        _ => Renderer::Native,
    };
    let switch = Rc::new(Cell::new(None));
    run(renderer, Rc::clone(&switch));
    if let Some(next) = switch.get() {
        relaunch(next);
    }
}

/// Starts a fresh process on `renderer`. `winit` allows only one `EventLoop`
/// per process on desktop, so the software backend cannot be torn down and
/// rebuilt in place; a child process gives each backend its own loop.
fn relaunch(renderer: Renderer) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .env("XUI_BACKEND", renderer.env_value())
        // The child must not switch again on its own.
        .env_remove("XUI_AUTOSWITCH_MS")
        .spawn();
}
