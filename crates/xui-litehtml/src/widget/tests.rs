//! The whole widget at a non-zero origin, as the portable software backend
//! places it: the page, the selection and the scrollbar paint inside the
//! view's bounds, and node-local pointer events hit what was painted there.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use xui_canvas::Surface;
use xui_core::backend::Event;
use xui_core::geometry::Rect as PxRect;
use xui_core::message::{Modifiers, MouseButton};
use xui_core::theme::Theme;

use super::HtmlWidget;
use crate::text::TextSystem;
use crate::view::HtmlViewEvent;
use crate::worker::Worker;

/// Where the view sits in a 1000 x 600 window (the mail reader pane that
/// showed only its background).
const VIEW: PxRect = PxRect::new(520, 36, 1000, 596);

/// A 40 px red block, then a link, on a blue page tall enough to scroll.
const PAGE: &str = r#"<!doctype html>
<body style="margin:0;background:#0000ff">
<div style="height:40px;background:#ff0000"></div>
<a href="https://example.com/x" style="display:block;height:30px;font-size:20px">link</a>
<div style="height:2000px"></div>
</body>"#;

/// A widget with its render worker on its own thread, as `HtmlView` starts it.
fn widget(html: &str) -> HtmlWidget {
    let text = TextSystem::for_tests();
    let (job_tx, job_rx) = mpsc::channel();
    let (out_tx, out_rx) = mpsc::channel();
    let latest_id = Arc::new(AtomicU64::new(0));
    let widget = HtmlWidget::new(
        text.clone(),
        job_tx,
        out_rx,
        Arc::clone(&latest_id),
        html.to_string(),
        1.0,
    );
    std::thread::spawn(move || {
        let worker = Worker::new(
            text,
            out_tx,
            latest_id,
            Arc::new(|| {}),
            Arc::new(Mutex::new(None)),
        );
        worker.run(job_rx);
    });
    widget
}

/// Paints `widget` at [`VIEW`] into a fresh window surface.
fn paint(widget: &HtmlWidget) -> Surface {
    let mut surface = Surface::new(1000, 600);
    surface.with_canvas_at(VIEW, 96, |canvas| widget.paint(canvas, Theme::light()));
    surface
}

/// Paints until the worker's frame has arrived, then returns that paint.
fn rendered(widget: &HtmlWidget) -> Surface {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let surface = paint(widget);
        if widget.is_ready() {
            return surface;
        }
        assert!(Instant::now() < deadline, "the worker never rendered");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn pixel(surface: &Surface, x: i32, y: i32) -> [u8; 4] {
    surface
        .to_image()
        .pixel(x as u32, y as u32)
        .expect("in the surface")
}

fn press(x: i32, y: i32) -> Event {
    Event::MouseDown {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

fn release(x: i32, y: i32) -> Event {
    Event::MouseUp {
        x,
        y,
        button: MouseButton::Left,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn the_page_paints_inside_the_view() {
    let widget = widget(PAGE);
    let surface = rendered(&widget);
    // The red block is at the view's top-left, the blue page below it.
    assert_eq!(
        pixel(&surface, VIEW.left + 5, VIEW.top + 5),
        [255, 0, 0, 255]
    );
    assert_eq!(
        pixel(&surface, VIEW.left + 200, VIEW.top + 39),
        [255, 0, 0, 255]
    );
    assert_eq!(
        pixel(&surface, VIEW.left + 300, VIEW.top + 300),
        [0, 0, 255, 255]
    );
    // Nothing of the page lands at the window's origin.
    assert_eq!(pixel(&surface, 5, 5), [0, 0, 0, 0]);
    assert_eq!(pixel(&surface, VIEW.left - 1, VIEW.top + 5), [0, 0, 0, 0]);
}

#[test]
fn a_node_local_click_on_the_link_reports_it() {
    let widget = widget(PAGE);
    rendered(&widget);
    // Events are node-local: the link is 40..70 px down the view.
    widget.handle_input(&press(10, 55));
    let effects = widget.handle_input(&release(10, 55));
    match effects.emit {
        Some(HtmlViewEvent::LinkClicked(href)) => assert_eq!(href, "https://example.com/x"),
        _ => panic!("the click on the link was not reported"),
    }
    // The red block above it is not a link.
    widget.handle_input(&press(10, 20));
    assert!(widget.handle_input(&release(10, 20)).emit.is_none());
}

#[test]
fn the_selection_highlight_lands_on_the_text() {
    let widget = widget(PAGE);
    let before = rendered(&widget);
    widget.select_all();
    let after = paint(&widget);
    // The highlight changes pixels on the link's line inside the view, and
    // none outside it.
    let changed = |x0: i32, y0: i32, x1: i32, y1: i32| {
        (y0..y1).any(|y| (x0..x1).any(|x| pixel(&before, x, y) != pixel(&after, x, y)))
    };
    assert!(changed(
        VIEW.left,
        VIEW.top + 40,
        VIEW.left + 100,
        VIEW.top + 70
    ));
    assert!(!changed(0, 0, VIEW.left, 120));
}

#[test]
fn the_scrollbar_is_painted_and_hit_at_the_view_edge() {
    let widget = widget(PAGE);
    let before = rendered(&widget);
    let width = VIEW.width();
    // The thumb is at the top of the track, along the view's right edge: a
    // node-local press there grabs it, and a drag scrolls the page.
    let bar_x = width - 3;
    assert!(widget.on_bar(bar_x, 5));
    assert!(!widget.on_bar(width / 2, 5));
    assert_ne!(
        pixel(&before, VIEW.left + bar_x, VIEW.top + 5),
        [0, 0, 255, 255],
        "the bar is painted at the view's right edge"
    );
    let grabbed = widget.handle_input(&press(bar_x, 5));
    assert!(grabbed.capture, "the press grabbed the thumb");
    widget.handle_input(&Event::MouseMove {
        x: bar_x,
        y: 105,
        modifiers: Modifiers::NONE,
    });
    widget.handle_input(&release(bar_x, 105));
    let after = paint(&widget);
    // Scrolled past the red block: blue at the view's top.
    assert_eq!(pixel(&after, VIEW.left + 5, VIEW.top + 5), [0, 0, 255, 255]);
}
