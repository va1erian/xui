#![forbid(unsafe_code)]

//! Network fetching through a host-supplied [`Fetcher`].
//!
//! NetSurf's own http client is libcurl, which this crate does not build.
//! Instead the application supplies a [`Fetcher`] with [`set_fetcher`], and
//! NetSurf hands it every `http:` and `https:` request: the page, its style
//! sheets and its images. The fetcher answers through a [`FetchResponder`],
//! from any thread; the answers queue up and NetSurf takes them on its engine
//! thread.
//!
//! The fetcher is a plain transport, as libcurl is to NetSurf:
//!
//! - It must **not** follow redirects. It reports the `3xx` status and the
//!   `Location` header, and NetSurf follows them (resolving relative
//!   locations, turning a `303` into a GET, and so on).
//! - It decodes any `Content-Encoding` it asked for itself and leaves that
//!   header (and a `Content-Length` that no longer matches) out.
//! - It sends the request headers it is given (user agent, `Accept`,
//!   cookies, the form's `Content-Type`); it may add its own, such as
//!   `Accept-Encoding` for what it decodes and `Host`.
//! - It reports `Set-Cookie` headers like any other; NetSurf keeps the
//!   cookies (in memory) and sends them back in later requests.
//! - It applies its own timeouts: NetSurf waits as long as it takes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, PoisonError, RwLock};

use crate::engine::Command;

/// An HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FetchMethod {
    /// `GET`.
    Get,
    /// `HEAD`.
    Head,
    /// `POST`: the request has a body.
    Post,
}

/// A request NetSurf wants made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchRequest {
    /// The absolute `http:` or `https:` URL.
    pub url: String,
    /// The method.
    pub method: FetchMethod,
    /// Headers to send, in order (`User-Agent`, `Accept`, `Cookie`, a form's
    /// `Content-Type`, cache validators...).
    pub headers: Vec<(String, String)>,
    /// The body of a `POST` (an url-encoded or multipart form).
    pub body: Option<Vec<u8>>,
}

/// One piece of a response, queued for the engine thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchEvent {
    Status(u16),
    Header(String, String),
    Data(Vec<u8>),
    Finish,
    Fail(String),
}

/// Where a [`Fetcher`] reports one response. It is `Send + 'static`, so the
/// fetcher can move it to the thread that does the work.
///
/// Report in order: [`status`](Self::status) once, the
/// [`header`](Self::header)s, the body in any number of [`data`](Self::data)
/// pieces, and last [`finish`](Self::finish) or [`fail`](Self::fail).
/// Dropping a responder without either counts as a failure, so a fetcher
/// that panics or forgets does not leave the page loading forever.
pub struct FetchResponder {
    id: u64,
    commands: Sender<Command>,
    aborted: Arc<AtomicBool>,
    ended: bool,
}

impl FetchResponder {
    pub(crate) fn new(id: u64, commands: Sender<Command>, aborted: Arc<AtomicBool>) -> Self {
        FetchResponder {
            id,
            commands,
            aborted,
            ended: false,
        }
    }

    fn send(&self, event: FetchEvent) {
        if self.is_aborted() {
            return;
        }
        let sent = self.commands.send(Command::Fetch { id: self.id, event });
        if sent.is_err() {
            // The engine is gone: nobody will read anything more.
            self.aborted.store(true, Ordering::Relaxed);
        }
    }

    /// The HTTP status code. Call it once, before the headers; a response
    /// without one is taken as `200`.
    pub fn status(&self, code: u16) {
        self.send(FetchEvent::Status(code));
    }

    /// One response header. Repeat a name for each of its values (as with
    /// `Set-Cookie`).
    pub fn header(&self, name: &str, value: &str) {
        self.send(FetchEvent::Header(name.to_string(), value.to_string()));
    }

    /// The next piece of the (decoded) body.
    pub fn data(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.send(FetchEvent::Data(bytes.to_vec()));
        }
    }

    /// The response is complete.
    pub fn finish(mut self) {
        self.ended = true;
        self.send(FetchEvent::Finish);
    }

    /// The request failed (DNS, connection, TLS, a broken transfer...);
    /// NetSurf shows `message` on its error page, and when the request was
    /// for the page a view is loading the view reports it as
    /// [`NetSurfViewEvent::FetchFailed`](crate::NetSurfViewEvent::FetchFailed).
    pub fn fail(mut self, message: &str) {
        self.ended = true;
        self.send(FetchEvent::Fail(message.to_string()));
    }

    /// Whether NetSurf no longer wants the response (the page was left, or a
    /// redirect made the body moot): the fetcher should stop and drop the
    /// responder. Everything reported after this is discarded.
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }
}

impl Drop for FetchResponder {
    fn drop(&mut self) {
        if !self.ended {
            self.send(FetchEvent::Fail(
                "the fetcher dropped the request without an answer".to_string(),
            ));
        }
    }
}

impl std::fmt::Debug for FetchResponder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchResponder")
            .field("id", &self.id)
            .field("aborted", &self.is_aborted())
            .finish()
    }
}

/// The application's HTTP client, which NetSurf fetches `http:` and `https:`
/// URLs through (see the [module docs](self) for what it must and must not
/// do).
pub trait Fetcher: Send + Sync + 'static {
    /// Makes `request` and reports the response through `responder`.
    ///
    /// It is called on a thread of its own for each request, so it may block
    /// until the response is complete; it may also hand the responder to
    /// another thread and return.
    fn fetch(&self, request: FetchRequest, responder: FetchResponder);
}

static FETCHER: RwLock<Option<Arc<dyn Fetcher>>> = RwLock::new(None);

/// Makes `fetcher` the one NetSurf fetches `http:` and `https:` URLs
/// through, for every view in the process. Call it before creating the first
/// view; a later call replaces the fetcher for requests made from then on.
///
/// Without a fetcher, NetSurf opens only `file:`, `data:`, `about:` and
/// `resource:` URLs.
pub fn set_fetcher(fetcher: Arc<dyn Fetcher>) {
    *FETCHER.write().unwrap_or_else(PoisonError::into_inner) = Some(fetcher);
}

/// The fetcher, if the application set one.
pub(crate) fn fetcher() -> Option<Arc<dyn Fetcher>> {
    FETCHER
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Runs `request` on `fetcher` on a thread of its own. If no thread can be
/// started the responder is dropped, which fails the fetch.
pub(crate) fn spawn(fetcher: Arc<dyn Fetcher>, request: FetchRequest, responder: FetchResponder) {
    let spawned = std::thread::Builder::new()
        .name("netsurf-fetch".to_string())
        .spawn(move || fetcher.fetch(request, responder));
    if let Err(e) = spawned {
        log::error!("xui-netsurf: could not start a fetch thread: {e}");
    }
}

/// A request handed to the fetcher that NetSurf still wants answered.
struct Live {
    url: String,
    aborted: Arc<AtomicBool>,
}

/// The engine's side of the host fetcher: the requests in flight, so a late
/// answer to an aborted one never reaches NetSurf. Engine thread only.
pub(crate) struct Fetches {
    commands: Sender<Command>,
    live: RefCell<HashMap<u64, Live>>,
}

impl Fetches {
    /// Answers will arrive as [`Command::Fetch`] on `commands`.
    pub(crate) fn new(commands: Sender<Command>) -> Fetches {
        Fetches {
            commands,
            live: RefCell::default(),
        }
    }

    /// NetSurf wants `request` made, answered as fetch `id`.
    pub(crate) fn start(&self, id: u64, request: FetchRequest) {
        let aborted = Arc::new(AtomicBool::new(false));
        let responder = FetchResponder::new(id, self.commands.clone(), Arc::clone(&aborted));
        self.live.borrow_mut().insert(
            id,
            Live {
                url: request.url.clone(),
                aborted,
            },
        );
        match fetcher() {
            Some(fetcher) => spawn(fetcher, request, responder),
            // Dropping the responder fails the fetch.
            None => drop(responder),
        }
    }

    /// NetSurf no longer wants fetch `id`.
    pub(crate) fn abort(&self, id: u64) {
        if let Some(live) = self.live.borrow_mut().remove(&id) {
            live.aborted.store(true, Ordering::Relaxed);
        }
    }

    /// Whether `event` should reach NetSurf (its fetch is still wanted),
    /// with the fetch's URL; the last event retires the fetch.
    pub(crate) fn accept(&self, id: u64, event: &FetchEvent) -> Option<String> {
        let mut live = self.live.borrow_mut();
        if matches!(event, FetchEvent::Finish | FetchEvent::Fail(_)) {
            live.remove(&id).map(|l| l.url)
        } else {
            live.get(&id).map(|l| l.url.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn responder() -> (FetchResponder, mpsc::Receiver<Command>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel();
        let aborted = Arc::new(AtomicBool::new(false));
        (
            FetchResponder::new(7, tx, Arc::clone(&aborted)),
            rx,
            aborted,
        )
    }

    fn events(rx: &mpsc::Receiver<Command>) -> Vec<FetchEvent> {
        rx.try_iter()
            .filter_map(|c| match c {
                Command::Fetch { id: 7, event } => Some(event),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reports_in_order() {
        let (r, rx, _) = responder();
        r.status(200);
        r.header("Content-Type", "text/html");
        r.data(b"<p>");
        r.data(b"");
        r.finish();
        assert_eq!(
            events(&rx),
            [
                FetchEvent::Status(200),
                FetchEvent::Header("Content-Type".into(), "text/html".into()),
                FetchEvent::Data(b"<p>".to_vec()),
                FetchEvent::Finish,
            ]
        );
    }

    #[test]
    fn a_dropped_responder_fails() {
        let (r, rx, _) = responder();
        r.status(200);
        drop(r);
        assert!(matches!(events(&rx).last(), Some(FetchEvent::Fail(_))));
    }

    #[test]
    fn nothing_is_sent_after_an_abort() {
        let (r, rx, aborted) = responder();
        aborted.store(true, Ordering::Relaxed);
        assert!(r.is_aborted());
        r.data(b"late");
        r.fail("late");
        assert!(events(&rx).is_empty());
    }
}
