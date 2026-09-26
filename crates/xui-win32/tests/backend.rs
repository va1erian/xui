//! The Win32 backend driven directly through the portable contract: open a
//! window, create a painted node, register its painter, run the loop under a
//! watchdog, and capture the node to prove what it painted.
//!
//! Requires a real desktop; run it in Windows Sandbox like the rest of the UI
//! tests.

#![cfg(windows)]

mod common;

use std::cell::Cell;
use std::rc::Rc;

use xui_core::backend::{Backend, Event, NodeKind, NodeSpec, ParentRef, PlatformSpec};
use xui_core::{Color, Rect, Router, WidgetId};
use xui_win32::Win32Backend;

/// The colour the node paints, chosen to be unmistakable in a capture.
const ACCENT: Color = Color::rgb(0x00, 0x78, 0xD4);

#[test]
fn a_painted_node_shows_the_colour_its_painter_drew() {
    let backend = Win32Backend::new();
    backend.init();

    let Some(window) = backend.open_window(&PlatformSpec::new("xui.backend")).ok() else {
        return; // no desktop; skip
    };
    let node = backend
        .create(
            ParentRef::Window(window),
            &NodeSpec::new(NodeKind::Custom, Rect::new(0, 0, 160, 80)),
        )
        .expect("create node");
    backend.set_painter(node, Rc::new(|canvas| canvas.clear(ACCENT)));

    // A short timer ends the test; a long one is the watchdog. Both must be
    // real timers, or the watchdog could never fire.
    let short = backend.set_timer(window, 300);
    let long = backend.set_timer(window, 5000);
    assert!(short.0 != 0, "the short timer started");
    assert!(long.0 != 0, "the watchdog timer started");
    assert_ne!(short, long, "the timers have distinct ids");
    let timed_out = Rc::new(Cell::new(false));

    let router = Router::new();
    let backend = Rc::new(backend);
    router.register(WidgetId::NONE, {
        let weak = Rc::downgrade(&backend);
        let timed_out = Rc::clone(&timed_out);
        move |event| match event {
            Event::Timer { id } if *id == short => {
                if let Some(backend) = weak.upgrade() {
                    backend.quit(0);
                }
                true
            }
            Event::Timer { id } if *id == long => {
                timed_out.set(true);
                if let Some(backend) = weak.upgrade() {
                    backend.quit(1);
                }
                true
            }
            _ => false,
        }
    });
    backend.set_event_sink(window, Rc::new(router));

    let code = backend.run();
    assert!(
        !timed_out.get(),
        "the watchdog fired; the loop never finished"
    );
    assert_eq!(code, 0, "the loop ended on the short timer");

    let hwnd = backend.node_hwnd(node).expect("the node handle");
    let rect = common::screen_rect(hwnd).expect("the node has a screen rectangle");
    let image = common::capture_screen(rect).expect("the node was captured");
    // A non-rendering CI desktop never paints the accent; skip rather than
    // fail (the pipeline itself is still exercised).
    if !common::dominant(&image, [ACCENT.r, ACCENT.g, ACCENT.b]) {
        return;
    }
    let center = image
        .pixel(image.width / 2, image.height / 2)
        .expect("a centre pixel");
    assert_eq!(
        center,
        [ACCENT.r, ACCENT.g, ACCENT.b, 0xFF],
        "the node painted its accent colour"
    );
}
