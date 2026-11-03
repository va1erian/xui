//! The host fetcher end to end: a fake in-process [`Fetcher`] serves a page
//! with a PNG, a JPEG and a GIF (one behind a redirect) and a missing image,
//! a page that fails, and a form; each is loaded into an offscreen
//! `NetSurfView` and judged by its events and its pixels.

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::image::Image;
use xui_netsurf::{
    FetchMethod, FetchRequest, FetchResponder, Fetcher, NetSurfView, NetSurfViewEvent, set_fetcher,
};

/// How long a page may take before the test fails instead of hanging.
const WATCHDOG: Duration = Duration::from_secs(30);

const PAGE: &str = r#"<!DOCTYPE html>
<html><head><title>Fetch test</title>
<style>body { margin: 0 } img { position: absolute; left: 0; display: block }</style>
</head><body>
<img src="red.png" style="top: 0">
<img src="/green.jpg" style="top: 40px">
<img src="http://example.test/moved/blue.gif" style="top: 80px">
<img src="missing.png" style="top: 120px">
</body></html>"#;

const FORM: &str = r#"<!DOCTYPE html>
<html><head><title>Form</title><style>body { margin: 0 }</style></head><body>
<form method="post" action="/submit">
<input type="hidden" name="q" value="hello world">
<input type="submit" value="Go" style="position: absolute; left: 0; top: 0; width: 120px; height: 40px">
</form></body></html>"#;

/// Requests the fake server has seen.
static SEEN: Mutex<Vec<FetchRequest>> = Mutex::new(Vec::new());

fn encode(image: image::DynamicImage, format: image::ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, format)
        .expect("encode a fixture");
    bytes.into_inner()
}

fn solid(width: u32, height: u32, rgb: [u8; 3]) -> image::DynamicImage {
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(width, height, image::Rgb(rgb)))
}

/// A fake server: answers from fixed routes, one thread per request.
struct FakeServer;

impl FakeServer {
    fn send(responder: FetchResponder, status: u16, headers: &[(&str, &str)], body: &[u8]) {
        responder.status(status);
        for (name, value) in headers {
            responder.header(name, value);
        }
        // In pieces, as a socket would deliver it.
        for chunk in body.chunks(100) {
            responder.data(chunk);
        }
        responder.finish();
    }
}

impl Fetcher for FakeServer {
    fn fetch(&self, request: FetchRequest, responder: FetchResponder) {
        SEEN.lock().unwrap().push(request.clone());
        let html = [("Content-Type", "text/html; charset=utf-8")];
        match request.url.as_str() {
            "http://example.test/start" => {
                Self::send(responder, 301, &[("Location", "/")], b"moved")
            }
            "http://example.test/" => {
                let headers = [html[0], ("Set-Cookie", "session=abc123; Path=/")];
                Self::send(responder, 200, &headers, PAGE.as_bytes());
            }
            "http://example.test/red.png" => {
                let png = encode(solid(40, 20, [255, 0, 0]), image::ImageFormat::Png);
                Self::send(responder, 200, &[("Content-Type", "image/png")], &png);
            }
            "http://example.test/green.jpg" => {
                let jpeg = encode(solid(32, 16, [0, 255, 0]), image::ImageFormat::Jpeg);
                Self::send(responder, 200, &[("Content-Type", "image/jpeg")], &jpeg);
            }
            "http://example.test/moved/blue.gif" => Self::send(
                responder,
                302,
                &[("Location", "../blue.gif"), ("Content-Type", "text/html")],
                b"<a href=../blue.gif>moved</a>",
            ),
            "http://example.test/blue.gif" => {
                let gif = encode(solid(24, 12, [0, 0, 255]), image::ImageFormat::Gif);
                Self::send(responder, 200, &[("Content-Type", "image/gif")], &gif);
            }
            "http://example.test/form" => Self::send(responder, 200, &html, FORM.as_bytes()),
            "http://example.test/submit" => {
                let page = b"<html><head><title>Posted</title></head><body>ok</body></html>";
                Self::send(responder, 200, &html, page);
            }
            "https://down.test/" => responder.fail("connection refused"),
            // A responder dropped without an answer counts as a failure.
            "http://dropped.test/" => drop(responder),
            _ => Self::send(
                responder,
                404,
                &html,
                b"<html><body>not found</body></html>",
            ),
        }
    }
}

fn install_fetcher() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| set_fetcher(Arc::new(FakeServer)));
}

fn seen(url: &str) -> Vec<FetchRequest> {
    SEEN.lock()
        .unwrap()
        .iter()
        .filter(|r| r.url == url)
        .cloned()
        .collect()
}

enum Msg {
    News,
}

struct Page {
    view: NetSurfView<Msg>,
    events: Rc<RefCell<Vec<NetSurfViewEvent>>>,
}

impl App for Page {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, _ui: &mut Ui<Msg>) {
        let events = self.view.update();
        self.events.borrow_mut().extend(events);
    }
}

/// What a load ended with.
struct Loaded {
    events: Vec<NetSurfViewEvent>,
    image: Image,
}

/// Opens `url` in a 400x300 offscreen view, runs `step` against the stage
/// until it returns true (or the watchdog fires), and returns the events and
/// the final pixels.
fn load(
    url: &'static str,
    step: impl Fn(&Stage<'_, Msg>, &[NetSurfViewEvent], bool) -> bool + 'static,
) -> Loaded {
    install_fetcher();
    let events = Rc::new(RefCell::new(Vec::new()));
    let ready = Rc::new(RefCell::new(false));
    let image = {
        let (events, ready) = (Rc::clone(&events), Rc::clone(&ready));
        let watched = Rc::clone(&events);
        render_with(
            Snapshot::new(Dip(400.0), Dip(300.0)).dpi(96),
            move |ui| {
                let view = NetSurfView::new(ui, ui.client_rect(), url, || Msg::News)?;
                Ok(Page { view, events })
            },
            move |stage| {
                let start = Instant::now();
                while start.elapsed() < WATCHDOG {
                    // A paint sends the view's size; the message drains news.
                    let _ = stage.ui().capture();
                    stage.emit(Msg::News);
                    let done = step(stage, &watched.borrow(), false);
                    if done {
                        // One more round so the newest frame is on screen.
                        std::thread::sleep(Duration::from_millis(200));
                        stage.emit(Msg::News);
                        let _ = stage.ui().capture();
                        *ready.borrow_mut() = step(stage, &watched.borrow(), true);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            },
        )
        .expect("render offscreen")
    };
    let events = events.borrow().clone();
    assert!(*ready.borrow(), "{url} did not settle; events: {events:?}");
    Loaded { events, image }
}

/// Whether the last load-state event says the page finished.
fn finished(events: &[NetSurfViewEvent]) -> bool {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            NetSurfViewEvent::LoadingChanged(on) => Some(!on),
            _ => None,
        })
        .unwrap_or(false)
}

fn has_title(events: &[NetSurfViewEvent], title: &str) -> bool {
    events
        .iter()
        .any(|e| matches!(e, NetSurfViewEvent::TitleChanged(t) if t == title))
}

fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
    image.pixel(x, y).expect("a pixel inside the snapshot")
}

fn close(px: [u8; 4], rgb: [u8; 3]) -> bool {
    px[..3].iter().zip(rgb).all(|(&a, b)| a.abs_diff(b) <= 40)
}

#[test]
fn a_page_with_images_through_a_redirect() {
    let loaded = load("http://example.test/start", |_, events, _| {
        has_title(events, "Fetch test")
            && finished(events)
            && !seen("http://example.test/blue.gif").is_empty()
    });
    let events = &loaded.events;
    assert!(
        events
            .iter()
            .any(|e| *e == NetSurfViewEvent::UrlChanged("http://example.test/".into())),
        "the page redirect was not followed: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, NetSurfViewEvent::FetchFailed { .. })),
        "a 404 is a page, not a failure: {events:?}"
    );

    let image = &loaded.image;
    let white = [255, 255, 255];
    // Each image at its own size: inside is its colour, just past it is the
    // white page.
    assert!(
        close(pixel(image, 5, 5), [255, 0, 0]),
        "PNG: {:?}",
        pixel(image, 5, 5)
    );
    assert!(close(pixel(image, 38, 18), [255, 0, 0]));
    assert!(
        close(pixel(image, 42, 5), white),
        "PNG width: {:?}",
        pixel(image, 42, 5)
    );
    assert!(close(pixel(image, 5, 22), white), "PNG height");
    assert!(
        close(pixel(image, 5, 45), [0, 255, 0]),
        "JPEG: {:?}",
        pixel(image, 5, 45)
    );
    assert!(close(pixel(image, 30, 54), [0, 255, 0]));
    assert!(close(pixel(image, 34, 45), white), "JPEG width");
    assert!(
        close(pixel(image, 5, 85), [0, 0, 255]),
        "GIF: {:?}",
        pixel(image, 5, 85)
    );
    assert!(close(pixel(image, 26, 85), white), "GIF width");

    // What a server sees.
    let page = &seen("http://example.test/")[0];
    assert_eq!(page.method, FetchMethod::Get);
    assert_eq!(page.body, None);
    let header = |name: &str| {
        page.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    };
    assert!(header("User-Agent").is_some_and(|ua| ua.contains("NetSurf")));
    assert!(header("Accept").is_some_and(|a| a.contains("text/html")));
    assert!(!seen("http://example.test/missing.png").is_empty());
    // The cookie the page set goes back with its images.
    let png = &seen("http://example.test/red.png")[0];
    assert!(
        png.headers
            .iter()
            .any(|(n, v)| n == "Cookie" && v.contains("session=abc123")),
        "{:?}",
        png.headers
    );
}

#[test]
fn a_failed_fetch_is_reported() {
    for (url, message) in [
        ("https://down.test/", "connection refused"),
        ("http://dropped.test/", "without an answer"),
    ] {
        let loaded = load(url, move |_, events, _| {
            finished(events)
                && events
                    .iter()
                    .any(|e| matches!(e, NetSurfViewEvent::FetchFailed { .. }))
        });
        let failure = loaded.events.iter().find_map(|e| match e {
            NetSurfViewEvent::FetchFailed { url, message } => Some((url.clone(), message.clone())),
            _ => None,
        });
        let (failed_url, why) = failure.expect("a FetchFailed event");
        assert_eq!(failed_url, url);
        assert!(why.contains(message), "{why}");
    }
}

#[test]
fn a_form_posts_its_fields() {
    let loaded = load("http://example.test/form", |stage, events, last| {
        if last {
            return has_title(events, "Posted");
        }
        if has_title(events, "Form")
            && finished(events)
            && seen("http://example.test/submit").is_empty()
        {
            stage.click(20, 20);
        }
        has_title(events, "Posted") && finished(events)
    });
    assert!(has_title(&loaded.events, "Posted"));
    let post = &seen("http://example.test/submit")[0];
    assert_eq!(post.method, FetchMethod::Post);
    assert_eq!(post.body.as_deref(), Some(&b"q=hello+world"[..]));
    assert!(post.headers.iter().any(|(n, v)| {
        n.eq_ignore_ascii_case("Content-Type") && v == "application/x-www-form-urlencoded"
    }));
}
