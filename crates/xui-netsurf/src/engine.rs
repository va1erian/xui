#![forbid(unsafe_code)]

//! The NetSurf engine thread. The NetSurf core keeps global state, so a
//! process runs one engine, on one thread, serving every view: a view sends
//! [`Command`]s and receives [`Output`]s on its own channel, with a wake-up so
//! the UI thread is never blocked.
//!
//! The thread runs NetSurf's timers (fetches, layout, image decoding are all
//! driven by them), handles the commands that arrived, and after each round
//! records a fresh display list for every window NetSurf invalidated. The
//! host fetcher's answers arrive as commands too, so one wakes the thread.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use xui_core::backend::TextShaper;
use xui_litehtml::DisplayList;

use crate::fetch::{self, FetchEvent, Fetches};
use crate::fonts::Fonts;
use crate::record::{Recorder, Registry};
use crate::sys;

/// The longest the thread sleeps without a timer due, so a window whose
/// content was not yet drawable is retried.
const IDLE_WAIT: Duration = Duration::from_millis(100);

/// The wake-up a view gives the engine: called after an [`Output`] is sent.
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;

/// A pointer action, in document coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseAction {
    Move,
    Press,
    Click,
    Release,
}

/// What a view asks of the engine.
pub(crate) enum Command {
    Open {
        id: u64,
        url: String,
        size: (i32, i32),
        out: Sender<Output>,
        wake: Wake,
    },
    Navigate {
        id: u64,
        url: String,
    },
    Resize {
        id: u64,
        size: (i32, i32),
    },
    Mouse {
        id: u64,
        action: MouseAction,
        x: i32,
        y: i32,
    },
    Key {
        id: u64,
        key: u32,
    },
    Close {
        id: u64,
    },
    /// The host fetcher's answer to fetch `id`.
    Fetch {
        id: u64,
        event: FetchEvent,
    },
}

/// A finished drawing of the page.
pub(crate) struct Frame {
    /// Changes when a new page replaces the old one: the keys in its
    /// display list start over, so the view starts a fresh painter.
    pub(crate) epoch: u64,
    pub(crate) list: Arc<DisplayList>,
}

/// What the engine reports to a view.
pub(crate) enum Output {
    Frame(Frame),
    Title(String),
    Url(String),
    Loading(bool),
    /// NetSurf's `gui_pointer_shape` for the page under the pointer.
    Pointer(i32),
    Failed(String),
    FetchFailed { url: String, message: String },
}

/// Per-window state the NetSurf callbacks update (through `&self`: they
/// arrive re-entrantly while the engine is inside a NetSurf call).
pub(crate) struct WinState {
    size: Cell<(i32, i32)>,
    dirty: Cell<bool>,
    epoch: Cell<u64>,
    /// Between NetSurf's start and stop of a load.
    loading: Cell<bool>,
    /// The URL NetSurf last reported, without its fragment.
    url: RefCell<String>,
    registry: RefCell<Registry>,
    out: Sender<Output>,
    wake: Wake,
}

impl WinState {
    fn send(&self, output: Output) {
        if self.out.send(output).is_ok() {
            (self.wake)();
        }
    }

    pub(crate) fn invalidate(&self) {
        self.dirty.set(true);
    }

    pub(crate) fn size(&self) -> (i32, i32) {
        self.size.get()
    }

    pub(crate) fn event(&self, event: i32) {
        match event {
            sys::EVENT_NEW_CONTENT => {
                self.epoch.set(self.epoch.get() + 1);
                if let Ok(mut registry) = self.registry.try_borrow_mut() {
                    *registry = Registry::default();
                }
                self.dirty.set(true);
            }
            sys::EVENT_START_THROBBER => {
                self.loading.set(true);
                self.send(Output::Loading(true));
            }
            sys::EVENT_STOP_THROBBER => {
                self.loading.set(false);
                self.dirty.set(true);
                self.send(Output::Loading(false));
            }
            sys::EVENT_UPDATE_EXTENT => self.dirty.set(true),
            _ => {}
        }
    }

    pub(crate) fn pointer(&self, shape: i32) {
        self.send(Output::Pointer(shape));
    }

    pub(crate) fn title(&self, title: &str) {
        self.send(Output::Title(title.to_string()));
    }

    pub(crate) fn url(&self, url: &str) {
        *self.url.borrow_mut() = defragment(url).to_string();
        self.send(Output::Url(url.to_string()));
    }
}

/// What the NetSurf host callbacks reach: the text measurer and the
/// fetches in flight.
pub(crate) struct Engine {
    pub(crate) fonts: Fonts,
    pub(crate) fetches: Fetches,
    /// Whether NetSurf hands `http(s):` to the host fetcher yet.
    fetcher_registered: Cell<bool>,
}

impl Engine {
    /// Registers the host fetcher with NetSurf once the application has set
    /// one.
    fn register_fetcher(&self) {
        if !self.fetcher_registered.get() && fetch::fetcher().is_some() {
            let ok = sys::register_fetcher();
            if !ok {
                log::error!("xui-netsurf: could not register the http(s) fetcher");
            }
            // Registering twice would add a second fetcher for the schemes.
            self.fetcher_registered.set(true);
        }
    }
}

/// The engine's command channel, starting the thread on first use with the
/// shaper the first view supplies.
pub(crate) fn commands(shaper: impl FnOnce() -> Arc<dyn TextShaper>) -> Sender<Command> {
    static SENDER: OnceLock<Mutex<Sender<Command>>> = OnceLock::new();
    SENDER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            let shaper = shaper();
            let fetch_tx = tx.clone();
            let spawned = std::thread::Builder::new()
                .name("netsurf-engine".to_string())
                .spawn(move || run(shaper, rx, fetch_tx));
            if let Err(e) = spawned {
                log::error!("xui-netsurf: could not start the engine thread: {e}");
            }
            Mutex::new(tx)
        })
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

struct Window {
    handle: sys::Window,
    state: Box<WinState>,
}

fn run(shaper: Arc<dyn TextShaper>, rx: Receiver<Command>, fetch_tx: Sender<Command>) {
    // NetSurf keeps the host pointer for the life of the process.
    let engine: &'static Engine = Box::leak(Box::new(Engine {
        fonts: Fonts::new(shaper),
        fetches: Fetches::new(fetch_tx),
        fetcher_registered: Cell::new(false),
    }));
    if let Err(e) = sys::init(engine) {
        log::error!("xui-netsurf: NetSurf did not start: {e}");
        refuse_all(&rx, &e);
        return;
    }
    let mut windows: HashMap<u64, Window> = HashMap::new();
    loop {
        let due = sys::poll();
        for window in windows.values() {
            if window.state.dirty.get() {
                draw(engine, window);
            }
        }
        let wait = if due < 0 {
            IDLE_WAIT
        } else {
            IDLE_WAIT.min(Duration::from_millis(due as u64))
        };
        match rx.recv_timeout(wait) {
            Ok(cmd) => {
                handle(engine, &mut windows, cmd);
                while let Ok(cmd) = rx.try_recv() {
                    handle(engine, &mut windows, cmd);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Answers every view that opens a page with `why`, for an engine that could
/// not start, so no view waits on a frame that will never come.
fn refuse_all(rx: &Receiver<Command>, why: &str) {
    for cmd in rx.iter() {
        if let Command::Open { out, wake, .. } = cmd
            && out
                .send(Output::Failed(format!("NetSurf did not start: {why}")))
                .is_ok()
        {
            wake();
        }
    }
}

fn handle(engine: &Engine, windows: &mut HashMap<u64, Window>, cmd: Command) {
    if matches!(cmd, Command::Open { .. } | Command::Navigate { .. }) {
        engine.register_fetcher();
    }
    match cmd {
        Command::Open {
            id,
            url,
            size,
            out,
            wake,
        } => {
            let state = Box::new(WinState {
                size: Cell::new(size),
                dirty: Cell::new(false),
                epoch: Cell::new(0),
                loading: Cell::new(false),
                url: RefCell::default(),
                registry: RefCell::default(),
                out,
                wake,
            });
            match sys::Window::open(&state, &url) {
                Some(handle) => {
                    windows.insert(id, Window { handle, state });
                }
                None => state.send(Output::Failed(format!("cannot open {url}"))),
            }
        }
        Command::Navigate { id, url } => {
            if let Some(w) = windows.get(&id)
                && !w.handle.navigate(&url)
            {
                w.state.send(Output::Failed(format!("cannot open {url}")));
            }
        }
        Command::Resize { id, size } => {
            if let Some(w) = windows.get(&id)
                && w.state.size.get() != size
            {
                w.state.size.set(size);
                w.handle.reformat();
            }
        }
        Command::Mouse { id, action, x, y } => {
            if let Some(w) = windows.get(&id) {
                w.handle.mouse(action, x, y);
            }
        }
        Command::Key { id, key } => {
            if let Some(w) = windows.get(&id) {
                w.handle.key(key);
            }
        }
        Command::Close { id } => {
            if let Some(w) = windows.remove(&id) {
                w.handle.destroy();
            }
        }
        Command::Fetch { id, event } => fetch_event(engine, windows, id, event),
    }
}

/// Passes a fetcher's answer to NetSurf. A failure of the page a window is
/// loading is also reported to that window, before NetSurf replaces the page
/// with its error page, so the application learns the reason.
fn fetch_event(engine: &Engine, windows: &HashMap<u64, Window>, id: u64, event: FetchEvent) {
    let Some(url) = engine.fetches.accept(id, &event) else {
        return;
    };
    if let FetchEvent::Fail(message) = &event {
        log::info!("xui-netsurf: fetching {url} failed: {message}");
        let failed = defragment(&url);
        let loading = windows
            .values()
            .filter(|w| w.state.loading.get() && *w.state.url.borrow() == failed);
        for w in loading {
            w.state.send(Output::FetchFailed {
                url: url.clone(),
                message: message.clone(),
            });
        }
    }
    sys::deliver(id, event);
}

/// `url` without its `#fragment`, which is never fetched.
fn defragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(base, _)| base)
}

/// Records the whole document into a display list and sends it.
fn draw(engine: &Engine, window: &Window) {
    let Some((width, height)) = window.handle.extent() else {
        return;
    };
    if !window.handle.ready() {
        return;
    }
    let state = &window.state;
    let (vw, vh) = state.size.get();
    let (w, h) = (width.max(vw), height.max(vh));
    let list = {
        let Ok(mut registry) = state.registry.try_borrow_mut() else {
            return;
        };
        state.dirty.set(false);
        let mut rec = Recorder::new(&engine.fonts, &mut registry);
        window.handle.redraw(&mut rec, (0, 0, w, h));
        rec.finish((w as f32, height as f32))
    };
    log::debug!("xui-netsurf: drew {w}x{h}, {} commands", list.cmds.len());
    state.send(Output::Frame(Frame {
        epoch: state.epoch.get(),
        list: Arc::new(list),
    }));
}
