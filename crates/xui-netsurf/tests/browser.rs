//! What a browser around the view needs from it: the status line text, links
//! NetSurf hands back (`mailto:`), downloads (one NetSurf starts for a type it
//! cannot show, one the application asks for, one cancelled) and stopping a
//! load. A fake in-process [`Fetcher`] serves the pages; each test drives an
//! offscreen `NetSurfView` and judges its events.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_netsurf::{
    DownloadInfo, DownloadSink, Downloader, FetchRequest, FetchResponder, Fetcher, NetSurfView,
    NetSurfViewEvent, set_downloader, set_fetcher,
};

/// How long a test may take before it fails instead of hanging.
const WATCHDOG: Duration = Duration::from_secs(30);

/// A link across the top, a `mailto:` link below it.
const LINKS: &str = r#"<!DOCTYPE html>
<html><head><title>Links</title><style>
body { margin: 0 } a { position: absolute; left: 0; display: block;
width: 200px; height: 30px }
</style></head><body>
<a href="/next" style="top: 0">next</a>
<a href="mailto:someone@example.test" style="top: 60px">mail</a>
<a href="/archive.zip" style="top: 120px">archive</a>
</body></html>"#;

/// The bytes of the archive the fake server serves.
fn archive() -> Vec<u8> {
    (0..200_000u32).map(|i| (i % 251) as u8).collect()
}

struct FakeServer;

fn send(responder: FetchResponder, headers: &[(&str, &str)], body: &[u8]) {
    responder.status(200);
    for (name, value) in headers {
        responder.header(name, value);
    }
    for chunk in body.chunks(4096) {
        responder.data(chunk);
    }
    responder.finish();
}

impl Fetcher for FakeServer {
    fn fetch(&self, request: FetchRequest, responder: FetchResponder) {
        let html = ("Content-Type", "text/html; charset=utf-8");
        match request.url.as_str() {
            "http://example.test/" => send(responder, &[html], LINKS.as_bytes()),
            "http://example.test/archive.zip" => send(
                responder,
                &[
                    ("Content-Type", "application/zip"),
                    ("Content-Disposition", "attachment; filename=\"files.zip\""),
                ],
                &archive(),
            ),
            "http://example.test/page.html" => send(responder, &[html], b"<p>a page</p>"),
            // Sends the window elsewhere with no click.
            "http://example.test/refresh" => send(
                responder,
                &[html],
                b"<meta http-equiv=\"refresh\" content=\"0; url=mailto:auto@example.test\"><p>wait</p>",
            ),
            // Never finishes: for Stop and Cancel.
            "http://example.test/slow" | "http://example.test/slow.bin" => {
                let kind = if request.url.ends_with(".bin") {
                    "application/octet-stream"
                } else {
                    "text/html"
                };
                responder.status(200);
                responder.header("Content-Type", kind);
                responder.data(&[b' '; 1024]);
                while !responder.is_aborted() {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            _ => send(responder, &[html], b"<p>elsewhere</p>"),
        }
    }
}

/// A download as the test's downloader saw it.
#[derive(Debug, Default, Clone)]
struct Saved {
    info: Option<DownloadInfo>,
    bytes: Vec<u8>,
    result: Option<Result<(), String>>,
}

/// Every download, in the order they started.
static SAVED: Mutex<Vec<Arc<Mutex<Saved>>>> = Mutex::new(Vec::new());

struct Sink(Arc<Mutex<Saved>>);

impl DownloadSink for Sink {
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.0.lock().unwrap().bytes.extend_from_slice(data);
        Ok(())
    }

    fn finish(self: Box<Self>, result: Result<(), String>) {
        self.0.lock().unwrap().result = Some(result);
    }
}

struct Keeper;

impl Downloader for Keeper {
    fn start(&self, info: &DownloadInfo) -> Option<Box<dyn DownloadSink>> {
        let saved = Arc::new(Mutex::new(Saved {
            info: Some(info.clone()),
            ..Saved::default()
        }));
        SAVED.lock().unwrap().push(Arc::clone(&saved));
        Some(Box::new(Sink(saved)))
    }
}

fn install() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        set_fetcher(Arc::new(FakeServer));
        set_downloader(Arc::new(Keeper));
    });
}

/// The download whose URL is `url`, once it started.
fn saved(url: &str) -> Option<Saved> {
    SAVED.lock().unwrap().iter().find_map(|s| {
        let s = s.lock().unwrap();
        (s.info.as_ref()?.url == url).then(|| s.clone())
    })
}

enum Msg {
    News,
    Download(&'static str),
    CancelLast,
    Stop,
}

struct Page {
    view: NetSurfView<Msg>,
    events: Rc<RefCell<Vec<NetSurfViewEvent>>>,
}

impl App for Page {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _ui: &mut Ui<Msg>) {
        match msg {
            Msg::News => {}
            Msg::Download(url) => self.view.download(url),
            Msg::Stop => self.view.stop(),
            Msg::CancelLast => {
                let last = self.events.borrow().iter().rev().find_map(|e| match e {
                    NetSurfViewEvent::DownloadStarted(info) => Some(info.id),
                    _ => None,
                });
                if let Some(id) = last {
                    self.view.cancel_download(id);
                }
            }
        }
        let events = self.view.update();
        self.events.borrow_mut().extend(events);
    }
}

/// Opens `url` offscreen and runs `step` until it returns true; the events.
/// One test's browser at a time: a launch nobody clicked for names no
/// window, so the engine sends it to the newest one, which must be ours.
static SERIAL: Mutex<()> = Mutex::new(());

fn run(
    url: &'static str,
    step: impl Fn(&Stage<'_, Msg>, &[NetSurfViewEvent]) -> bool + 'static,
) -> Vec<NetSurfViewEvent> {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    install();
    let events = Rc::new(RefCell::new(Vec::new()));
    let done = Rc::new(RefCell::new(false));
    {
        let (events, done) = (Rc::clone(&events), Rc::clone(&done));
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
                    let _ = stage.ui().capture();
                    stage.emit(Msg::News);
                    let seen = watched.borrow().clone();
                    if step(stage, &seen) {
                        *done.borrow_mut() = true;
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            },
        )
        .expect("render offscreen");
    }
    let events = events.borrow().clone();
    assert!(*done.borrow(), "{url} did not settle; events: {events:?}");
    events
}

fn loaded(events: &[NetSurfViewEvent]) -> bool {
    let title = events
        .iter()
        .any(|e| matches!(e, NetSurfViewEvent::TitleChanged(t) if t == "Links"));
    let idle = events.iter().rev().find_map(|e| match e {
        NetSurfViewEvent::LoadingChanged(on) => Some(!on),
        _ => None,
    });
    title && idle == Some(true)
}

fn finished(events: &[NetSurfViewEvent]) -> Option<Option<String>> {
    events.iter().find_map(|e| match e {
        NetSurfViewEvent::DownloadFinished { error, .. } => Some(error.clone()),
        _ => None,
    })
}

#[test]
fn hovering_a_link_shows_its_url_in_the_status() {
    let events = run("http://example.test/", |stage, events| {
        if loaded(events) {
            stage.hover(20, 10);
        }
        events.iter().any(
            |e| matches!(e, NetSurfViewEvent::StatusChanged(t) if t.contains("example.test/next")),
        )
    });
    // The load's own progress goes through the same event.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, NetSurfViewEvent::StatusChanged(t) if !t.is_empty())),
        "{events:?}"
    );
}

#[test]
fn a_mailto_link_is_handed_back() {
    let events = run("http://example.test/", |stage, events| {
        let launched = events
            .iter()
            .any(|e| matches!(e, NetSurfViewEvent::LaunchUrl { .. }));
        if loaded(events) && !launched {
            stage.click(20, 70);
        }
        launched
    });
    assert_eq!(
        launches(&events),
        [("mailto:someone@example.test".to_string(), true)]
    );
}

#[test]
fn a_launch_no_one_clicked_for_is_marked() {
    let events = run("http://example.test/refresh", |_, events| {
        events
            .iter()
            .any(|e| matches!(e, NetSurfViewEvent::LaunchUrl { .. }))
    });
    assert_eq!(
        launches(&events),
        [("mailto:auto@example.test".to_string(), false)]
    );
}

/// Every `LaunchUrl` (URL, by the user).
fn launches(events: &[NetSurfViewEvent]) -> Vec<(String, bool)> {
    events
        .iter()
        .filter_map(|e| match e {
            NetSurfViewEvent::LaunchUrl { url, by_user } => Some((url.clone(), *by_user)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_type_netsurf_cannot_show_is_downloaded() {
    let events = run("http://example.test/", |stage, events| {
        let started = events
            .iter()
            .any(|e| matches!(e, NetSurfViewEvent::DownloadStarted(_)));
        if loaded(events) && !started {
            stage.click(20, 130);
        }
        finished(events).is_some()
    });
    assert_eq!(finished(&events), Some(None), "{events:?}");
    let saved = saved("http://example.test/archive.zip").expect("the download started");
    let info = saved.info.as_ref().unwrap();
    assert_eq!(info.filename, "files.zip");
    assert_eq!(info.mime, "application/zip");
    assert_eq!(saved.result, Some(Ok(())));
    assert!(
        saved.bytes == archive(),
        "{} bytes saved",
        saved.bytes.len()
    );
    let progress = events.iter().rev().find_map(|e| match e {
        NetSurfViewEvent::DownloadProgress { received, .. } => Some(*received),
        _ => None,
    });
    assert_eq!(progress, Some(archive().len() as u64));
    // The page stays: the address goes back to it once the download starts.
    let last_url = events.iter().rev().find_map(|e| match e {
        NetSurfViewEvent::UrlChanged(url) => Some(url.as_str()),
        _ => None,
    });
    assert_eq!(last_url, Some("http://example.test/"), "{events:?}");
}

#[test]
fn the_application_can_download_a_page_and_cancel_a_download() {
    // Each request goes out once: the step runs many times per state.
    let asked = std::cell::Cell::new(None);
    let events = run("http://example.test/", move |stage, events| {
        let starts = events
            .iter()
            .filter(|e| matches!(e, NetSurfViewEvent::DownloadStarted(_)))
            .count();
        let ends = events
            .iter()
            .filter(|e| matches!(e, NetSurfViewEvent::DownloadFinished { .. }))
            .count();
        let next = match (loaded(events), starts, ends) {
            (true, 0, 0) => Some(Msg::Download("http://example.test/page.html")),
            (true, 1, 1) => Some(Msg::Download("http://example.test/slow.bin")),
            (true, 2, 1) => Some(Msg::CancelLast),
            _ => None,
        };
        if let Some(msg) = next
            && asked.get() != Some((starts, ends))
        {
            asked.set(Some((starts, ends)));
            stage.emit(msg);
        }
        ends == 2
    });
    let page = saved("http://example.test/page.html").expect("page saved");
    assert_eq!(page.bytes, b"<p>a page</p>");
    assert_eq!(page.result, Some(Ok(())));
    let slow = saved("http://example.test/slow.bin").expect("slow download started");
    assert_eq!(slow.result, Some(Err("Cancelled".to_string())));
    assert!(events.iter().any(|e| matches!(
        e,
        NetSurfViewEvent::DownloadFinished { error: Some(why), .. } if why == "Cancelled"
    )));
}

#[test]
fn stop_ends_a_load() {
    let events = run("http://example.test/slow", |stage, events| {
        let loading = events.iter().rev().find_map(|e| match e {
            NetSurfViewEvent::LoadingChanged(on) => Some(*on),
            _ => None,
        });
        if loading == Some(true) {
            stage.emit(Msg::Stop);
        }
        loading == Some(false)
    });
    assert!(events.contains(&NetSurfViewEvent::LoadingChanged(false)));
}
