//! A gallery of the portable `xui-core` widgets, hosted on the Win32 backend,
//! so you can play with them.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui-win32 --example widgets
//! ```
//!
//! `WIN32UI_DEMO_AUTOCLOSE_MS` makes it quit itself, for a headless smoke run.

#[cfg(windows)]
mod gallery {
    use std::rc::Rc;

    use xui_core::app::{App, Ui, run_app};
    use xui_core::backend::{Backend, PlatformSpec};
    use xui_core::widget::{
        Button, CheckBox, ComboBox, Edit, GroupBox, HasText, Hyperlink, Label, ListView,
        MultilineEdit, NumberField, Panel, ProgressBar, RadioGroup, Separator, Slider, StatusBar,
        ToggleButton, Toolbar, TreeRow, TreeView,
    };
    use xui_core::{Dip, Rect, Theme};
    use xui_win32::{SystemTheme, Win32Backend};

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
        Link,
        Click,
        Theme(usize),
        Autoclose,
    }

    struct Gallery {
        echo: Label<Msg>,
        status: StatusBar<Msg>,
        bar: ProgressBar<Msg>,
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
        _sep: Separator<Msg>,
        _theme: RadioGroup<Msg>,
        _button: Button<Msg>,
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
                Msg::Link => "link clicked".to_string(),
                Msg::Click => "button clicked".to_string(),
                Msg::Theme(choice) => match choice {
                    1 => {
                        ui.set_theme(Theme::dark());
                        "theme: dark".to_string()
                    }
                    2 => {
                        ui.set_theme(Theme::system());
                        "theme: system".to_string()
                    }
                    _ => {
                        ui.set_theme(Theme::light());
                        "theme: light".to_string()
                    }
                },
                Msg::Autoclose => {
                    ui.quit();
                    return;
                }
            };
            self.status.set_text(0, "Ready");
            self.status.set_text(1, &note);
        }
    }

    pub(crate) fn main() {
        let backend: Rc<dyn Backend> = Rc::new(Win32Backend::new());
        let _ = run_app(
            backend,
            PlatformSpec::new("xui widgets").size(Dip(780.0), Dip(640.0)),
            |ui| {
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
                let button = Button::new(ui, rect(16.0, 458.0, 190.0, 486.0), "Click me")
                    .unwrap()
                    .on_click(|| Some(Msg::Click));
                let theme = RadioGroup::new(
                    ui,
                    rect(16.0, 366.0, 190.0, 450.0),
                    &["Light", "Dark", "System"],
                )
                .unwrap()
                .on_select(|index| Some(Msg::Theme(index)));
                if std::env::var("WIN32UI_GALLERY_THEME").as_deref() == Ok("dark") {
                    ui.set_theme(Theme::dark());
                    theme.select(1);
                }
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
                let grouped =
                    CheckBox::new(ui, rect(416.0, 312.0, 748.0, 340.0), "Inside the group")
                        .unwrap()
                        .on_toggle(|checked| Some(Msg::Check(checked)));

                let multi =
                    MultilineEdit::new(ui, rect(400.0, 364.0, 764.0, 428.0), "Notes…").unwrap();

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
                let sep = Separator::new(ui, rect(16.0, 552.0, 764.0, 554.0)).unwrap();
                let status =
                    StatusBar::new(ui, rect(16.0, 560.0, 764.0, 584.0), &["Ready", ""]).unwrap();

                let echo = Label::new(ui, rect(16.0, 590.0, 764.0, 618.0), "Edit: ").unwrap();

                if let Ok(millis) = std::env::var("WIN32UI_DEMO_AUTOCLOSE_MS") {
                    let _ = millis.parse::<u32>().map(|ms| ui.set_timer(ms));
                    ui.on_timer(|_| Some(Msg::Autoclose));
                }

                Gallery {
                    echo,
                    status,
                    bar,
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
                    _sep: sep,
                    _theme: theme,
                    _button: button,
                }
            },
        );
    }
}

#[cfg(windows)]
fn main() {
    gallery::main();
}

#[cfg(not(windows))]
fn main() {}
