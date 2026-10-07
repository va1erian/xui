//! The engine thread behind one view: it owns the Blitz document, loads
//! pages, takes the view's input and draws frames.
//!
//! The UI thread and Blitz's own callbacks ([`providers`]) queue
//! [`Command`]s; the loop takes everything queued, then settles once:
//! restyle and relayout, report what changed, and draw a new frame if
//! anything asked for one. Frames and the cursor go back to the view as
//! [`Output`]s, events straight to the application's `on_event`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use blitz_dom::{BaseDocument, DocumentConfig, local_name};
use blitz_html::HtmlDocument;
use blitz_traits::navigation::NavigationOptions;
use blitz_traits::shell::{ColorScheme, Viewport};
use cursor_icon::CursorIcon;
use parley::FontContext;
use xui_core::Color;
use xui_core::backend::Cursor;
use xui_core::image::Image;
use xui_core::message::Key;

use crate::download;
use crate::fetch::{FetchMethod, FetchRequest};
use crate::fonts;
use crate::net::{self, Divert, Done, Net, Outcome};
use crate::view::BlitzViewEvent;

mod input;
pub(crate) mod page;
mod providers;
mod render;

pub(crate) use input::{Input, PointerAction};
use providers::Post;
use render::{Raster, cursor_for};

/// What a page request accepts.
const ACCEPT_PAGE: &str = "text/html,application/xhtml+xml,text/plain;q=0.9,image/*;q=0.8,*/*;q=0.5";
/// CSS pixels one arrow key scrolls.
const KEY_LINE: f64 = 40.0;
/// How often an animating page is redrawn.
const ANIMATION_FRAME: Duration = Duration::from_millis(16);

/// What the view and Blitz ask of the engine.
pub(crate) enum Command {
    /// Show `html`, resolving its links against `base_url`.
    Html { html: String, base_url: String },
    /// Open `url` (the application asked).
    Navigate(String),
    /// A link or form Blitz followed.
    Link(Box<NavigationOptions>),
    /// A page request ended.
    Page { generation: u64, outcome: Outcome },
    Stop,
    Download(String),
    /// The view's size in device pixels and its scale.
    Resize { size: (u32, u32), scale: f32 },
    Dark(bool),
    Background(Color),
    Input(Input),
    ScrollTo(f64),
    ClearSelection,
    /// Something in the document changed: settle and draw.
    Redraw,
    Cursor(Option<CursorIcon>),
    Copy(String),
    Quit,
}

/// One frame for the view.
pub(crate) struct Frame {
    pub image: Image,
    /// The page is fully loaded and laid out.
    pub ready: bool,
}

/// What the engine tells the view's UI side.
pub(crate) enum Output {
    Frame(Frame),
    Cursor(Cursor),
    /// The page could not be opened (an error page shows instead), or loading
    /// started again.
    Failed(bool),
}

/// Wakes the UI thread (posts the view's `on_frame` message).
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;
/// Hands an event to the application.
pub(crate) type OnEvent = Arc<dyn Fn(BlitzViewEvent) + Send + Sync>;

/// The way back to the view, from the engine and from fetch threads.
#[derive(Clone)]
pub(crate) struct Reporter {
    out: Sender<Output>,
    wake: Wake,
    /// A wake is posted and the view has not drained yet, so another would
    /// only queue a duplicate message.
    pending: Arc<AtomicBool>,
    on_event: OnEvent,
}

impl Reporter {
    pub(crate) fn new(out: Sender<Output>, wake: Wake, pending: Arc<AtomicBool>, on_event: OnEvent) -> Reporter {
        Reporter {
            out,
            wake,
            pending,
            on_event,
        }
    }

    fn send(&self, output: Output) {
        if self.out.send(output).is_ok() && !self.pending.swap(true, Ordering::AcqRel) {
            (self.wake)();
        }
    }

    /// Hands `event` to the application.
    pub(crate) fn event(&self, event: BlitzViewEvent) {
        (self.on_event)(event);
    }
}

/// How a view treats links.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub follow_links: bool,
}

/// Starts a view's engine thread; the sender is how the view talks to it.
pub(crate) fn spawn(options: Options, report: Reporter, size: (u32, u32), scale: f32) -> Sender<Command> {
    let (tx, rx) = std::sync::mpsc::channel();
    let commands = tx.clone();
    // A Blitz document is not `Send`: the engine is built on its own thread.
    let spawned = std::thread::Builder::new()
        .name("blitz-engine".to_string())
        .spawn(move || Engine::new(options, commands, report, size, scale).run(rx));
    if let Err(e) = spawned {
        log::error!("xui-blitz: could not start the engine thread: {e}");
    }
    tx
}

/// The document on show and the loader of its resources.
struct Doc {
    doc: BaseDocument,
    net: Arc<Net>,
}

struct Engine {
    options: Options,
    tx: Sender<Command>,
    report: Reporter,
    doc: Option<Doc>,
    /// The font context documents clone, and the font registry's version it
    /// was built at.
    fonts: Option<(u64, FontContext)>,
    raster: Raster,
    size: (u32, u32),
    scale: f32,
    dark: bool,
    background: Color,
    /// Numbers page requests: an answer to an older one is ignored.
    generation: u64,
    /// The page request in flight: its URL and abort flag.
    loading: Option<(String, Arc<AtomicBool>)>,
    /// A fragment to scroll to once the new page is laid out.
    fragment: Option<String>,
    busy: bool,
    url: String,
    title: Option<String>,
    status: String,
    cursor: Cursor,
    dirty: bool,
    started: Instant,
}

impl Engine {
    fn new(options: Options, tx: Sender<Command>, report: Reporter, size: (u32, u32), scale: f32) -> Engine {
        Engine {
            options,
            tx,
            report,
            doc: None,
            fonts: None,
            raster: Raster::new(),
            size,
            scale,
            dark: false,
            background: Color::rgb(255, 255, 255),
            generation: 0,
            loading: None,
            fragment: None,
            busy: false,
            url: String::new(),
            title: None,
            status: String::new(),
            cursor: Cursor::Default,
            dirty: false,
            started: Instant::now(),
        }
    }

    fn run(mut self, rx: Receiver<Command>) {
        loop {
            let animating = self.doc.as_ref().is_some_and(|d| d.doc.is_animating());
            let first = if animating {
                match rx.recv_timeout(ANIMATION_FRAME) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            };
            self.dirty |= animating;
            for command in first.into_iter().chain(rx.try_iter()) {
                if matches!(command, Command::Quit) {
                    return self.quit();
                }
                self.handle(command);
            }
            self.settle();
        }
        self.quit();
    }

    fn quit(&mut self) {
        if let Some((_, abort)) = self.loading.take() {
            abort.store(true, Ordering::Relaxed);
        }
        if let Some(doc) = self.doc.take() {
            doc.net.abort();
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Html { html, base_url } => {
                self.stop();
                self.show(&html, base_url);
            }
            Command::Navigate(url) => self.navigate(&url),
            Command::Link(options) => self.link(*options),
            Command::Page { generation, outcome } if generation == self.generation => {
                self.page(outcome)
            }
            Command::Page { .. } => {}
            Command::Stop => {
                self.stop();
                if let Some(doc) = &self.doc {
                    doc.net.abort();
                }
            }
            Command::Download(url) => self.download(url),
            Command::Resize { size, scale } => {
                if (size, scale) != (self.size, self.scale) {
                    (self.size, self.scale) = (size, scale);
                    self.update_viewport();
                }
            }
            Command::Dark(dark) => {
                if dark != self.dark {
                    self.dark = dark;
                    self.update_viewport();
                }
            }
            Command::Background(color) => {
                self.background = color;
                self.dirty = true;
            }
            Command::Input(input) => self.input(input),
            Command::ScrollTo(y) => {
                if let Some(d) = &mut self.doc {
                    let x = d.doc.viewport_scroll().x;
                    d.doc.set_viewport_scroll(blitz_dom::Point { x, y });
                    self.dirty = true;
                }
            }
            Command::ClearSelection => {
                if let Some(d) = &mut self.doc {
                    d.doc.clear_text_selection();
                    self.dirty = true;
                }
            }
            Command::Redraw => self.dirty = true,
            Command::Cursor(icon) => {
                let cursor = cursor_for(icon);
                if cursor != self.cursor {
                    self.cursor = cursor;
                    self.report.send(Output::Cursor(cursor));
                }
            }
            Command::Copy(text) => self.report.event(BlitzViewEvent::CopyRequested(text)),
            Command::Quit => {}
        }
    }

    fn viewport(&self) -> Viewport {
        let scheme = if self.dark {
            ColorScheme::Dark
        } else {
            ColorScheme::Light
        };
        Viewport::new(self.size.0, self.size.1, self.scale, scheme)
    }

    fn update_viewport(&mut self) {
        let viewport = self.viewport();
        if let Some(d) = &mut self.doc {
            d.doc.set_viewport(viewport);
        }
        self.dirty = true;
    }

    fn font_context(&mut self) -> FontContext {
        let version = fonts::version();
        match &self.fonts {
            Some((v, fonts)) if *v == version => fonts.clone(),
            _ => {
                let fresh = fonts::font_context();
                self.fonts = Some((version, fresh.clone()));
                fresh
            }
        }
    }

    /// Replaces the document with `html`, at `url`.
    fn show(&mut self, html: &str, url: String) {
        if let Some(old) = self.doc.take() {
            old.net.abort();
        }
        let net = Arc::new(Net::new());
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
        let doc = HtmlDocument::from_html(html, config).into_inner();
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
    fn navigate(&mut self, url: &str) {
        let Ok(parsed) = url::Url::parse(url) else {
            return self.report.event(BlitzViewEvent::Failed(format!("not a URL: {url}")));
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
    fn link(&mut self, options: NavigationOptions) {
        let url = options.url.to_string();
        if !self.options.follow_links {
            return self.report.event(BlitzViewEvent::LinkClicked(url));
        }
        if !net::is_loadable(&options.url) {
            return self
                .report
                .event(BlitzViewEvent::LaunchUrl { url, by_user: true });
        }
        self.load(net::fetch_request(options.into_request(), ACCEPT_PAGE));
    }

    /// Requests a page; it replaces the document when it arrives (unless it
    /// turns out to be a download).
    fn load(&mut self, request: FetchRequest) {
        self.stop();
        let abort = Arc::new(AtomicBool::new(false));
        let generation = self.generation;
        let tx = Mutex::new(self.tx.clone());
        let report = self.report.clone();
        let divert: Divert = Box::new(move |head| {
            download::is_download(head.headers)
                .then(|| download::begin(head, report))
                .flatten()
        });
        let done: Done = Box::new(move |outcome| {
            let tx = tx.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = tx.send(Command::Page { generation, outcome });
        });
        self.loading = Some((request.url.clone(), Arc::clone(&abort)));
        self.report.send(Output::Failed(false));
        net::start(request, abort, Some(divert), done);
    }

    /// Abandons the page request in flight, if any.
    fn stop(&mut self) {
        self.generation += 1;
        if let Some((_, abort)) = self.loading.take() {
            abort.store(true, Ordering::Relaxed);
        }
    }

    fn page(&mut self, outcome: Outcome) {
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
    fn fail(&mut self, url: String, message: String) {
        self.show(&page::error_page(&url, &message), url.clone());
        self.report.send(Output::Failed(true));
        self.report.event(BlitzViewEvent::FetchFailed { url, message });
    }

    fn download(&mut self, url: String) {
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

    fn input(&mut self, input: Input) {
        let Some(d) = &mut self.doc else {
            return;
        };
        if let Input::Key { key, .. } = &input
            && !text_input_focused(&d.doc)
            && let Some(dy) = key_scroll(*key, self.size.1 as f64 / self.scale as f64)
        {
            d.doc.scroll_viewport_by(0.0, dy);
            self.dirty = true;
            return;
        }
        let scroll = d.doc.viewport_scroll();
        let Some(event) = input::to_ui_event(&input, (scroll.x, scroll.y)) else {
            return;
        };
        blitz_dom::Document::handle_ui_event(&mut d.doc, event);
        if matches!(input, Input::Pointer { .. }) {
            let link = hovered_link(&d.doc).unwrap_or_default();
            self.set_status(link);
        }
        if !matches!(
            input,
            Input::Pointer {
                action: PointerAction::Move,
                ..
            }
        ) {
            self.dirty = true;
        }
    }

    fn set_status(&mut self, status: String) {
        if status != self.status {
            self.status.clone_from(&status);
            self.report.event(BlitzViewEvent::StatusChanged(status));
        }
    }

    /// After a batch of commands: lays the page out, reports what changed and
    /// draws a frame if one is due.
    fn settle(&mut self) {
        let mut busy = self.loading.is_some();
        if let Some(d) = &mut self.doc {
            if self.dirty {
                d.doc.resolve(self.started.elapsed().as_secs_f64());
            }
            let blocked = d.doc.has_pending_critical_resources();
            busy |= blocked || d.net.in_flight() > 0;
            if !blocked && let Some(fragment) = self.fragment.take() {
                d.doc.scroll_to_fragment(&fragment);
                self.dirty = true;
            }
            let title = d
                .doc
                .find_title_node()
                .map(|n| n.text_content().split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            if self.title.as_ref() != Some(&title) {
                self.title = Some(title.clone());
                self.report.event(BlitzViewEvent::TitleChanged(title));
            }
        }
        if busy != self.busy {
            self.busy = busy;
            self.report.event(BlitzViewEvent::LoadingChanged(busy));
            // The frame carries readiness, so the view hears it.
            self.dirty = true;
        }
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let Some(d) = &mut self.doc else {
            return;
        };
        let scale = d.doc.viewport().scale_f64();
        if let Some(image) = self.raster.draw(&mut d.doc, self.size, scale, self.background) {
            self.report.send(Output::Frame(Frame {
                image,
                ready: !busy,
            }));
        }
    }
}

/// Whether the focused element takes typed text (so the movement keys are
/// its, not the page's).
fn text_input_focused(doc: &BaseDocument) -> bool {
    doc.get_focussed_node_id()
        .and_then(|id| doc.get_node(id))
        .and_then(|n| n.element_data())
        .is_some_and(|e| e.text_input_data().is_some())
}

/// How far a movement key scrolls the page, given the viewport's height in
/// CSS pixels.
fn key_scroll(key: Key, page: f64) -> Option<f64> {
    Some(match key {
        Key::UP => -KEY_LINE,
        Key::DOWN => KEY_LINE,
        Key::PAGE_UP => -page * 0.9,
        Key::PAGE_DOWN => page * 0.9,
        Key::HOME => f64::MIN / 4.0,
        Key::END => f64::MAX / 4.0,
        _ => return None,
    })
}

/// The URL of the link under the pointer, if any.
fn hovered_link(doc: &BaseDocument) -> Option<String> {
    let mut id = doc.get_hover_node_id();
    while let Some(node) = id.and_then(|i| doc.get_node(i)) {
        if node
            .element_data()
            .is_some_and(|e| e.name.local == local_name!("a"))
            && let Some(href) = node.attr(local_name!("href"))
        {
            return doc.url().join(href).ok().map(|u| u.to_string());
        }
        id = node.parent;
    }
    None
}
