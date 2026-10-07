//! Pages: showing one, loading one (links, forms, the app's `navigate`), and
//! what the request's outcome becomes: the page, an error page or a download.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::navigation::NavigationOptions;

use super::providers::Post;
use super::{Command, Doc, Engine, Output, hints, page};
use crate::download;
use crate::fetch::{FetchMethod, FetchRequest};
use crate::net::{self, Divert, Done, Net, Outcome};
use crate::view::BlitzViewEvent;

/// What a page request accepts.
const ACCEPT_PAGE: &str =
    "text/html,application/xhtml+xml,text/plain;q=0.9,image/*;q=0.8,*/*;q=0.5";

impl Engine {
    /// Replaces the document with `html`, at `url`.
    pub(super) fn show(&mut self, html: &str, url: String) {
        if let Some(old) = self.doc.take() {
            old.net.abort();
        }
        let net = Arc::new(Net::new(&url));
        let post = Arc::new(Post::new(self.tx.clone()));
        let config = DocumentConfig {
            viewport: Some(self.viewport()),
            base_url: Some(url.clone()),
            net_provider: Some(Arc::clone(&net) as _),
            navigation_provider: Some(Arc::clone(&post) as _),
            shell_provider: Some(post as _),
            font_ctx: Some(self.font_context()),
            ..DocumentConfig::default()
        };
        let mut doc = HtmlDocument::from_html(html, config).into_inner();
        if let Some(css) = hints::style_sheet(&doc) {
            doc.add_user_agent_stylesheet(&css);
        }
        self.doc = Some(Doc { doc, net });
        self.fragment = url::Url::parse(&url)
            .ok()
            .and_then(|u| u.fragment().map(str::to_string));
        if url != self.url {
            self.url.clone_from(&url);
            self.report.event(BlitzViewEvent::UrlChanged(url));
        }
        self.title = None;
        self.set_status(String::new());
        self.dirty = true;
    }

    /// Opens `url` for the application.
    pub(super) fn navigate(&mut self, url: &str) {
        let Ok(parsed) = url::Url::parse(url) else {
            return self
                .report
                .event(BlitzViewEvent::Failed(format!("not a URL: {url}")));
        };
        if !net::is_loadable(&parsed) {
            return self.report.event(BlitzViewEvent::LaunchUrl {
                url: url.to_string(),
                by_user: true,
            });
        }
        self.load(FetchRequest {
            url: parsed.to_string(),
            method: FetchMethod::Get,
            headers: vec![
                ("User-Agent".to_string(), net::USER_AGENT.to_string()),
                ("Accept".to_string(), ACCEPT_PAGE.to_string()),
            ],
            body: None,
        });
    }

    /// A link or form the user followed in the page.
    pub(super) fn link(&mut self, options: NavigationOptions) {
        let url = options.url.to_string();
        if !self.options.follow_links {
            return self.report.event(BlitzViewEvent::LinkClicked(url));
        }
        if !net::is_loadable(&options.url) {
            return self
                .report
                .event(BlitzViewEvent::LaunchUrl { url, by_user: true });
        }
        // A web page may not send the view to the disk.
        if options.url.scheme() == "file" && !net::is_file_url(&self.url) {
            return self.report.event(BlitzViewEvent::Failed(format!(
                "a web page cannot open a local file: {url}"
            )));
        }
        self.load(net::fetch_request(options.into_request(), ACCEPT_PAGE));
    }

    /// Requests a page; it replaces the document when it arrives (unless it
    /// turns out to be a download).
    pub(super) fn load(&mut self, request: FetchRequest) {
        self.stop();
        self.begin_load();
        let abort = Arc::new(AtomicBool::new(false));
        let generation = self.generation;
        let tx = Mutex::new(self.tx.clone());
        let report = self.report.clone();
        let diverted = Mutex::new(self.tx.clone());
        let divert: Divert = Box::new(move |head| {
            let download = download::is_download(head.headers)
                .then(|| download::begin(head, report))
                .flatten()?;
            // The page load is over (the page on show stays); the download
            // goes on by itself, stopped only by its own cancel.
            let tx = diverted
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = tx.send(Command::Diverted { generation });
            Some(download)
        });
        let done: Done = Box::new(move |outcome| {
            let tx = tx
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = tx.send(Command::Page {
                generation,
                outcome,
            });
        });
        self.loading = Some((request.url.clone(), Arc::clone(&abort)));
        self.report.send(Output::Failed(false));
        net::start(request, abort, Some(divert), done);
    }

    /// Reports that a load started; `settle` reports its end, once the page
    /// and its resources are in.
    pub(super) fn begin_load(&mut self) {
        if !self.busy {
            self.busy = true;
            self.report.event(BlitzViewEvent::LoadingChanged(true));
        }
    }

    /// Abandons the page request in flight, if any.
    pub(super) fn stop(&mut self) {
        self.generation += 1;
        if let Some((_, abort)) = self.loading.take() {
            abort.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn page(&mut self, outcome: Outcome) {
        let Some((requested, _)) = self.loading.take() else {
            return;
        };
        match outcome {
            Outcome::Loaded(loaded) if loaded.status >= 400 && loaded.body.is_empty() => {
                let message = format!("The server answered {}.", loaded.status);
                self.fail(loaded.url, message);
            }
            Outcome::Loaded(loaded) => {
                let html = page::html_for(&loaded);
                self.show(&html, loaded.url);
            }
            Outcome::Diverted => {}
            Outcome::Failed(message) => self.fail(requested, message),
        }
    }

    /// Shows the error page for `url` and reports why it failed.
    pub(super) fn fail(&mut self, url: String, message: String) {
        self.show(&page::error_page(&url, &message), url.clone());
        self.report.send(Output::Failed(true));
        self.report
            .event(BlitzViewEvent::FetchFailed { url, message });
    }

    pub(super) fn download(&mut self, url: String) {
        let report = self.report.clone();
        let divert: Divert = Box::new(move |head| download::begin(head, report));
        let report = self.report.clone();
        let done: Done = Box::new(move |outcome| match outcome {
            Outcome::Failed(why) => report.event(BlitzViewEvent::Failed(why)),
            Outcome::Loaded(l) => log::info!("xui-blitz: download of {} not taken", l.url),
            Outcome::Diverted => {}
        });
        let request = FetchRequest {
            url,
            method: FetchMethod::Get,
            headers: vec![("User-Agent".to_string(), net::USER_AGENT.to_string())],
            body: None,
        };
        net::start(request, Arc::default(), Some(divert), done);
    }
}
