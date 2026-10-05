//! An offscreen light/dark snapshot of *every* portable widget in one window.
//!
//! The gallery exercises each widget's construction and paint path without a
//! desktop: `OffscreenBackend` composites with the same code as the windowed
//! backend, and the two PNGs it writes (`target/ui/widgets-{light,dark}.png`)
//! are the visual diff. The assertions catch the cheap, common regressions —
//! a widget that stops painting, a dark theme that renders blank, a layout that
//! paints nothing — while the files let a human eyeball a suspected change.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::{OffscreenBackend, RgbaImage};
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec};
use xui_core::image::Image;
use xui_core::prelude::*;
use xui_core::widget::{Dialog, Glyph, Tooltip};
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

/// The widgets, kept alive for the whole `run_app` call: dropping one destroys
/// its node, so a widget not held here would vanish before the render.
struct Gallery {
    _alive: Vec<Box<dyn Any>>,
}

impl App for Gallery {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The form controls, top to bottom; `link` gets the hyperlink, for its
/// tooltip.
fn controls(link: &Handle<Hyperlink<()>>) -> Layout<()> {
    column().gap(8).children((
        label("xui widgets"),
        edit().text("type here"),
        number_field(0.0, 100.0, 5.0),
        row().gap(10).children((
            checkbox("Enabled").width(134),
            toggle_button("Bold").fill(1),
        )),
        radio_group(&["Small", "Medium", "Large"]),
        combo_box(&["Alpha", "Beta", "Gamma"]),
        slider(0.0, 100.0),
        progress(100).height(8),
        hyperlink("Open docs").bind(link),
        top_bar().then(|bar| {
            let seek_id = TopBarId::new(4);
            bar.icon(TopBarId::new(1), Glyph::Menu)
                .icon(TopBarId::new(5), Glyph::Play)
                .icon(TopBarId::new(6), Glyph::Previous)
                .icon(TopBarId::new(7), Glyph::Next)
                .toggle(TopBarId::new(2), Glyph::Star)
                .toggle(TopBarId::new(8), Glyph::Repeat)
                .toggle(TopBarId::new(9), Glyph::Shuffle)
                .label(TopBarId::new(3), "xui")
                .slider(seek_id, 0.0, 100.0)
                .expand(seek_id)
        }),
        button("A button"),
        flow_text()
            .run(Run::normal("Flow text with a "))
            .run(Run::link("link"))
            .separator(" · ")
            .run(Run::weak("and a weak run")),
        // A page title, a section caption and the default action.
        label("Page title").title(),
        label("Section caption").caption(),
        button("Apply").primary().align(Align::Start),
    ))
}

/// The menu bar, the item views and the framed editors.
fn views() -> Layout<()> {
    let tree = tree_view()
        .rows(vec![
            TreeRow::new("Inbox", 0)
                .expandable(true)
                .expanded(true)
                .icon(Glyph::Folder),
            TreeRow::new("Work", 1).icon(Glyph::Tag),
            TreeRow::new("Home", 1).icon(art_image()),
            TreeRow::new("Archive", 0)
                .expandable(true)
                .icon(Glyph::History),
        ])
        // Select a nested row so the selection-aware indent guide is exercised.
        .then(|tree| {
            tree.select(Some(1));
            tree
        });
    column().gap(8).children((
        menu_bar(|menu| {
            menu.item(MenuId::new(1), "&Open");
            menu.separator();
            menu.check(MenuId::new(2), "Auto &save", true);
        }),
        list()
            .then(|list| {
                list.set_model(vec![
                    "Inbox".to_string(),
                    "Sent".to_string(),
                    "Drafts".to_string(),
                ]);
                list
            })
            .height(112),
        tree.height(96),
        group("Group", column().child(checkbox("Inside the group"))),
        multiline_edit()
            .then(|notes| {
                notes.set_text("Notes…");
                notes
            })
            .height(64),
        panel(column().padding(12).child(label("In a panel"))).height(68),
        text_toolbar(&["New", "Open", "Save"]),
    ))
}

/// The containers, each holding labels of its own.
fn containers() -> Layout<()> {
    let rows: Vec<_> = (1..=4)
        .map(|row| label(format!("Row {row}")).fixed(40))
        .collect();
    column().gap(12).children((
        scroll(column().children(rows)).height(184),
        tabs()
            .page("General", column().child(label("General")))
            .page("Advanced", column().child(label("Advanced")))
            .height(176),
        split(
            column().child(label("Left")),
            column().child(label("Right")),
        )
        .height(168),
    ))
}

/// The tile views and the colour choosers.
fn colors() -> Layout<()> {
    let palette = [
        Color::hex(0x00_78_D4),
        Color::hex(0x00_B2_94),
        Color::hex(0xE8_11_23),
    ];
    column().gap(10).children((
        grid_view(&["A", "B", "C"]).height(90),
        icon_view(&["Documents", "Pictures", "Music", "Videos"]).height(150),
        color_picker(&palette).columns(3).height(80),
        // The tabbed colour panel, on its first tab.
        color_panel().color(Color::hex(0xEB_40_34)).height(300),
    ))
}

/// Builds one of every portable widget, laid out so none overlaps another,
/// and returns the ones no layout holds.
fn build(ui: &Ui<()>) -> Vec<Box<dyn Any>> {
    let link = Handle::new();
    ui.root(
        column().padding(16).gap(8).children((
            row()
                .gap(16)
                .children((
                    controls(&link).width(360),
                    views().width(360),
                    containers().fill(1),
                    colors().width(240),
                    // The tabbed colour panel again, on its second tab.
                    column()
                        .child(
                            color_panel()
                                .color(Color::hex(0x00_78_D4))
                                .then(|full| {
                                    full.select_tab(1);
                                    full
                                })
                                .height(300),
                        )
                        .width(220),
                ))
                .fill(1),
            separator(),
            status_bar(&["Ready", ""]),
            material_status_bar(&["Native", "light"]),
            label("events appear here"),
        )),
    )
    .unwrap();

    // A tooltip, a context menu (hidden until shown) and a dialog (closed)
    // belong to the window, not to a layout.
    vec![
        Box::new(Tooltip::attach(ui, link.get().id(), "A tooltip").unwrap()),
        Box::new(Menu::context(ui).build(|menu| {
            menu.item(MenuId::new(10), "Cu&t");
            menu.item(MenuId::new(11), "&Copy");
        })),
        Box::new(
            Dialog::confirm(ui, "Save changes?", "Your edits will be lost otherwise.").unwrap(),
        ),
    ]
}

/// Writes a PNG snapshot under `target/ui/`.
fn save(name: &str, image: &RgbaImage) {
    let dir = std::path::Path::new("target/ui");
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(name);
    let file = std::fs::File::create(&path).expect("create snapshot");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(&image.pixels)
        .expect("png data");
}

/// Pixels that differ from the window background, i.e. anything a widget drew.
fn ink(image: &RgbaImage, background: Color) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| {
            pixel[0] != background.r || pixel[1] != background.g || pixel[2] != background.b
        })
        .count()
}

/// Pixels whose RGB differs between two images of the same size.
fn differing(a: &RgbaImage, b: &RgbaImage) -> usize {
    a.pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.pixels.as_chunks::<4>().0.iter())
        .filter(|(left, right)| left[..3] != right[..3])
        .count()
}

#[test]
fn every_portable_widget_renders_light_dark_and_midnight() {
    let backend = Rc::new(OffscreenBackend::new());
    let rendered: Rc<RefCell<Vec<RgbaImage>>> = Rc::new(RefCell::new(Vec::new()));

    let backend_for_make = Rc::clone(&backend);
    let rendered_for_make = Rc::clone(&rendered);
    let run: Rc<dyn Backend> = backend.clone();

    run_app(
        run,
        PlatformSpec::new("xui widget gallery").size(Dip(1500.0), Dip(900.0)),
        move |ui| {
            // The Midnight window with no widgets, so its gradient alone
            // cannot pass for painted widgets below.
            ui.set_theme(Theme::midnight());
            let empty = backend_for_make
                .render(ui.window())
                .expect("an empty midnight render");
            ui.set_theme(Theme::light());
            let alive = build(ui);

            let light = backend_for_make
                .render(ui.window())
                .expect("a light render");
            save("widgets-light.png", &light);

            ui.set_theme(Theme::dark());
            let dark = backend_for_make.render(ui.window()).expect("a dark render");
            save("widgets-dark.png", &dark);

            ui.set_theme(Theme::midnight());
            let midnight = backend_for_make
                .render(ui.window())
                .expect("a midnight render");
            save("widgets-midnight.png", &midnight);

            let mut rendered = rendered_for_make.borrow_mut();
            rendered.push(light);
            rendered.push(dark);
            rendered.push(midnight);
            rendered.push(empty);

            Gallery { _alive: alive }
        },
    )
    .expect("the offscreen gallery ran");

    let rendered = rendered.borrow();
    let light = &rendered[0];
    let dark = &rendered[1];
    let midnight = &rendered[2];
    let empty = &rendered[3];

    // Many widgets drew something on each theme.
    assert!(
        ink(light, Theme::light().background) > 2_000,
        "the light gallery painted: {}",
        ink(light, Theme::light().background)
    );
    assert!(
        ink(dark, Theme::dark().background) > 2_000,
        "the dark gallery painted: {}",
        ink(dark, Theme::dark().background)
    );
    assert!(
        differing(empty, midnight) > 2_000,
        "the midnight widgets painted over the bare window: {}",
        differing(empty, midnight)
    );
    // And the theme actually changed what was drawn.
    assert!(
        differing(dark, midnight) > 2_000,
        "dark and midnight differ: {}",
        differing(dark, midnight)
    );
    assert!(
        differing(light, dark) > 2_000,
        "light and dark differ: {}",
        differing(light, dark)
    );
}
