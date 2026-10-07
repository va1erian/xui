//! One request, from its first byte to its outcome: redirects followed, and a
//! page response handed to a download when it is not one to show.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::fetch::{self, FetchMethod, FetchRequest, FetchResponder, Response};

/// The most redirects one request follows.
const MAX_REDIRECTS: u8 = 10;

/// A complete response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Loaded {
    /// The URL it finally came from, after redirects.
    pub url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Loaded {
    /// The first value of header `name`.
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }
}

/// How a request ended.
#[derive(Debug)]
pub(crate) enum Outcome {
    Loaded(Loaded),
    /// The response went to the stream [`Divert`] returned.
    Diverted,
    Failed(String),
}

/// What a request's start knows: where its response came from, its status
/// and headers.
pub(crate) struct Head<'a> {
    pub url: &'a str,
    pub headers: &'a [(String, String)],
}

/// Called once a final (not redirecting) response's headers are in; returns
/// the stream the rest goes to, or `None` to collect it as usual.
pub(crate) type Divert = Box<dyn FnOnce(&Head<'_>) -> Option<Box<dyn Response>> + Send>;
/// Told how the request ended.
pub(crate) type Done = Box<dyn FnOnce(Outcome) + Send>;

/// The first value of header `name` (case-insensitive).
pub(crate) fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// One request and what it has received so far.
pub(crate) struct Exchange {
    request: FetchRequest,
    hops: u8,
    aborted: Arc<AtomicBool>,
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    divert: Option<Divert>,
    diverted: Option<Box<dyn Response>>,
    decided: bool,
    done: Option<Done>,
}

impl Exchange {
    pub(crate) fn new(
        request: FetchRequest,
        aborted: Arc<AtomicBool>,
        divert: Option<Divert>,
        done: Done,
    ) -> Exchange {
        Exchange {
            request,
            hops: 0,
            aborted,
            status: 200,
            headers: Vec::new(),
            body: Vec::new(),
            divert,
            diverted: None,
            decided: false,
            done: Some(done),
        }
    }

    /// Hands the request to the application's fetcher.
    pub(crate) fn spawn(self) {
        let request = self.request.clone();
        let aborted = Arc::clone(&self.aborted);
        fetch::spawn(request, FetchResponder::new(Box::new(self), aborted));
    }

    /// The next request when this response redirects.
    fn redirect(&self) -> Option<FetchRequest> {
        if !(300..400).contains(&self.status) || self.hops >= MAX_REDIRECTS {
            return None;
        }
        let location = header(&self.headers, "Location")?;
        let base = url::Url::parse(&self.request.url).ok()?;
        let next = base.join(location.trim()).ok()?;
        let to_get = self.status == 303
            || (matches!(self.status, 301 | 302) && self.request.method == FetchMethod::Post);
        let mut request = self.request.clone();
        request.url = next.to_string();
        if to_get {
            request.method = FetchMethod::Get;
            request.body = None;
            request
                .headers
                .retain(|(n, _)| !n.eq_ignore_ascii_case("Content-Type"));
        }
        Some(request)
    }

    /// At the first body byte (or the end), decides where the body goes.
    fn decide(&mut self) {
        if self.decided {
            return;
        }
        self.decided = true;
        if self.redirect().is_some() {
            return;
        }
        let Some(divert) = self.divert.take() else {
            return;
        };
        let head = Head {
            url: &self.request.url,
            headers: &self.headers,
        };
        if let Some(mut stream) = divert(&head) {
            stream.status(self.status);
            for (name, value) in &self.headers {
                stream.header(name, value);
            }
            self.diverted = Some(stream);
        }
    }
}

impl Response for Exchange {
    fn status(&mut self, code: u16) {
        self.status = code;
    }

    fn header(&mut self, name: &str, value: &str) {
        self.headers.push((name.to_string(), value.to_string()));
    }

    fn data(&mut self, bytes: &[u8]) {
        self.decide();
        match &mut self.diverted {
            Some(stream) => stream.data(bytes),
            None => self.body.extend_from_slice(bytes),
        }
    }

    fn end(mut self: Box<Self>, result: Result<(), String>) {
        if result.is_ok() {
            self.decide();
        }
        let done = self.done.take();
        if let Some(stream) = self.diverted.take() {
            stream.end(result);
            if let Some(done) = done {
                done(Outcome::Diverted);
            }
            return;
        }
        let Some(done) = done else {
            return;
        };
        if let Err(message) = result {
            return done(Outcome::Failed(message));
        }
        if let Some(request) = self.redirect() {
            let mut next =
                Exchange::new(request, Arc::clone(&self.aborted), self.divert.take(), done);
            next.hops = self.hops + 1;
            return next.spawn();
        }
        done(Outcome::Loaded(Loaded {
            url: self.request.url,
            status: self.status,
            headers: self.headers,
            body: self.body,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn request(method: FetchMethod) -> FetchRequest {
        FetchRequest {
            url: "http://example.com/a/b".into(),
            method,
            headers: vec![(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )],
            body: Some(b"x=1".to_vec()),
        }
    }

    fn exchange(method: FetchMethod) -> (Box<Exchange>, mpsc::Receiver<Outcome>) {
        let (tx, rx) = mpsc::channel();
        let done: Done = Box::new(move |o| {
            let _ = tx.send(o);
        });
        let e = Exchange::new(request(method), Arc::default(), None, done);
        (Box::new(e), rx)
    }

    #[test]
    fn a_response_is_collected() {
        let (mut e, rx) = exchange(FetchMethod::Get);
        e.status(200);
        e.header("content-type", "text/html");
        e.data(b"<p>");
        e.data(b"hi");
        e.end(Ok(()));
        let Outcome::Loaded(l) = rx.recv().unwrap() else {
            panic!("not loaded");
        };
        assert_eq!(l.body, b"<p>hi");
        assert_eq!(l.header("Content-Type"), Some("text/html"));
    }

    #[test]
    fn a_see_other_after_a_post_becomes_a_get() {
        let (mut e, _rx) = exchange(FetchMethod::Post);
        e.status(303);
        e.header("Location", "../c?d=1");
        let next = e.redirect().unwrap();
        assert_eq!(next.url, "http://example.com/c?d=1");
        assert_eq!(next.method, FetchMethod::Get);
        assert_eq!(next.body, None);
        assert!(next.headers.is_empty());
    }

    #[test]
    fn a_temporary_redirect_keeps_the_post() {
        let (mut e, _rx) = exchange(FetchMethod::Post);
        e.status(307);
        e.header("Location", "https://other.example/");
        let next = e.redirect().unwrap();
        assert_eq!(next.method, FetchMethod::Post);
        assert_eq!(next.body.as_deref(), Some(&b"x=1"[..]));
    }

    #[test]
    fn a_failure_is_reported() {
        let (e, rx) = exchange(FetchMethod::Get);
        e.end(Err("refused".into()));
        assert!(matches!(rx.recv().unwrap(), Outcome::Failed(m) if m == "refused"));
    }

    #[test]
    fn a_diverted_response_streams_and_reports_diverted() {
        struct Count(mpsc::Sender<usize>, usize);
        impl Response for Count {
            fn status(&mut self, _: u16) {}
            fn header(&mut self, _: &str, _: &str) {}
            fn data(&mut self, b: &[u8]) {
                self.1 += b.len();
            }
            fn end(self: Box<Self>, _: Result<(), String>) {
                let _ = self.0.send(self.1);
            }
        }
        let (tx, rx) = mpsc::channel();
        let (count_tx, count_rx) = mpsc::channel();
        let divert: Divert = Box::new(move |head| {
            (header(head.headers, "Content-Type") == Some("application/zip"))
                .then(|| Box::new(Count(count_tx, 0)) as Box<dyn Response>)
        });
        let mut e = Box::new(Exchange::new(
            request(FetchMethod::Get),
            Arc::default(),
            Some(divert),
            Box::new(move |o| {
                let _ = tx.send(o);
            }),
        ));
        e.status(200);
        e.header("Content-Type", "application/zip");
        e.data(b"PK..");
        e.data(b"rest");
        e.end(Ok(()));
        assert_eq!(count_rx.recv().unwrap(), 8);
        assert!(matches!(rx.recv().unwrap(), Outcome::Diverted));
    }
}
