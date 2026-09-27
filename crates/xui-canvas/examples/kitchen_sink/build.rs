//! Builds the portable kitchen-sink widget tree: an emusic-shaped shell
//! (menu bar, transport top bar, navigator tree, four central pages) that
//! exercises the complex portable widgets on the canvas backend.

use std::any::Any;
use std::rc::Rc;

use xui_core::app::Ui;
use xui_core::color::Color;
use xui_core::geometry::Rect;
use xui_core::units::Dip;
use xui_core::widget::{
    Button, CheckBox, ColorPicker, ComboBox, Dialog, Edit, Fill, FlowText, Glyph, GridView,
    GroupBox, Hyperlink, Label, ListView, Menu, MenuId, MultilineEdit, NumberField, Panel,
    ProgressBar, RadioGroup, Run, ScrollView, Separator, Slider, Split, StatusBar, Tabs, TileSize,
    ToggleButton, Toolbar, Tooltip, TopBar, TopBarId, TreeRow, TreeView,
};

use super::app::{App, Msg, page_for_row};
use super::data;

/// The window's design size.
pub(crate) const WIDTH: f32 = 1280.0;
pub(crate) const HEIGHT: f32 = 800.0;

/// The central page area, in design units (the navigator is to its left).
const PAGE: (f32, f32, f32, f32) = (228.0, 80.0, 1272.0, 700.0);

pub(crate) fn build(ui: &mut Ui<Msg>) -> App {
    let dpi = ui.dpi();
    let p = move |value: f32| Dip(value).to_px(dpi).value();
    let r = move |l: f32, t: f32, ri: f32, b: f32| Rect::new(p(l), p(t), p(ri), p(b));
    let mut alive: Vec<Box<dyn Any>> = Vec::new();

    // --- menu bar -----------------------------------------------------------
    let menu_new = MenuId::new(1);
    let menu_quit = MenuId::new(2);
    let menu_dialog = MenuId::new(3);
    let menu_light = MenuId::new(4);
    let menu_dark = MenuId::new(5);
    let menu = Menu::bar(ui, r(8.0, 4.0, 1272.0, 32.0))
        .unwrap()
        .on_select(move |id| {
            if id == menu_new {
                Some(Msg::Navigate(0))
            } else if id == menu_dialog {
                Some(Msg::DialogOpen)
            } else if id == menu_light {
                Some(Msg::Theme(0))
            } else if id == menu_dark {
                Some(Msg::Theme(1))
            } else if id == menu_quit {
                Some(Msg::Autoclose)
            } else {
                Some(Msg::Tool(0))
            }
        })
        .build(|m| {
            m.submenu(MenuId::new(10), "&File", |f| {
                f.item(menu_new, "&New window");
                f.separator();
                f.item(menu_quit, "E&xit");
            });
            m.submenu(MenuId::new(11), "&View", |v| {
                v.item(menu_dialog, "Show &dialog");
                v.separator();
                v.item(menu_light, "Theme: &Light");
                v.item(menu_dark, "Theme: &Dark");
            });
        });
    alive.push(Box::new(menu));

    // --- transport top bar --------------------------------------------------
    let seek = TopBarId::new(1);
    let prev = TopBarId::new(2);
    let play = TopBarId::new(3);
    let next = TopBarId::new(4);
    let star = TopBarId::new(5);
    let repeat = TopBarId::new(6);
    let shuffle = TopBarId::new(7);
    let volume = TopBarId::new(8);
    let search = TopBarId::new(9);
    let topbar = TopBar::new(ui, r(8.0, 36.0, 1272.0, 72.0))
        .unwrap()
        .icon(prev, Glyph::Previous)
        .icon(play, Glyph::Play)
        .icon(next, Glyph::Next)
        .toggle(star, Glyph::Star)
        .toggle(repeat, Glyph::Repeat)
        .toggle(shuffle, Glyph::Shuffle)
        .slider(seek, 0.0, 100.0)
        .expand(seek)
        .label(TopBarId::new(10), "Portable kitchen sink")
        .slider(volume, 0.0, 100.0)
        .icon(search, Glyph::Search)
        .on_click(move |id| {
            if id == prev {
                Some(Msg::Tool(1))
            } else if id == play {
                Some(Msg::Tool(2))
            } else if id == next {
                Some(Msg::Tool(3))
            } else if id == search {
                Some(Msg::Search(String::new()))
            } else {
                None
            }
        })
        .on_toggle(move |id, checked| {
            (id == star || id == repeat || id == shuffle).then_some(Msg::Toggle(checked))
        })
        .on_change(move |id, value| (id == volume).then_some(Msg::Slide(value)));
    alive.push(Box::new(topbar));

    // --- navigator ----------------------------------------------------------
    let nav = TreeView::new(
        ui,
        r(8.0, 80.0, 220.0, 700.0),
        &[
            TreeRow::new("Library", 0)
                .expandable(true)
                .expanded(true)
                .icon(Glyph::Folder),
            TreeRow::new("Music", 1).icon(Glyph::Audio),
            TreeRow::new("Playlists", 1).icon(Glyph::Tag),
            TreeRow::new("Albums", 0).icon(Glyph::Album),
            TreeRow::new("Settings", 0).icon(Glyph::Settings),
            TreeRow::new("Notes", 0).icon(Glyph::Text("N")),
        ],
    )
    .unwrap()
    .on_select(|row| Some(Msg::Navigate(page_for_row(row))));
    nav.select(Some(0));

    // --- page 0: library ----------------------------------------------------
    let library = Panel::new(ui, r(PAGE.0, PAGE.1, PAGE.2, PAGE.3)).unwrap();
    let library_id = library.id();
    let search_edit = Edit::new(
        library.ui(),
        Rect::new(p(0.0), p(0.0), p(1044.0), p(28.0)),
        "Search the library",
    )
    .unwrap()
    .on_change(|text| Some(Msg::Search(text.to_string())));
    let tracks = Rc::new(data::tracks(4000));
    let order: Vec<usize> = (0..tracks.len()).collect();
    let list = ListView::with_model(
        library.ui(),
        Rect::new(p(0.0), p(32.0), p(1044.0), p(576.0)),
        data::TrackModel::new(Rc::clone(&tracks), order.clone()),
    )
    .unwrap()
    .column("Title", Fill)
    .column("Artist", Dip(150.0))
    .column("Album", Dip(150.0))
    .column("Genre", Dip(110.0))
    .column_right("Year", Dip(56.0))
    .column_right("Time", Dip(64.0))
    .column("Format", Dip(72.0))
    .column_right("Plays", Dip(56.0))
    .multi_select(true)
    .on_selection(|rows| Some(Msg::Select(rows.to_vec())))
    .on_activate(|row| Some(Msg::Activate(row)))
    .on_context(|row, at| Some(Msg::Context(row, at)))
    .on_sort(|column| Some(Msg::Sort(column)))
    .on_resize(|column, width| Some(Msg::Resize(column, width)));
    list.set_selection(&[1, 2, 3]);
    let library_tools = Toolbar::new(
        library.ui(),
        Rect::new(p(0.0), p(584.0), p(472.0), p(616.0)),
        &["Play", "Pause", "Next"],
    )
    .unwrap()
    .on_click(|index| Some(Msg::Tool(index)));
    let scan = ProgressBar::new(
        library.ui(),
        Rect::new(p(482.0), p(600.0), p(1044.0), p(608.0)),
        100,
    )
    .unwrap();
    scan.set_value(40);
    let list_tip = Tooltip::attach(library.ui(), list.id(), "Right-click a row").unwrap();
    let scan_tip = Tooltip::attach(library.ui(), scan.id(), "Scan progress").unwrap();
    alive.push(Box::new(library));
    alive.push(Box::new(search_edit));
    alive.push(Box::new(library_tools));
    alive.push(Box::new(scan));
    alive.push(Box::new(list_tip));
    alive.push(Box::new(scan_tip));

    // --- page 1: albums -----------------------------------------------------
    let albums = Panel::new(ui, r(PAGE.0, PAGE.1, PAGE.2, PAGE.3)).unwrap();
    let albums_id = albums.id();
    let album_data = Rc::new(data::albums(600));
    let grid = GridView::with_model(
        albums.ui(),
        Rect::new(p(0.0), p(0.0), p(1044.0), p(620.0)),
        data::AlbumModel::new(Rc::clone(&album_data)),
    )
    .unwrap()
    .tile_size(TileSize::new(Dip(150.0), Dip(170.0)).gap(Dip(12.0)))
    .on_select(|index| Some(Msg::AlbumSelect(index)))
    .on_activate(|index| Some(Msg::AlbumActivate(index)))
    .on_paint_tile(|canvas, paint| {
        let dpi = paint.dpi;
        let inner = Dip(6.0).to_px(dpi).value();
        let caption = Dip(34.0).to_px(dpi).value();
        let rect = paint.rect;
        let art = Rect::new(
            rect.left + inner,
            rect.top + inner,
            rect.right - inner,
            rect.bottom - inner - caption,
        );
        if let Some(image) = paint.tile.image {
            canvas.draw_image(image, art);
        }
        let text = Rect::new(
            rect.left + inner,
            art.bottom,
            rect.right - inner,
            rect.bottom - inner,
        );
        let style =
            xui_core::backend::TextStyle::new(paint.theme.text_secondary, Dip(11.0)).middle();
        canvas.draw_text(paint.tile.label, text, &style);
    });
    alive.push(Box::new(grid));
    alive.push(Box::new(albums));

    // --- page 2: settings ---------------------------------------------------
    let settings = Panel::new(ui, r(PAGE.0, PAGE.1, PAGE.2, PAGE.3)).unwrap();
    let settings_id = settings.id();
    let tabs = Tabs::new(
        settings.ui(),
        Rect::new(p(0.0), p(0.0), p(1044.0), p(620.0)),
    )
    .unwrap()
    .on_change(|index| Some(Msg::Tool(100 + index)));

    let appearance = Panel::new(tabs.ui(), Rect::default()).unwrap();
    let theme_group = GroupBox::new(appearance.ui(), r(0.0, 0.0, 510.0, 300.0), "Theme").unwrap();
    let theme_choice = RadioGroup::new(
        appearance.ui(),
        r(16.0, 36.0, 494.0, 150.0),
        &["Light", "Dark"],
    )
    .unwrap()
    .on_select(|index| Some(Msg::Theme(index)));
    let follow = CheckBox::new(
        appearance.ui(),
        r(16.0, 160.0, 494.0, 188.0),
        "Follow the system",
    )
    .unwrap()
    .on_toggle(|checked| Some(Msg::Check(checked)));
    let accent_group =
        GroupBox::new(appearance.ui(), r(0.0, 312.0, 510.0, 612.0), "Accent").unwrap();
    let palette = [
        Color::hex(0x00_78_D4),
        Color::hex(0x00_B2_94),
        Color::hex(0x10_7C_10),
        Color::hex(0xFF_B9_00),
        Color::hex(0xFF_8C_00),
        Color::hex(0xE8_11_23),
        Color::hex(0x87_64_B8),
        Color::hex(0x4C_4A_48),
        Color::hex(0x00_5F_B8),
        Color::hex(0x6B_2E_8B),
        Color::hex(0x00_86_7B),
        Color::hex(0xC2_3B_22),
    ];
    let swatches = ColorPicker::new(appearance.ui(), r(16.0, 344.0, 494.0, 468.0), &palette)
        .unwrap()
        .columns(6)
        .selected(palette[0])
        .on_select(|color| Some(Msg::Swatch(color)));
    let playback_group =
        GroupBox::new(appearance.ui(), r(522.0, 0.0, 1044.0, 300.0), "Playback").unwrap();
    let gapless = CheckBox::new(appearance.ui(), r(538.0, 36.0, 1028.0, 64.0), "Gapless")
        .unwrap()
        .on_toggle(|checked| Some(Msg::Check(checked)));
    let replay = ToggleButton::new(appearance.ui(), r(538.0, 72.0, 1028.0, 100.0), "ReplayGain")
        .unwrap()
        .on_toggle(|checked| Some(Msg::Toggle(checked)));
    let mode = ComboBox::new(
        appearance.ui(),
        r(538.0, 108.0, 1028.0, 136.0),
        &["None", "Track", "Album"],
    )
    .unwrap()
    .on_select(|index| Some(Msg::Combo(index)));
    let crossfade = Slider::new(appearance.ui(), r(538.0, 144.0, 1028.0, 172.0), 0.0, 12.0)
        .unwrap()
        .on_change(|value| Some(Msg::Slide(value)));
    let buffer = NumberField::new(
        appearance.ui(),
        r(538.0, 180.0, 1028.0, 208.0),
        1.0,
        60.0,
        1.0,
    )
    .unwrap()
    .on_commit(|value| Some(Msg::Number(value)));
    let library_group =
        GroupBox::new(appearance.ui(), r(522.0, 312.0, 1044.0, 612.0), "Library").unwrap();
    let folders = ListView::new(
        appearance.ui(),
        r(538.0, 344.0, 1028.0, 520.0),
        &data::folders(),
    )
    .unwrap()
    .column("Folder", Fill)
    .on_select(|row| Some(Msg::Folder(row)));
    let add = Button::new(appearance.ui(), r(538.0, 528.0, 760.0, 556.0), "Add folder")
        .unwrap()
        .on_click(|| Some(Msg::Tool(200)));
    let rescan = Button::new(appearance.ui(), r(770.0, 528.0, 1028.0, 556.0), "Rescan")
        .unwrap()
        .on_click(|| Some(Msg::Tool(201)));

    // The About tab holds a ScrollView: its rows are laid out top to bottom by
    // the engine and clipped to the viewport.
    let about_page = Panel::new(tabs.ui(), Rect::default()).unwrap();
    let about = ScrollView::new(
        about_page.ui(),
        Rect::new(p(0.0), p(0.0), p(1044.0), p(620.0)),
    )
    .unwrap();
    let mut about_rows: Vec<Label<Msg>> = Vec::new();
    for line in 0..14 {
        let label = Label::new(
            about.ui(),
            Rect::new(0, 0, 10, 10),
            &format!("About line {line}: the canvas backend clips this to the viewport"),
        )
        .unwrap();
        about.add(label.id(), Dip(30.0));
        about_rows.push(label);
    }
    let tabs = tabs
        .page("Appearance", &[appearance.id()])
        .page("About", &[about_page.id()]);

    alive.push(Box::new(settings));
    alive.push(Box::new(theme_group));
    alive.push(Box::new(theme_choice));
    alive.push(Box::new(follow));
    alive.push(Box::new(accent_group));
    alive.push(Box::new(swatches));
    alive.push(Box::new(playback_group));
    alive.push(Box::new(gapless));
    alive.push(Box::new(replay));
    alive.push(Box::new(mode));
    alive.push(Box::new(crossfade));
    alive.push(Box::new(buffer));
    alive.push(Box::new(library_group));
    alive.push(Box::new(folders));
    alive.push(Box::new(add));
    alive.push(Box::new(rescan));
    alive.push(Box::new(appearance));
    alive.push(Box::new(about_page));
    alive.push(Box::new(about));
    alive.extend(
        about_rows
            .into_iter()
            .map(|row| Box::new(row) as Box<dyn Any>),
    );
    alive.push(Box::new(tabs));

    // --- page 3: notes ------------------------------------------------------
    let notes = Panel::new(ui, r(PAGE.0, PAGE.1, PAGE.2, PAGE.3)).unwrap();
    let notes_id = notes.id();
    let split = Split::row(notes.ui(), Rect::new(p(0.0), p(0.0), p(1044.0), p(360.0))).unwrap();
    let note_a = MultilineEdit::new(
        split.ui(),
        Rect::new(p(0.0), p(0.0), p(500.0), p(340.0)),
        "",
    )
    .unwrap();
    let note_b = MultilineEdit::new(
        split.ui(),
        Rect::new(p(0.0), p(0.0), p(500.0), p(340.0)),
        "",
    )
    .unwrap();
    split.pane_a(&[note_a.id()]);
    split.pane_b(&[note_b.id()]);
    split.set_min(Dip(120.0), Dip(80.0));
    split.set_position(Dip(520.0));
    let flow = FlowText::new(notes.ui(), r(0.0, 372.0, 1044.0, 420.0))
        .unwrap()
        .run(Run::normal("The portable kitchen sink exercises "))
        .run(Run::link("complex widgets"))
        .separator(" · ")
        .run(Run::weak("with a weak run"));
    let link = Hyperlink::new(notes.ui(), r(0.0, 432.0, 400.0, 460.0), "Open the docs")
        .unwrap()
        .on_click(|| Some(Msg::Link));
    let wrap = ToggleButton::new(notes.ui(), r(0.0, 470.0, 240.0, 498.0), "Word wrap")
        .unwrap()
        .on_toggle(|checked| Some(Msg::Toggle(checked)));
    let note_progress = ProgressBar::new(notes.ui(), r(0.0, 520.0, 1044.0, 528.0), 100).unwrap();
    note_progress.set_value(60);
    let separator = Separator::new(notes.ui(), r(0.0, 540.0, 1044.0, 542.0)).unwrap();
    let hint = Label::new(
        notes.ui(),
        r(0.0, 550.0, 1044.0, 580.0),
        "Both panes are MultilineEdit in a Split; the text above is FlowText.",
    )
    .unwrap();
    alive.push(Box::new(notes));
    alive.push(Box::new(split));
    alive.push(Box::new(note_a));
    alive.push(Box::new(note_b));
    alive.push(Box::new(flow));
    alive.push(Box::new(link));
    alive.push(Box::new(wrap));
    alive.push(Box::new(note_progress));
    alive.push(Box::new(separator));
    alive.push(Box::new(hint));

    // --- status bars and dialog --------------------------------------------
    let status = StatusBar::new(
        ui,
        r(8.0, 704.0, 1272.0, 734.0),
        &["Ready", "portable canvas"],
    )
    .unwrap();
    let material = xui_core::widget::MaterialStatusBar::new(
        ui,
        r(8.0, 738.0, 1272.0, 768.0),
        &["canvas", "light"],
    )
    .unwrap();
    let dialog = Dialog::confirm(
        ui,
        "Apply changes?",
        "This dialog is a painted scrim, so it renders on the canvas backend.",
    )
    .unwrap()
    .accept_label("Apply")
    .on_action(|action| Some(Msg::DialogAction(action)));
    let context = Menu::context(ui)
        .on_select(|_id| Some(Msg::Tool(300)))
        .build(|m| {
            m.item(MenuId::new(20), "&Play");
            m.item(MenuId::new(21), "&Queue");
            m.separator();
            m.item(MenuId::new(22), "&Properties");
        });
    alive.push(Box::new(material));

    let pages = vec![library_id, albums_id, settings_id, notes_id];
    for (index, id) in pages.iter().enumerate() {
        ui.set_visible(*id, index == 0);
    }

    App::new(
        status, nav, dialog, context, list, tracks, order, pages, alive,
    )
}
