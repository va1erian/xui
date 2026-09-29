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
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::widget::{
    Button, CheckBox, ColorPanel, ColorPicker, ComboBox, Dialog, Edit, FlowText, Glyph, GridView,
    GroupBox, Hyperlink, IconView, Label, ListView, MaterialStatusBar, Menu, MenuId, MultilineEdit,
    NumberField, Panel, ProgressBar, RadioGroup, Run, ScrollView, Separator, Slider, Split,
    StatusBar, Tabs, ToggleButton, Toolbar, Tooltip, TopBar, TopBarId, TreeRow, TreeView,
};
use xui_core::{Color, Dip, Theme};

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect::new(left, top, right, bottom)
}

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

/// Builds one of every portable widget, laid out so none overlaps another.
fn build(ui: &Ui<()>) -> Vec<Box<dyn Any>> {
    // Seeded with the first widget so the collection is not built up by
    // `Vec::new()` followed only by pushes.
    let mut keep: Vec<Box<dyn Any>> = vec![Box::new(
        Label::new(ui, rect(16, 12, 380, 40), "xui widgets").unwrap(),
    )];
    keep.push(Box::new(
        Edit::new(ui, rect(16, 48, 380, 76), "type here").unwrap(),
    ));
    keep.push(Box::new(
        NumberField::new(ui, rect(16, 84, 380, 112), 0.0, 100.0, 5.0).unwrap(),
    ));
    keep.push(Box::new(
        CheckBox::new(ui, rect(16, 120, 150, 148), "Enabled").unwrap(),
    ));
    keep.push(Box::new(
        ToggleButton::new(ui, rect(160, 120, 380, 148), "Bold").unwrap(),
    ));
    keep.push(Box::new(
        RadioGroup::new(ui, rect(16, 156, 380, 240), &["Small", "Medium", "Large"]).unwrap(),
    ));
    keep.push(Box::new(
        ComboBox::new(ui, rect(16, 248, 380, 276), &["Alpha", "Beta", "Gamma"]).unwrap(),
    ));
    keep.push(Box::new(
        Slider::new(ui, rect(16, 284, 380, 312), 0.0, 100.0).unwrap(),
    ));
    keep.push(Box::new(
        ProgressBar::new(ui, rect(16, 320, 380, 328), 100).unwrap(),
    ));
    let link = Hyperlink::new(ui, rect(16, 336, 380, 364), "Open docs").unwrap();
    keep.push(Box::new(
        Tooltip::attach(ui, link.id(), "A tooltip").unwrap(),
    ));
    keep.push(Box::new(link));
    keep.push(Box::new(
        Button::new(ui, rect(16, 458, 380, 486), "A button").unwrap(),
    ));
    keep.push(Box::new(
        FlowText::new(ui, rect(16, 500, 760, 544))
            .unwrap()
            .run(Run::normal("Flow text with a "))
            .run(Run::link("link"))
            .separator(" · ")
            .run(Run::weak("and a weak run")),
    ));

    // Menus: a bar plus a context menu (the popup itself is hidden until shown).
    keep.push(Box::new(
        Menu::bar(ui, rect(400, 12, 764, 40))
            .unwrap()
            .build(|menu| {
                menu.item(MenuId::new(1), "&Open");
                menu.separator();
                menu.check(MenuId::new(2), "Auto &save", true);
            }),
    ));
    keep.push(Box::new(Menu::context(ui).build(|menu| {
        menu.item(MenuId::new(10), "Cu&t");
        menu.item(MenuId::new(11), "&Copy");
    })));

    keep.push(Box::new(
        ListView::new(ui, rect(400, 48, 764, 160), &["Inbox", "Sent", "Drafts"]).unwrap(),
    ));
    let tree = TreeView::new(
        ui,
        rect(400, 168, 764, 264),
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
    .unwrap();
    // Select a nested row so the selection-aware indent guide is exercised.
    tree.select(Some(1));
    keep.push(Box::new(tree));
    keep.push(Box::new(
        GridView::with_model(
            ui,
            rect(1040, 48, 1280, 240),
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
        )
        .unwrap(),
    ));
    keep.push(Box::new(
        IconView::new(
            ui,
            rect(1040, 148, 1280, 300),
            &["Documents", "Pictures", "Music", "Videos"],
        )
        .unwrap(),
    ));

    keep.push(Box::new(
        GroupBox::new(ui, rect(400, 276, 764, 356), "Group").unwrap(),
    ));
    keep.push(Box::new(
        CheckBox::new(ui, rect(416, 312, 748, 340), "Inside the group").unwrap(),
    ));
    keep.push(Box::new(
        MultilineEdit::new(ui, rect(400, 364, 764, 428), "Notes…").unwrap(),
    ));

    // A panel owns the label created through its scoped `ui()`.
    let panel = Panel::new(ui, rect(400, 436, 764, 504)).unwrap();
    keep.push(Box::new(
        Label::new(panel.ui(), rect(12, 12, 300, 40), "In a panel").unwrap(),
    ));
    keep.push(Box::new(panel));

    keep.push(Box::new(
        Toolbar::new(ui, rect(400, 512, 764, 544), &["New", "Open", "Save"]).unwrap(),
    ));

    let new_id = TopBarId::new(1);
    let star_id = TopBarId::new(2);
    let seek_id = TopBarId::new(4);
    keep.push(Box::new(
        TopBar::new(ui, rect(16, 396, 380, 424))
            .unwrap()
            .icon(new_id, Glyph::Menu)
            .icon(TopBarId::new(5), Glyph::Play)
            .icon(TopBarId::new(6), Glyph::Previous)
            .icon(TopBarId::new(7), Glyph::Next)
            .toggle(star_id, Glyph::Star)
            .toggle(TopBarId::new(8), Glyph::Repeat)
            .toggle(TopBarId::new(9), Glyph::Shuffle)
            .label(TopBarId::new(3), "xui")
            .slider(seek_id, 0.0, 100.0)
            .expand(seek_id),
    ));

    // Containers: the children are created through the container's `ui()` and
    // registered with it, then all of them are kept alive.
    let scroll = ScrollView::new(ui, rect(788, 48, 1024, 232)).unwrap();
    for row in 1..=4 {
        let label = Label::new(scroll.ui(), rect(0, 0, 10, 10), &format!("Row {row}")).unwrap();
        scroll.add(label.id(), Dip(40.0));
        keep.push(Box::new(label));
    }
    keep.push(Box::new(scroll));

    let tabs = Tabs::new(ui, rect(788, 244, 1024, 420)).unwrap();
    let general = Label::new(tabs.ui(), rect(0, 0, 10, 10), "General").unwrap();
    let advanced = Label::new(tabs.ui(), rect(0, 0, 10, 10), "Advanced").unwrap();
    // `page` consumes the handle, so rebind as the builder is chained.
    let tabs = tabs
        .page("General", &[general.id()])
        .page("Advanced", &[advanced.id()]);
    keep.push(Box::new(general));
    keep.push(Box::new(advanced));
    keep.push(Box::new(tabs));

    let split = Split::row(ui, rect(788, 432, 1024, 600)).unwrap();
    let pane_a = Label::new(split.ui(), rect(0, 0, 10, 10), "Left").unwrap();
    let pane_b = Label::new(split.ui(), rect(0, 0, 10, 10), "Right").unwrap();
    split.pane_a(&[pane_a.id()]);
    split.pane_b(&[pane_b.id()]);
    keep.push(Box::new(pane_a));
    keep.push(Box::new(pane_b));
    keep.push(Box::new(split));

    let palette = [
        Color::hex(0x00_78_D4),
        Color::hex(0x00_B2_94),
        Color::hex(0xE8_11_23),
    ];
    keep.push(Box::new(
        ColorPicker::new(ui, rect(1040, 260, 1280, 340), &palette)
            .unwrap()
            .columns(3),
    ));
    // The tabbed colour panel, one instance on each tab.
    keep.push(Box::new(
        ColorPanel::new(ui, rect(1040, 350, 1260, 650))
            .unwrap()
            .with_color(Color::hex(0xEB_40_34)),
    ));
    let full = ColorPanel::new(ui, rect(1266, 350, 1486, 650))
        .unwrap()
        .with_color(Color::hex(0x00_78_D4));
    full.select_tab(1);
    keep.push(Box::new(full));
    keep.push(Box::new(
        Dialog::confirm(ui, "Save changes?", "Your edits will be lost otherwise.").unwrap(),
    ));

    keep.push(Box::new(
        Separator::new(ui, rect(16, 552, 1024, 554)).unwrap(),
    ));
    keep.push(Box::new(
        StatusBar::new(ui, rect(16, 560, 1024, 584), &["Ready", ""]).unwrap(),
    ));
    keep.push(Box::new(
        MaterialStatusBar::new(ui, rect(16, 588, 1024, 612), &["Native", "light"]).unwrap(),
    ));
    keep.push(Box::new(
        Label::new(ui, rect(16, 620, 1024, 644), "events appear here").unwrap(),
    ));

    keep
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
fn every_portable_widget_renders_light_and_dark() {
    let backend = Rc::new(OffscreenBackend::new());
    let rendered: Rc<RefCell<Vec<RgbaImage>>> = Rc::new(RefCell::new(Vec::new()));

    let backend_for_make = Rc::clone(&backend);
    let rendered_for_make = Rc::clone(&rendered);
    let run: Rc<dyn Backend> = backend.clone();

    run_app(
        run,
        PlatformSpec::new("xui widget gallery").size(Dip(1500.0), Dip(900.0)),
        move |ui| {
            let alive = build(ui);

            let light = backend_for_make
                .render(ui.window())
                .expect("a light render");
            save("widgets-light.png", &light);

            ui.set_theme(Theme::dark());
            let dark = backend_for_make.render(ui.window()).expect("a dark render");
            save("widgets-dark.png", &dark);

            let mut rendered = rendered_for_make.borrow_mut();
            rendered.push(light);
            rendered.push(dark);

            Gallery { _alive: alive }
        },
    )
    .expect("the offscreen gallery ran");

    let rendered = rendered.borrow();
    let light = &rendered[0];
    let dark = &rendered[1];

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
    // And the theme actually changed what was drawn.
    assert!(
        differing(light, dark) > 2_000,
        "light and dark differ: {}",
        differing(light, dark)
    );
}
