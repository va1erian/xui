//! Loading: the page and everything it links to.
//!
//! `file:`, `data:` and `about:blank` are answered here; `http:` and `https:`
//! go to the application's [`Fetcher`](crate::Fetcher). All of them run
//! through the same [`Exchange`], so a `file:` download and an `https:` one
//! take the same path. [`Net`] is the document's
//! [`NetProvider`](blitz_traits::net::NetProvider): style sheets, fonts and
//! images.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use blitz_traits::net::{Body, Bytes, NetHandler, NetProvider, Request};

use crate::fetch::{FetchMethod, FetchRequest, Response};

mod data_url;
mod exchange;
mod form;

pub(crate) use exchange::{Divert, Done, Head, Loaded, Outcome, header};
use exchange::Exchange;

/// What the view says it is, to servers.
pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (compatible; xui-blitz/0.1; Blitz)";

/// Whether the view loads `url` itself (as opposed to handing it to the
/// system): the schemes [`start`] answers.
pub(crate) fn is_loadable(url: &url::Url) -> bool {
    match url.scheme() {
        "file" | "data" => true,
        "about" => url.path() == "blank",
        "http" | "https" => crate::fetch::fetcher().is_some(),
        _ => false,
    }
}

/// Starts `request`; `done` hears how it ended, from whichever thread
/// finishes it. `aborted` stops it early.
pub(crate) fn start(request: FetchRequest, aborted: Arc<AtomicBool>, divert: Option<Divert>, done: Done) {
    let scheme = request.url.split(':').next().unwrap_or("").to_ascii_lowercase();
    if matches!(scheme.as_str(), "http" | "https") {
        if crate::fetch::fetcher().is_none() {
            return done(Outcome::Failed(format!("no fetcher for {scheme}: URLs")));
        }
        return Exchange::new(request, aborted, divert, done).spawn();
    }
    let local = match scheme.as_str() {
        "data" => data_url::decode(&request.url).ok_or_else(|| "malformed data: URL".to_string()),
        "file" => read_file(&request.url),
        "about" if request.url.eq_ignore_ascii_case("about:blank") => {
            Ok(("text/html".to_string(), Vec::new()))
        }
        _ => Err(format!("cannot open {scheme}: URLs")),
    };
    let mut exchange = Box::new(Exchange::new(request, aborted, divert, done));
    match local {
        Ok((mime, body)) => {
            exchange.status(200);
            exchange.header("Content-Type", &mime);
            exchange.header("Content-Length", &body.len().to_string());
            exchange.data(&body);
            exchange.end(Ok(()));
        }
        Err(message) => exchange.end(Err(message)),
    }
}

/// A `file:` URL's bytes and a type guessed from its extension.
fn read_file(url: &str) -> Result<(String, Vec<u8>), String> {
    let path = url::Url::parse(url)
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .ok_or_else(|| format!("not a local file: {url}"))?;
    let body = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    Ok((mime_for_extension(&ext).to_string(), body))
}

/// The MIME type a file extension says, for local files.
pub(crate) fn mime_for_extension(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" | "xhtml" | "shtml" => "text/html",
        "txt" | "text" | "md" | "log" | "rs" | "toml" | "json" | "xml" => "text/plain",
        "css" => "text/css",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}

/// Blitz's request as the fetcher's: method, headers (with a user agent and
/// `accept` unless Blitz set them), and an encoded form body.
pub(crate) fn fetch_request(request: Request, accept: &str) -> FetchRequest {
    let method = match request.method.as_str() {
        "POST" => FetchMethod::Post,
        "HEAD" => FetchMethod::Head,
        _ => FetchMethod::Get,
    };
    let mut headers: Vec<(String, String)> = request
        .headers
        .iter()
        .filter_map(|(n, v)| Some((n.as_str().to_string(), v.to_str().ok()?.to_string())))
        .collect();
    let mut add = |name: &str, value: &str| {
        if header(&headers, name).is_none() {
            headers.push((name.to_string(), value.to_string()));
        }
    };
    add("User-Agent", USER_AGENT);
    add("Accept", accept);
    let body = match request.body {
        Body::Empty => None,
        Body::Bytes(bytes) => {
            if let Some(ty) = &request.content_type {
                add("Content-Type", ty);
            }
            Some(bytes.to_vec())
        }
        Body::Form(form) => {
            let (body, ty) = form::encode(&form, request.content_type.as_deref());
            add("Content-Type", &ty);
            Some(body)
        }
    };
    FetchRequest {
        url: request.url.to_string(),
        method,
        headers,
        body,
    }
}

/// A document's sub-resource loader. Every request it starts shares the
/// document's abort flag, so leaving the page stops them all.
pub(crate) struct Net {
    aborted: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
}

impl Net {
    pub(crate) fn new() -> Net {
        Net {
            aborted: Arc::default(),
            in_flight: Arc::default(),
        }
    }

    /// How many sub-resources are still loading.
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Acquire)
    }

    /// Stops every load the document started.
    pub(crate) fn abort(&self) {
        self.aborted.store(true, Ordering::Relaxed);
    }
}

impl NetProvider for Net {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.to_string();
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        let in_flight = Arc::clone(&self.in_flight);
        let done: Done = Box::new(move |outcome| {
            // A failed load still answers, with nothing: Blitz waits for every
            // render-blocking style sheet, and an empty one is no style at all.
            let (url, body) = match outcome {
                Outcome::Loaded(l) if l.status < 400 => (l.url, l.body),
                Outcome::Loaded(l) => {
                    log::info!("xui-blitz: {} answered {}", l.url, l.status);
                    (l.url, Vec::new())
                }
                Outcome::Failed(why) => {
                    log::info!("xui-blitz: {url}: {why}");
                    (url, Vec::new())
                }
                Outcome::Diverted => (url, Vec::new()),
            };
            in_flight.fetch_sub(1, Ordering::AcqRel);
            handler.bytes(url, Bytes::from(body));
        });
        start(fetch_request(request, "*/*"), Arc::clone(&self.aborted), None, done);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn load(url: &str) -> Outcome {
        let (tx, rx) = mpsc::channel();
        let request = FetchRequest {
            url: url.into(),
            method: FetchMethod::Get,
            headers: Vec::new(),
            body: None,
        };
        start(
            request,
            Arc::default(),
            None,
            Box::new(move |o| {
                let _ = tx.send(o);
            }),
        );
        rx.recv().unwrap()
    }

    #[test]
    fn data_urls_load_with_their_type() {
        let Outcome::Loaded(l) = load("data:text/html,%3Cp%3Ehi") else {
            panic!("not loaded")
        };
        assert_eq!(l.body, b"<p>hi");
        assert_eq!(l.header("content-type"), Some("text/html"));
    }

    #[test]
    fn files_load_with_a_guessed_type() {
        let dir = std::env::temp_dir().join(format!("xui-blitz-net-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("page.htm");
        std::fs::write(&path, "<h1>x</h1>").unwrap();
        let url = url::Url::from_file_path(&path).unwrap();
        let Outcome::Loaded(l) = load(url.as_str()) else {
            panic!("not loaded")
        };
        assert_eq!(l.body, b"<h1>x</h1>");
        assert_eq!(l.header("Content-Type"), Some("text/html"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_schemes_and_missing_files_fail() {
        assert!(matches!(load("gopher://x/"), Outcome::Failed(_)));
        assert!(matches!(load("file:///no/such/file.html"), Outcome::Failed(_)));
    }

    #[test]
    fn requests_get_a_user_agent_and_an_encoded_form() {
        use blitz_traits::net::{Entry, FormData, Method};
        let mut request = Request::get(url::Url::parse("https://example.com/s").unwrap());
        request.method = Method::POST;
        request.body = Body::Form(FormData(vec![Entry {
            name: "q".into(),
            value: "x y".into(),
        }]));
        let r = fetch_request(request, "text/html");
        assert_eq!(r.method, FetchMethod::Post);
        assert_eq!(r.body.as_deref(), Some(&b"q=x+y"[..]));
        assert_eq!(header(&r.headers, "user-agent"), Some(USER_AGENT));
        assert_eq!(header(&r.headers, "accept"), Some("text/html"));
        assert_eq!(
            header(&r.headers, "content-type"),
            Some("application/x-www-form-urlencoded")
        );
    }
}
