//! A ListView's scrollbar thumb must track its scroll offset even when the list
//! was placed (and thus resized) by a parent through `apply_moves` after the
//! model was installed (va1erian/xui#158, via emusic#466).
//!
//! The list is built at `Rect::default()` and then placed by the layout that
//! owns it. Rendering the window and reading the scrollbar
//! pixel rows catches the thumb geometry the real backend would present.

use std::cell::RefCell;
use std::rc::Rc;

use xui_canvas::{OffscreenBackend, RgbaImage};
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, Event, PlatformSpec};
use xui_core::message::Modifiers;
use xui_core::prelude::{LayoutExt, column, list};
use xui_core::widget::Fill;
use xui_core::{Dip, Theme};

struct TestApp;

impl App for TestApp {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// The top device row painted with the scrollbar thumb colour, or `None`.
fn thumb_top(image: &RgbaImage, thumb: (u8, u8, u8)) -> Option<i32> {
    let width = image.width as i32;
    for (index, pixel) in image.pixels.as_chunks::<4>().0.iter().enumerate() {
        let row = (index as i32) / width;
        if (pixel[0], pixel[1], pixel[2]) == thumb {
            return Some(row);
        }
    }
    None
}

#[test]
fn a_parent_placed_list_keeps_its_thumb_tracking_the_offset() {
    let backend = Rc::new(OffscreenBackend::new());
    let seen: Rc<RefCell<Vec<Option<i32>>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_for_make = Rc::clone(&seen);
    let backend_for_make = Rc::clone(&backend);
    let run: Rc<dyn Backend> = backend.clone();
    let thumb = Theme::light().scrollbar;
    let thumb = (thumb.r, thumb.g, thumb.b);

    run_app(
        run,
        PlatformSpec::new("scrollbar").size(Dip(240.0), Dip(200.0)),
        move |ui| {
            let rows: Vec<String> = (0..400).map(|index| format!("row {index}")).collect();
            // Built with an empty default rect and its model, then placed by
            // the layout that owns it.
            ui.root(
                column().child(
                    list()
                        .column("Name", Fill)
                        .then(move |list| {
                            list.set_model(rows);
                            list
                        })
                        .fill(1),
                ),
            )
            .unwrap();
            let window = ui.window();

            let before = backend_for_make.render(window).expect("a render");
            seen_for_make.borrow_mut().push(thumb_top(&before, thumb));

            backend_for_make.inject(
                window,
                Event::MouseWheel {
                    delta: -20,
                    horizontal: false,
                    x: 60,
                    y: 60,
                    modifiers: Modifiers::NONE,
                },
            );

            let after = backend_for_make.render(window).expect("a render");
            seen_for_make.borrow_mut().push(thumb_top(&after, thumb));
            TestApp
        },
    )
    .expect("the offscreen app ran");

    let seen = seen.borrow();
    let before = seen[0].expect("a thumb before scrolling");
    let after = seen[1].expect("a thumb after scrolling");
    assert!(
        after > before,
        "the thumb follows the scrolled offset: {before} -> {after}"
    );
}
