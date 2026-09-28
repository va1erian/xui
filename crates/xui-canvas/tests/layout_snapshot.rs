//! An offscreen light/dark snapshot of a form built with `xui_core::arrange`,
//! rendered by the painted backend.
//!
//! The PNGs (`target/ui/layout-{light,dark}.png`) are the visual check; the
//! assertions catch a layout that overlaps, escapes the window, or paints
//! nothing.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::{OffscreenBackend, RgbaImage};
use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{LayoutExt, Mounted, column, row, spacer};
use xui_core::backend::{Backend, PlatformSpec, WidgetId};
use xui_core::widget::{Button, CheckBox, Edit, Label, ProgressBar, Slider};
use xui_core::{Dip, Insets, Rect, Theme, dip};

struct Form {
    _mounted: Mounted<()>,
}

impl App for Form {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The widgets' ids in tree order, so the test can inspect where they landed.
fn build(ui: &Ui<()>) -> (Form, Vec<WidgetId>) {
    let title = Rc::new(Label::auto(ui, "Layout").unwrap());
    let name = Rc::new(Edit::auto(ui, "Ada").unwrap());
    let loud = Rc::new(CheckBox::auto(ui, "Loud").unwrap());
    let volume = Rc::new(Slider::auto(ui, 0.0, 100.0).unwrap());
    let level = Rc::new(ProgressBar::auto(ui, 100).unwrap());
    let reset = Rc::new(Button::auto(ui, "Reset").unwrap());
    let quit = Rc::new(Button::auto(ui, "Quit").unwrap());
    level.set_value(60);

    let root = column()
        .margins(Insets::all(dip(16.0)))
        .spacing(dip(8.0))
        .child(&title)
        .child(&name)
        .child(&loud)
        .child(&volume)
        .child(&level)
        .child(spacer())
        .child(
            row()
                .spacing(dip(8.0))
                .child(spacer())
                .child(&reset)
                .child(&quit)
                .height(dip(28.0)),
        );
    let mounted = ui.mount(root).unwrap();
    let ids = vec![
        title.id(),
        name.id(),
        loud.id(),
        volume.id(),
        level.id(),
        reset.id(),
        quit.id(),
    ];
    (Form { _mounted: mounted }, ids)
}

fn save(name: &str, image: &RgbaImage) {
    let dir = std::path::Path::new("target/ui");
    let _ = std::fs::create_dir_all(dir);
    let file = std::fs::File::create(dir.join(name)).expect("create snapshot");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(&image.pixels)
        .expect("png data");
}

#[test]
fn a_mounted_form_lays_out_and_renders_light_and_dark() {
    let backend = Rc::new(OffscreenBackend::new());
    let seen: Rc<RefCell<Vec<(Rect, Rect)>>> = Rc::new(RefCell::new(Vec::new()));
    let images: Rc<RefCell<Vec<RgbaImage>>> = Rc::new(RefCell::new(Vec::new()));

    let (backend_for_make, seen_for_make, images_for_make) =
        (Rc::clone(&backend), Rc::clone(&seen), Rc::clone(&images));
    let run: Rc<dyn Backend> = backend.clone();
    run_app(
        run,
        PlatformSpec::new("layout").size(Dip(420.0), Dip(280.0)),
        move |ui| {
            let (form, ids) = build(ui);
            let client = ui.client_rect();
            let mut seen = seen_for_make.borrow_mut();
            for id in ids {
                seen.push((client, ui.bounds(id)));
            }

            let light = backend_for_make
                .render(ui.window())
                .expect("a light render");
            save("layout-light.png", &light);
            ui.set_theme(Theme::dark());
            let dark = backend_for_make.render(ui.window()).expect("a dark render");
            save("layout-dark.png", &dark);
            let mut images = images_for_make.borrow_mut();
            images.push(light);
            images.push(dark);
            form
        },
    )
    .expect("the offscreen form ran");

    let seen = seen.borrow();
    assert_eq!(seen.len(), 7);
    let mut previous_bottom = 0;
    for (index, (client, bounds)) in seen.iter().enumerate().take(5) {
        assert!(
            bounds.width() > 0 && bounds.height() > 0,
            "widget {index} has a size"
        );
        assert!(
            bounds.left >= client.left && bounds.right <= client.right,
            "widget {index} stays inside the window: {bounds:?} in {client:?}"
        );
        assert!(
            bounds.top >= previous_bottom,
            "widget {index} does not overlap the one above: {bounds:?}"
        );
        previous_bottom = bounds.bottom;
    }
    // The button row sits at the bottom, right-aligned by the spacer.
    let (client, quit) = seen[6];
    assert!(quit.bottom <= client.bottom - 16 && quit.right <= client.right - 16);
    assert!(
        quit.top >= previous_bottom,
        "the buttons are below the form"
    );
    assert!(seen[5].1.right <= quit.left, "Reset is left of Quit");

    let images = images.borrow();
    assert_ne!(images[0].pixels, images[1].pixels, "the themes differ");
}
