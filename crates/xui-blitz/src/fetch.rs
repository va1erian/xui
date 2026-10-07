//! Network fetching through a host-supplied [`Fetcher`].
//!
//! The view opens `file:`, `data:` and `about:` URLs itself. Everything over
//! `http:` and `https:` (the page, its style sheets, fonts and images) goes
//! to the application's [`Fetcher`], set with [`set_fetcher`], so the HTTP
//! client and its TLS are the application's choice. The contract is the one
//! `xui-netsurf` had, so a fetcher written for it works unchanged:
//!
//! - It must **not** follow redirects. It reports the `3xx` status and the
//!   `Location` header, and the view follows them (resolving relative
//!   locations, turning a `303`, or a `301`/`302` after a `POST`, into a GET).
//! - It decodes any `Content-Encoding` it asked for itself and leaves that
//!   header out.
//! - It sends the request headers it is given and may add its own
//!   (`Accept-Encoding`, `Host`).
//! - It applies its own timeouts: the view waits as long as it takes.
//!
//! The view keeps no cookies (yet): a fetcher that wants them keeps a jar.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

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

/// A request the view wants made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchRequest {
    /// The absolute `http:` or `https:` URL.
    pub url: String,
    /// The method.
    pub method: FetchMethod,
    /// Headers to send, in order (`User-Agent`, `Accept`, a form's
    /// `Content-Type`...).
    pub headers: Vec<(String, String)>,
    /// The body of a `POST` (an url-encoded or multipart form).
    pub body: Option<Vec<u8>>,
}

/// Where a response's pieces go inside the crate: collected for the engine,
/// or streamed to a download.
pub(crate) trait Response: Send {
    fn status(&mut self, code: u16);
    fn header(&mut self, name: &str, value: &str);
    fn data(&mut self, bytes: &[u8]);
    /// The response ended: `Ok` when it is complete, else why not.
    fn end(self: Box<Self>, result: Result<(), String>);
}

/// The flag that stops one request. A page response that turns into a
/// download swaps in the download's own flag, so leaving the page (which sets
/// the page's flag) no longer stops it, and cancelling the download does.
pub(crate) struct Abort(Mutex<Arc<AtomicBool>>);

impl Abort {
    pub(crate) fn new(flag: Arc<AtomicBool>) -> Arc<Abort> {
        Arc::new(Abort(Mutex::new(flag)))
    }

    /// The flag in force.
    pub(crate) fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub(crate) fn is_set(&self) -> bool {
        self.flag().load(Ordering::Relaxed)
    }

    /// From now on, `flag` stops the request.
    pub(crate) fn replace(&self, flag: Arc<AtomicBool>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = flag;
    }
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
    response: Mutex<Option<Box<dyn Response>>>,
    aborted: Arc<Abort>,
}

impl FetchResponder {
    pub(crate) fn new(response: Box<dyn Response>, aborted: Arc<Abort>) -> FetchResponder {
        FetchResponder {
            response: Mutex::new(Some(response)),
            aborted,
        }
    }

    fn with(&self, f: impl FnOnce(&mut dyn Response)) {
        if self.is_aborted() {
            return;
        }
        let mut guard = self.response.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(response) = guard.as_mut() {
            f(response.as_mut());
        }
    }

    fn end(&self, result: Result<(), String>) {
        let response = self
            .response
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(response) = response {
            // An aborted fetch still ends, so whoever waits on it (a page
            // load, a download) is told rather than left waiting.
            let result = if self.is_aborted() {
                Err("cancelled".to_string())
            } else {
                result
            };
            response.end(result);
        }
    }

    /// The HTTP status code. Call it once, before the headers; a response
    /// without one is taken as `200`.
    pub fn status(&self, code: u16) {
        self.with(|r| r.status(code));
    }

    /// One response header. Repeat a name for each of its values.
    pub fn header(&self, name: &str, value: &str) {
        self.with(|r| r.header(name, value));
    }

    /// The next piece of the (decoded) body.
    pub fn data(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.with(|r| r.data(bytes));
        }
    }

    /// The response is complete.
    pub fn finish(self) {
        self.end(Ok(()));
    }

    /// The request failed (DNS, connection, TLS, a broken transfer...). When
    /// it was for the page a view is loading, the view shows an error page
    /// and reports [`BlitzViewEvent::FetchFailed`](crate::BlitzViewEvent::FetchFailed).
    pub fn fail(self, message: &str) {
        self.end(Err(message.to_string()));
    }

    /// Whether the view no longer wants the response (the page was left, or
    /// a download was cancelled): the fetcher should stop and drop the
    /// responder. Everything reported after this is discarded.
    pub fn is_aborted(&self) -> bool {
        self.aborted.is_set()
    }
}

impl Drop for FetchResponder {
    fn drop(&mut self) {
        self.end(Err(
            "the fetcher dropped the request without an answer".to_string()
        ));
    }
}

impl std::fmt::Debug for FetchResponder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchResponder")
            .field("aborted", &self.is_aborted())
            .finish()
    }
}

/// Makes `http:` and `https:` requests for the view (see the module docs).
pub trait Fetcher: Send + Sync + 'static {
    /// Makes `request` and reports the response through `responder`.
    ///
    /// It is called on a thread of its own for each request, so it may block
    /// until the response is complete; it may also hand the responder to
    /// another thread and return.
    fn fetch(&self, request: FetchRequest, responder: FetchResponder);
}

static FETCHER: RwLock<Option<Arc<dyn Fetcher>>> = RwLock::new(None);

/// Makes `fetcher` the one every view fetches `http:` and `https:` URLs
/// through. Call it before creating the first view; a later call replaces
/// the fetcher for requests made from then on.
///
/// Without a fetcher, views open only `file:`, `data:` and `about:` URLs.
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

/// Runs `request` on the fetcher on a thread of its own. Without a fetcher,
/// or a thread, the responder is dropped, which fails the fetch.
pub(crate) fn spawn(request: FetchRequest, responder: FetchResponder) {
    let Some(fetcher) = fetcher() else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("blitz-fetch".to_string())
        .spawn(move || fetcher.fetch(request, responder));
    if let Err(e) = spawned {
        log::error!("xui-blitz: could not start a fetch thread: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[derive(Debug, PartialEq)]
    enum Piece {
        Status(u16),
        Header(String, String),
        Data(Vec<u8>),
        End(Result<(), String>),
    }

    struct Record(mpsc::Sender<Piece>);

    impl Response for Record {
        fn status(&mut self, code: u16) {
            let _ = self.0.send(Piece::Status(code));
        }
        fn header(&mut self, name: &str, value: &str) {
            let _ = self.0.send(Piece::Header(name.into(), value.into()));
        }
        fn data(&mut self, bytes: &[u8]) {
            let _ = self.0.send(Piece::Data(bytes.to_vec()));
        }
        fn end(self: Box<Self>, result: Result<(), String>) {
            let _ = self.0.send(Piece::End(result));
        }
    }

    fn responder() -> (FetchResponder, mpsc::Receiver<Piece>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel();
        let aborted = Arc::new(AtomicBool::new(false));
        let r = FetchResponder::new(Box::new(Record(tx)), Abort::new(Arc::clone(&aborted)));
        (r, rx, aborted)
    }

    #[test]
    fn reports_in_order_and_ends_once() {
        let (r, rx, _) = responder();
        r.status(200);
        r.header("Content-Type", "text/html");
        r.data(b"<p>");
        r.data(b"");
        r.finish();
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            [
                Piece::Status(200),
                Piece::Header("Content-Type".into(), "text/html".into()),
                Piece::Data(b"<p>".to_vec()),
                Piece::End(Ok(())),
            ]
        );
    }

    #[test]
    fn a_replaced_flag_decides_from_then_on() {
        let (r, rx, page) = responder();
        let download = Arc::new(AtomicBool::new(false));
        r.aborted.replace(Arc::clone(&download));
        page.store(true, Ordering::Relaxed);
        r.data(b"still wanted");
        download.store(true, Ordering::Relaxed);
        r.data(b"dropped");
        r.finish();
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            [
                Piece::Data(b"still wanted".to_vec()),
                Piece::End(Err("cancelled".into()))
            ]
        );
    }

    #[test]
    fn a_dropped_responder_fails() {
        let (r, rx, _) = responder();
        r.status(200);
        drop(r);
        assert!(matches!(rx.try_iter().last(), Some(Piece::End(Err(_)))));
    }

    #[test]
    fn an_aborted_fetch_ends_as_cancelled_and_drops_the_rest() {
        let (r, rx, aborted) = responder();
        aborted.store(true, Ordering::Relaxed);
        assert!(r.is_aborted());
        r.data(b"late");
        r.finish();
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            [Piece::End(Err("cancelled".into()))]
        );
    }
}
