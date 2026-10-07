//! Loading through the application's fetcher: a fake server answers on a
//! thread of its own, with a redirect, a style sheet, a failure and a file to
//! download. The fetcher and downloader are process-wide, so this binary holds
//! one test.

mod common;

use std::sync::{Arc, Mutex};

use common::{builder, rgb, run};
use xui_blitz::{
    BlitzViewEvent, DownloadInfo, DownloadSink, Downloader, FetchRequest, FetchResponder, Fetcher,
};
use xui_core::theme::Theme;

/// The fake server.
struct Server;

impl Fetcher for Server {
    fn fetch(&self, request: FetchRequest, responder: FetchResponder) {
        let path = request.url.trim_start_matches("http://test");
        match path {
            "/old" => {
                responder.status(301);
                responder.header("Location", "/page");
                responder.finish();
            }
            "/page" => {
                responder.status(200);
                responder.header("Content-Type", "text/html; charset=utf-8");
                responder.data(
                    b"<title>Served</title><link rel=stylesheet href=style.css>\
                      <a href=/file.zip style='display:block;height:50px'>get</a>",
                );
                responder.finish();
            }
            "/style.css" => {
                responder.status(200);
                responder.header("Content-Type", "text/css");
                responder.data(b"body { margin: 0; background: #008000 }");
                responder.finish();
            }
            "/file.zip" => {
                responder.status(200);
                responder.header("Content-Type", "application/zip");
                responder.header("Content-Disposition", "attachment; filename=\"a:b.zip\"");
                responder.data(&[7; 100_000]);
                responder.data(&[8; 50]);
                responder.finish();
            }
            "/down" => responder.fail("connection refused"),
            _ => {
                responder.status(404);
                responder.finish();
            }
        }
    }
}

#[derive(Default)]
struct Saved {
    started: Vec<DownloadInfo>,
    bytes: usize,
    finished: Option<Result<(), String>>,
}

struct Saver(Arc<Mutex<Saved>>);
struct Sink(Arc<Mutex<Saved>>);

impl Downloader for Saver {
    fn start(&self, info: &DownloadInfo) -> Option<Box<dyn DownloadSink>> {
        self.0.lock().unwrap().started.push(info.clone());
        Some(Box::new(Sink(Arc::clone(&self.0))))
    }
}

impl DownloadSink for Sink {
    fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.0.lock().unwrap().bytes += data.len();
        Ok(())
    }
    fn finish(self: Box<Self>, result: Result<(), String>) {
        self.0.lock().unwrap().finished = Some(result);
    }
}

#[test]
fn pages_style_sheets_failures_and_downloads_go_through_the_host() {
    xui_blitz::set_fetcher(Arc::new(Server));
    let saved = Arc::new(Mutex::new(Saved::default()));
    xui_blitz::set_downloader(Arc::new(Saver(Arc::clone(&saved))));

    let image = run(
        Theme::light(),
        || builder().url("http://test/old").follow_links(true),
        move |p| {
            // The redirect is followed and the style sheet applies.
            p.wait("the served page", |e| {
                e.contains(&BlitzViewEvent::TitleChanged("Served".into()))
            });
            p.wait_loaded();
            assert!(
                p.take_events()
                    .contains(&BlitzViewEvent::UrlChanged("http://test/page".into()))
            );

            // A response to save rather than show becomes a download; the
            // page stays.
            p.stage.click(20, 20);
            p.wait("the download", |e| {
                e.iter()
                    .any(|e| matches!(e, BlitzViewEvent::DownloadFinished { error: None, .. }))
            });
            let events = p.take_events();
            let started = events.iter().find_map(|e| match e {
                BlitzViewEvent::DownloadStarted(info) => Some(info.clone()),
                _ => None,
            });
            let info = started.expect("a DownloadStarted");
            assert_eq!(info.filename, "a_b.zip");
            assert_eq!(info.mime, "application/zip");
            assert!(events.contains(&BlitzViewEvent::DownloadProgress {
                id: info.id,
                received: 100_050
            }));
            let saved = saved.lock().unwrap();
            assert_eq!(saved.bytes, 100_050);
            assert_eq!(saved.finished, Some(Ok(())));
        },
    );
    assert_eq!(rgb(&image, 200, 200), [0, 128, 0]);

    // A failed fetch shows the error page and says why.
    run(
        Theme::light(),
        || builder().url("http://test/down"),
        |p| {
            p.wait("the failure", |e| {
                e.contains(&BlitzViewEvent::FetchFailed {
                    url: "http://test/down".into(),
                    message: "connection refused".into(),
                })
            });
            p.wait("the error page", |e| {
                e.contains(&BlitzViewEvent::TitleChanged("Problem loading page".into()))
            });
        },
    );
}
