//! Builds the portable kitchen-sink widget tree: an emusic-shaped shell
//! (menu bar, transport top bar, navigator tree, four central pages) that
//! exercises the complex portable widgets on the canvas backend.

use std::rc::Rc;

use xui_core::color::Color;
use xui_core::geometry::Rect;
use xui_core::prelude::*;
use xui_core::widget::{Dialog, Glyph, TileSize, Tooltip};

use super::app::{App, Msg, page_for_row};
use super::data;

/// The window's design size.
pub(crate) const WIDTH: f32 = 1280.0;
pub(crate) const HEIGHT: f32 = 800.0;

/// The handles `build` fills and the app keeps.
struct Handles {
    status: Handle<StatusBar<Msg>>,
    nav: Handle<TreeView<Msg>>,
    list: Handle<ListView<Msg>>,
    scan: Handle<ProgressBar<Msg>>,
    pages: Vec<Handle<Panel<Msg>>>,
}

pub(crate) fn build(ui: &mut Ui<Msg>) -> App {
    let handles = Handles {
        status: Handle::new(),
        nav: Handle::new(),
        list: Handle::new(),
        scan: Handle::new(),
        pages: (0..4).map(|_| Handle::new()).collect(),
    };
    let tracks = Rc::new(data::tracks(4000));
    let order: Vec<usize> = (0..tracks.len()).collect();
    let model = data::TrackModel::new(Rc::clone(&tracks), order.clone());

    let page = |index: usize, content: Layout<Msg>| panel(content).bind(&handles.pages[index]);
    ui.root(
        column().padding(8).gap(4).children((
            menu(),
            transport().height(36),
            row()
                .gap(8)
                .children((
                    navigator().bind(&handles.nav).width(212),
                    stack()
                        .children((
                            page(0, library(&handles, model)),
                            page(1, albums()),
                            page(2, settings()),
                            page(3, notes()),
                        ))
                        .fill(1),
                ))
                .fill(1),
            status_bar(&["Ready", "portable canvas"])
                .bind(&handles.status)
                .height(30),
            material_status_bar(&["canvas", "light"]).height(30),
        )),
    )
    .expect("the kitchen sink layout");

    let tips = vec![
        Tooltip::attach(ui, handles.list.get().id(), "Right-click a row").unwrap(),
        Tooltip::attach(ui, handles.scan.get().id(), "Scan progress").unwrap(),
    ];
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

    let app = App {
        status: handles.status,
        nav: handles.nav,
        dialog,
        context,
        list: handles.list,
        tracks,
        order,
        sort: None,
        pages: handles.pages,
        _tips: tips,
    };
    app.show_page(0, ui);
    app.nav.get().select(Some(0));
    app
}

/// The File and View menus.
fn menu() -> Build<Menu<Msg>, Msg> {
    let menu_new = MenuId::new(1);
    let menu_quit = MenuId::new(2);
    let menu_dialog = MenuId::new(3);
    let menu_light = MenuId::new(4);
    let menu_dark = MenuId::new(5);
    menu_bar(move |m| {
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
    })
    .on_select(move |id| {
        if id == menu_new {
            Msg::Navigate(0)
        } else if id == menu_dialog {
            Msg::DialogOpen
        } else if id == menu_light {
            Msg::Theme(0)
        } else if id == menu_dark {
            Msg::Theme(1)
        } else if id == menu_quit {
            Msg::Autoclose
        } else {
            Msg::Tool(0)
        }
    })
}

/// The transport band: playback buttons, toggles, seek and volume.
fn transport() -> Build<TopBar<Msg>, Msg> {
    let seek = TopBarId::new(1);
    let prev = TopBarId::new(2);
    let play = TopBarId::new(3);
    let next = TopBarId::new(4);
    let star = TopBarId::new(5);
    let repeat = TopBarId::new(6);
    let shuffle = TopBarId::new(7);
    let volume = TopBarId::new(8);
    let search = TopBarId::new(9);
    top_bar().then(move |bar| {
        bar.icon(prev, Glyph::Previous)
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
            .on_change(move |id, value| (id == volume).then_some(Msg::Slide(value)))
    })
}

/// The navigator tree, one row per page (and two library sub-rows).
fn navigator() -> Build<TreeView<Msg>, Msg> {
    tree_view()
        .rows(vec![
            TreeRow::new("Library", 0)
                .expandable(true)
                .expanded(true)
                .icon(Glyph::Folder),
            TreeRow::new("Music", 1).icon(Glyph::Audio),
            TreeRow::new("Playlists", 1).icon(Glyph::Tag),
            TreeRow::new("Albums", 0).icon(Glyph::Album),
            TreeRow::new("Settings", 0).icon(Glyph::Settings),
            TreeRow::new("Notes", 0).icon(Glyph::Text("N")),
        ])
        .on_select(|row| Msg::Navigate(page_for_row(row)))
}

/// Page 0: a search field over the track table, with a toolbar and a scan bar.
fn library(handles: &Handles, model: data::TrackModel) -> Layout<Msg> {
    let tracks = list()
        .column("Title", Fill)
        .column("Artist", Dip(150.0))
        .column("Album", Dip(150.0))
        .column("Genre", Dip(110.0))
        .column_right("Year", Dip(56.0))
        .column_right("Time", Dip(64.0))
        .column("Format", Dip(72.0))
        .column_right("Plays", Dip(56.0))
        .on_activate(Msg::Activate)
        .then(move |list| {
            let list = list
                .multi_select(true)
                .on_selection(|rows| Some(Msg::Select(rows.to_vec())))
                .on_context(|row, at| Some(Msg::Context(row, at)))
                .on_sort(|column| Some(Msg::Sort(column)))
                .on_resize(|column, width| Some(Msg::Resize(column, width)));
            list.set_model(model);
            list.set_selection(&[1, 2, 3]);
            list
        })
        .bind(&handles.list);
    column().padding(8).gap(4).children((
        edit().text("Search the library").on_change(Msg::Search),
        tracks.fill(1),
        row().gap(10).align(Align::Center).children((
            text_toolbar(&["Play", "Pause", "Next"])
                .on_click(Msg::Tool)
                .width(472),
            progress(100)
                .value(40)
                .bind(&handles.scan)
                .height(8)
                .fill(1),
        )),
    ))
}

/// Page 1: a virtual grid of album tiles, painted by the app.
fn albums() -> Layout<Msg> {
    let albums = Rc::new(data::albums(600));
    let grid = grid_view_with(data::AlbumModel::new(albums))
        .then(|grid| {
            grid.tile_size(TileSize::new(Dip(150.0), Dip(170.0)).gap(Dip(12.0)))
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
                        xui_core::backend::TextStyle::new(paint.theme.text_secondary, Dip(11.0))
                            .middle();
                    canvas.draw_text(paint.tile.label, text, &style);
                })
        })
        .on_select(Msg::AlbumSelect)
        .on_activate(Msg::AlbumActivate);
    column().padding(8).child(grid.fill(1))
}

/// Page 2: tabs of grouped settings and a scrolling About page.
fn settings() -> Layout<Msg> {
    let about: Vec<_> = (0..14)
        .map(|line| {
            label(format!(
                "About line {line}: the canvas backend clips this to the viewport"
            ))
            .fixed(30)
        })
        .collect();
    column().padding(8).child(
        tabs()
            .on_change(|index| Msg::Tool(100 + index))
            .page("Appearance", appearance())
            .page(
                "About",
                column().child(scroll(column().children(about)).fill(1)),
            )
            .fill(1),
    )
}

/// The Appearance tab: theme, playback, accent and library groups in a grid.
fn appearance() -> Layout<Msg> {
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
    let folders: Vec<String> = data::folders().iter().map(|f| f.to_string()).collect();
    grid([Track::Fill(1), Track::Fill(1)])
        .padding(12)
        .gap(12)
        .children((
            group(
                "Theme",
                column().gap(8).children((
                    radio_group(&["Light", "Dark"]).on_select(Msg::Theme),
                    checkbox("Follow the system").on_toggle(Msg::Check),
                )),
            )
            .fill(1),
            group(
                "Playback",
                column().gap(8).children((
                    checkbox("Gapless").on_toggle(Msg::Check),
                    toggle_button("ReplayGain").on_toggle(Msg::Toggle),
                    combo_box(&["None", "Track", "Album"]).on_select(Msg::Combo),
                    slider(0.0, 12.0).on_change(Msg::Slide),
                    number_field(1.0, 60.0, 1.0)
                        .then(|field| field.on_commit(|value| Some(Msg::Number(value)))),
                )),
            ),
            group(
                "Accent",
                column().child(
                    color_picker(&palette)
                        .columns(6)
                        .selected(palette[0])
                        .on_select(Msg::Swatch)
                        .height(124),
                ),
            )
            .fill(1),
            group(
                "Library",
                column().gap(8).children((
                    list()
                        .column("Folder", Fill)
                        .then(move |list| {
                            list.set_model(folders);
                            list
                        })
                        .on_select(Msg::Folder)
                        .fill(1),
                    row().gap(10).children((
                        button("Add folder").on_click(Msg::Tool(200)).fill(1),
                        button("Rescan").on_click(Msg::Tool(201)).fill(1),
                    )),
                )),
            ),
        ))
}

/// Page 3: two notes in a split, then flow text and assorted small widgets.
fn notes() -> Layout<Msg> {
    let pane = || column().child(multiline_edit().fill(1));
    column().padding(8).gap(12).children((
        split(pane(), pane()).min(120, 80).position(520).height(360),
        flow_text()
            .run(Run::normal("The portable kitchen sink exercises "))
            .run(Run::link("complex widgets"))
            .separator(" · ")
            .run(Run::weak("with a weak run")),
        hyperlink("Open the docs")
            .on_click(Msg::Link)
            .align(Align::Start),
        toggle_button("Word wrap").on_toggle(Msg::Toggle).width(240),
        progress(100).value(60).height(8),
        separator(),
        label("Both panes are MultilineEdit in a Split; the text above is FlowText."),
    ))
}
