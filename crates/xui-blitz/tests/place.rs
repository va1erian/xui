//! A `BlitzView` a layout places: the slot's pixels and its scrollbar.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use common::{Msg, builder, rgb};
use xui_blitz::{BlitzView, BlitzViewEvent};
use xui_canvas::snapshot::{Snapshot, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::arrange::{Handle, LayoutExt, build, column};
use xui_core::geometry::Rect;
use xui_core::theme::Theme;

const BLUE: [u8; 3] = [0, 0, 255];

/// A view a layout built, kept so the app can pump its frames.
struct Placed {
    view: Rc<BlitzView<Msg>>,
    events: Rc<RefCell<Vec<BlitzViewEvent>>>,
}

impl App for Placed {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::Frame => self.view.update(),
            Msg::Event(event) => self.events.borrow_mut().push(event),
            Msg::Navigate(url) => self.view.navigate(&url),
        }
    }
}

#[test]
fn a_layout_places_the_view_and_its_scrollbar() {
    let html = r#"<body style="margin: 0; background: #00f">
        <div style="height: 2000px"></div>"#;
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    let image = render_with(
        Snapshot::new(Dip(400.0), Dip(300.0)).theme(Theme::light()),
        move |ui| {
            let view: Handle<BlitzView<Msg>> = Handle::new();
            ui.root(
                column().padding(40).child(
                    build(move |ui| builder().html(html).build(ui, Rect::default()))
                        .bind(&view)
                        .fill(1),
                ),
            )?;
            Ok(Placed {
                view: view.get(),
                events,
            })
        },
        move |stage| {
            let start = Instant::now();
            let loaded = |events: &[BlitzViewEvent]| {
                events
                    .iter()
                    .rev()
                    .any(|e| matches!(e, BlitzViewEvent::LoadingChanged(false)))
            };
            while !loaded(&seen.borrow()) {
                stage.emit(Msg::Frame);
                let _ = stage.ui().capture();
                assert!(
                    start.elapsed() < Duration::from_secs(30),
                    "timed out waiting for the page"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            for _ in 0..20 {
                stage.emit(Msg::Frame);
                let _ = stage.ui().capture();
                std::thread::sleep(Duration::from_millis(10));
            }
        },
    )
    .expect("render");

    // The view fills the padded slot (40..360 × 40..260), not the window.
    assert_eq!(rgb(&image, 200, 150), BLUE);
    let window = rgb(&image, 10, 10);
    assert_ne!(window, BLUE, "the padding is not the view");
    // Its scrollbar was re-laid along the slot's right edge, where the page
    // does not paint: 400 - 40 - 8 + 4.
    let gutter: Vec<[u8; 3]> = (45..255).map(|y| rgb(&image, 356, y)).collect();
    assert!(
        gutter.iter().any(|px| *px != BLUE && *px != window),
        "no scrollbar in the slot's gutter: {gutter:?}"
    );
}
