//! The engine draws many frames, and navigates between pages with pictures,
//! without its rasteriser panicking. `vello_cpu` panics ("Image ImageId(n) not
//! found in registry") when a frame replays commands that name a picture the
//! image cache has dropped, which happens if the render context keeps the
//! earlier frames' commands (see `Raster::draw`). The engine thread's panic
//! does not fail a test by itself, so a hook counts them.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{Msg, builder, run};
use xui_blitz::BlitzViewEvent;
use xui_core::backend::Event;
use xui_core::message::Modifiers;
use xui_core::theme::Theme;

/// A 1x1 PNG, base64.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

/// A page titled `title` with a PNG, a CSS background picture and an SVG.
fn page(title: &str) -> String {
    format!(
        "<title>{title}</title><body style='margin:0'>\
         <img src='data:image/png;base64,{PNG}' width=100 height=100>\
         <div style=\"height:100px;background:url('data:image/png;base64,{PNG}')\"></div>\
         <img src=\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' \
         width='50' height='50'><rect width='50' height='50' fill='red'/></svg>\">"
    )
}

fn navigate(p: &common::Probe<'_, '_>, url: String, title: &str) {
    p.stage.emit(Msg::Navigate(url));
    p.wait("the next page", |e| {
        e.contains(&BlitzViewEvent::TitleChanged(title.into()))
    });
    p.wait_loaded();
}

static PANICS: AtomicUsize = AtomicUsize::new(0);

#[test]
fn pictures_survive_navigation_and_many_frames() {
    std::panic::set_hook(Box::new(|info| {
        PANICS.fetch_add(1, Ordering::SeqCst);
        eprintln!("{info}");
    }));
    run(
        Theme::light(),
        || builder().html(page("One")).follow_links(true),
        |p| {
            p.wait_loaded();
            // Pages with pictures, one after another.
            for n in 0..8 {
                let title = format!("P{n}");
                navigate(p, format!("data:text/html,{}", page(&title)), &title);
            }
            // Then a page without any, scrolled for many frames: the cache has
            // dropped the pictures by now.
            let tall = "data:text/html,<title>Two</title><div style='height:3000px'>x</div>";
            navigate(p, tall.into(), "Two");
            for i in 0..200 {
                p.stage.inject(Event::MouseWheel {
                    delta: if i % 2 == 0 { -120 } else { 120 },
                    horizontal: false,
                    x: 100,
                    y: 100,
                    modifiers: Modifiers::NONE,
                });
                let _ = p.stage.ui().capture();
                p.stage.emit(Msg::Frame);
                std::thread::sleep(Duration::from_millis(5));
            }
        },
    );
    assert_eq!(PANICS.load(Ordering::SeqCst), 0, "the engine panicked");
}
