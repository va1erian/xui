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
use xui_core::icon::Lucide;
use xui_core::image::Image;
use xui_core::widget::{
    Button, CheckBox, ColorPicker, ComboBox, Dialog, DialogAction, Edit, Glyph, GroupBox, HasText,
    Hyperlink, IconView, Label, ListView, MaterialStatusBar, Menu, MenuId, MultilineEdit,
    NumberField, Panel, ProgressBar, RadioGroup, ScrollView, Separator, Slider, Split, StatusBar,
    Tabs, ToggleButton, Toolbar, Tooltip, TopBar, TopBarId, TreeRow, TreeView,
};
use xui_core::{Color, Dip, Properties, Rect, Theme, Value};

/// A 16x16 two-tone artwork icon built in memory, so the gallery exercises an
/// `Image` row icon (and the backend's decoded-image cache) beside the glyphs.
fn art_image() -> Image {
    let mut pixels = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let (r, g, b) = if (x / 4 + y / 4) % 2 == 0 {
                (0x7A, 0xC8, 0xE8)
            } else {
                (0x2A, 0x62, 0xA8)
            };
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Image::from_rgba(16, 16, pixels).expect("16x16 RGBA")
}

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

#[cfg(all(feature = "d2d", windows))]
fn backend_for(renderer: Renderer) -> Rc<dyn Backend> {
    if let Some(backend) = support::offscreen_backend() {
        return backend;
    }
    match renderer {
        Renderer::Native => Rc::new(xui_win32::Win32Backend::new()),
        Renderer::Canvas => Rc::new(xui_canvas::WinitBackend::new()),
    }
}

#[cfg(not(all(feature = "d2d", windows)))]
fn backend_for(renderer: Renderer) -> Rc<dyn Backend> {
    if let Some(backend) = support::offscreen_backend() {
        return backend;
    }
    match renderer {
        Renderer::Native | Renderer::Canvas => Rc::new(xui_canvas::WinitBackend::new()),
    }
}

/// Whether the gallery can switch backends (it needs both to be compiled in).
const CAN_SWITCH: bool = cfg!(all(feature = "d2d", windows));

// Only the headless hooks are used here; the DIP converter is for the demos in
// `controls/`.
#[allow(dead_code)]
#[path = "controls/support.rs"]
mod support;

enum Msg {
    Edit(String),
    Number(f64),
    Check(bool),
    Toggle(bool),
    Radio(usize),
    Combo(usize),
    Slide(f64),
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
    Scroll(i32),
    Tab(usize),
    SplitPane(f32),
    Swatch(Color),
    DialogOpen,
    DialogAction(DialogAction),
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
    _icons: IconView<Msg>,
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
    _tip_link: Tooltip<Msg>,
    _tip_bar: Tooltip<Msg>,
    _scroll: ScrollView<Msg>,
    _tabs: Tabs<Msg>,
    _split: Split<Msg>,
    _swatches: ColorPicker<Msg>,
    _dialog: Dialog<Msg>,
    _dialog_button: Button<Msg>,
    _content: Vec<Label<Msg>>,
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
                let at = |value: f32| Dip(value).to_px(ui.dpi()).value();
                self._context.show_context(at(400.0), at(60.0));
                "context menu".to_string()
            }
            Msg::Link => "link clicked".to_string(),
            Msg::Scroll(offset) => format!("scroll: {offset}px"),
            Msg::Tab(index) => format!("tab #{index}"),
            Msg::SplitPane(position) => format!("split: {position:.0}"),
            Msg::Swatch(color) => format!("accent #{:02X}{:02X}{:02X}", color.r, color.g, color.b),
            Msg::DialogOpen => {
                self._dialog.open();
                "dialog: open".to_string()
            }
            Msg::DialogAction(action) => match action {
                DialogAction::Accept(text) if text.is_empty() => "dialog: accepted".to_string(),
                DialogAction::Accept(text) => format!("dialog: accepted {text}"),
                DialogAction::Cancel => "dialog: cancelled".to_string(),
            },
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
        PlatformSpec::new(&title).size(Dip(1760.0), Dip(940.0)),
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
            // A tooltip observes the widget's hover without taking over its
            // own click handling; the bar gets one too.
            let tip_link = Tooltip::attach(ui, link.id(), "Open the documentation").unwrap();
            let tip_bar = Tooltip::attach(ui, bar.id(), "Scan progress").unwrap();
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
            let icons = IconView::new(
                ui,
                rect(1040.0, 176.0, 1280.0, 452.0),
                &["Documents", "Pictures", "Music", "Videos"],
            )
            .unwrap()
            .on_select(|index| Some(Msg::Icon(index)));
            let tree = TreeView::new(
                ui,
                rect(400.0, 168.0, 764.0, 264.0),
                &[
                    TreeRow::new("Inbox", 0)
                        .expandable(true)
                        .expanded(true)
                        .icon(Glyph::Folder),
                    TreeRow::new("Work", 1).icon(Glyph::Tag),
                    TreeRow::new("Home", 1).icon(art_image()),
                    TreeRow::new("Archive", 0)
                        .expandable(true)
                        .icon(Glyph::History),
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

            let toolbar = Toolbar::empty(ui, rect(400.0, 512.0, 764.0, 544.0))
                .unwrap()
                .item(Lucide::FilePlus, "New")
                .item(Lucide::FolderOpen, "Open")
                .separator()
                .item_with_text(Lucide::Save, "Save", "Save")
                .on_click(|index| Some(Msg::Tool(index)));

            let new_id = TopBarId::new(1);
            let star_id = TopBarId::new(2);
            let volume_id = TopBarId::new(3);
            let search_id = TopBarId::new(4);
            let play_id = TopBarId::new(6);
            let topbar = TopBar::new(ui, rect(16.0, 424.0, 380.0, 452.0))
                .unwrap()
                .icon(new_id, Glyph::Menu)
                .icon(play_id, Glyph::Play)
                .toggle(star_id, Glyph::Star)
                .label(TopBarId::new(5), "xui")
                .slider(volume_id, 0.0, 100.0)
                .expand(volume_id)
                .icon(search_id, Glyph::Search)
                .on_click(move |id| match id {
                    id if id == new_id => Some(Msg::TopBarNew),
                    id if id == search_id => Some(Msg::TopBarSearch),
                    id if id == play_id => Some(Msg::TopBarPlay),
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

            // The containers' content widgets are held here: a container moves
            // its children but does not own them, so the app keeps them alive.
            let mut content: Vec<Label<Msg>> = Vec::new();

            let scroll = ScrollView::new(ui, rect(788.0, 48.0, 1024.0, 232.0)).unwrap();
            for row in 1..=6 {
                let label = Label::new(
                    scroll.ui(),
                    Rect::new(0, 0, 10, 10),
                    &format!("Scroll row {row}"),
                )
                .unwrap();
                scroll.add(label.id(), Dip(40.0));
                content.push(label);
            }
            let scroll = scroll.on_scroll(|offset| Some(Msg::Scroll(offset.value())));

            let tabs = Tabs::new(ui, rect(788.0, 244.0, 1024.0, 420.0)).unwrap();
            let general =
                Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "General settings").unwrap();
            let advanced =
                Label::new(tabs.ui(), Rect::new(0, 0, 10, 10), "Advanced settings").unwrap();
            let (general_id, advanced_id) = (general.id(), advanced.id());
            content.push(general);
            content.push(advanced);
            let tabs = tabs
                .page("General", &[general_id])
                .page("Advanced", &[advanced_id])
                .on_change(|index| Some(Msg::Tab(index)));

            let split = Split::row(ui, rect(788.0, 432.0, 1024.0, 600.0)).unwrap();
            let left = Label::new(split.ui(), Rect::new(0, 0, 10, 10), "Left pane").unwrap();
            let right = Label::new(split.ui(), Rect::new(0, 0, 10, 10), "Right pane").unwrap();
            let (left_id, right_id) = (left.id(), right.id());
            content.push(left);
            content.push(right);
            split.pane_a(&[left_id]);
            split.pane_b(&[right_id]);
            split.set_min(Dip(60.0), Dip(60.0));
            split.set_position(Dip(110.0));
            let split = split.on_moved(|position| Some(Msg::SplitPane(position.value())));

            let palette = [
                Color::hex(0x00_78_D4),
                Color::hex(0x00_B2_94),
                Color::hex(0x10_7C_10),
                Color::hex(0xFF_B9_00),
                Color::hex(0xFF_8C_00),
                Color::hex(0xE8_11_23),
                Color::hex(0x87_64_B8),
                Color::hex(0x4C_4A_48),
            ];
            let swatches = ColorPicker::new(ui, rect(1040.0, 48.0, 1280.0, 120.0), &palette)
                .unwrap()
                .columns(4)
                .selected(palette[0])
                .on_select(|color| Some(Msg::Swatch(color)));
            let dialog_button = Button::new(ui, rect(1040.0, 132.0, 1280.0, 160.0), "Show dialog")
                .unwrap()
                .on_click(|| Some(Msg::DialogOpen));
            let dialog = Dialog::confirm(ui, "Save changes?", "Your edits will be lost otherwise.")
                .unwrap()
                .accept_label("Save")
                .on_action(|action| Some(Msg::DialogAction(action)));

            let sep = Separator::new(ui, rect(16.0, 552.0, 764.0, 554.0)).unwrap();
            let status =
                StatusBar::new(ui, rect(16.0, 560.0, 764.0, 584.0), &["Ready", ""]).unwrap();
            let material =
                MaterialStatusBar::new(ui, rect(16.0, 588.0, 764.0, 610.0), &["Native", "light"])
                    .unwrap();
            let echo = Label::new(ui, rect(16.0, 612.0, 764.0, 634.0), "Edit: ").unwrap();

            support::snapshot_hook(ui);

            // Both smoke hooks fire through one timer mapper, told apart by id.
            let switch_at = Rc::new(Cell::new(None));
            let autoclose_at = Rc::new(Cell::new(None));
            let dialog_at = Rc::new(Cell::new(None));
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
            // A screenshot cannot press a button: open the dialog from a timer
            // once the window is live.
            if std::env::var("XUI_GALLERY_DIALOG").is_ok() {
                dialog_at.set(Some(ui.set_timer(500)));
            }
            let switch_at_for_timer = Rc::clone(&switch_at);
            let autoclose_at_for_timer = Rc::clone(&autoclose_at);
            let dialog_at_for_timer = Rc::clone(&dialog_at);
            ui.on_timer(move |fired| {
                if switch_at_for_timer.get() == Some(fired) {
                    Some(Msg::Switch)
                } else if autoclose_at_for_timer.get() == Some(fired) {
                    Some(Msg::Autoclose)
                } else if dialog_at_for_timer.get() == Some(fired) {
                    Some(Msg::DialogOpen)
                } else {
                    None
                }
            });

            // A screenshot run cannot hover, so show one tip outright. Show it
            // last, after every sibling exists, so `raise` puts it on top.
            if std::env::var("XUI_GALLERY_TOOLTIP").is_ok() {
                tip_link.show();
            }

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
                _icons: icons,
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
                _tip_link: tip_link,
                _tip_bar: tip_bar,
                _scroll: scroll,
                _tabs: tabs,
                _split: split,
                _swatches: swatches,
                _dialog: dialog,
                _dialog_button: dialog_button,
                _content: content,
            }
        },
    );
}

fn main() {
    let renderer = match std::env::var("XUI_BACKEND").as_deref() {
        Ok("canvas") => Renderer::Canvas,
        _ if xui_canvas::snapshot::Gallery::from_env().offscreen() => Renderer::Canvas,
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
