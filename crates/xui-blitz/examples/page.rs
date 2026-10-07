//! Shows a page in a [`BlitzView`] filling a window, or renders it headlessly
//! to a PNG.
//!
//! ```text
//! cargo run -p xui-blitz --example page -- page.html
//! cargo run -p xui-blitz --example page -- --screenshot out.png page.html
//! ```
//!
//! The page is a local file or a `data:` URL: the example installs no
//! [`Fetcher`](xui_blitz::Fetcher), so it refuses `http:` and `https:`.
//! `--screenshot` renders offscreen (no window or display needed) once the page
//! and its resources have loaded, writes the PNG and exits; it fails, writing
//! nothing, when the page cannot be opened or takes over a minute. `--width`/`--height` set the size in
//! design units, `--dpi` the screenshot's dpi and `--dark` the dark theme. With
//! no page it shows a built-in demo. Set `XUI_DEMO_AUTOCLOSE_MS` to have the
//! window quit itself.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use xui_blitz::{BlitzView, BlitzViewEvent};
use xui_canvas::WinitBackend;
use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec, Result};
use xui_core::theme::Theme;
use xui_core::{Color, Dip};

const DEMO: &str = include_str!("demo.html");
/// How long `--screenshot` waits for the page.
const SETTLE: Duration = Duration::from_secs(60);

enum Msg {
    Frame,
    Event(BlitzViewEvent),
    Quit,
}

/// Where the page is, as the screenshot waits for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Loading,
    Ready,
    Failed,
}

struct Page {
    view: BlitzView<Msg>,
    state: Rc<Cell<State>>,
}

impl App for Page {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Frame => self.view.update(),
            Msg::Event(BlitzViewEvent::LinkClicked(url)) => self.view.navigate(&url),
            Msg::Event(BlitzViewEvent::CopyRequested(text)) => ui.set_clipboard_text(&text),
            Msg::Event(event) => eprintln!("page: {event:?}"),
            Msg::Quit => ui.quit(),
        }
        self.state.set(if self.view.has_failed() {
            State::Failed
        } else if self.view.is_ready() {
            State::Ready
        } else {
            State::Loading
        });
    }
}

fn build(ui: &Ui<Msg>, url: &str, dark: bool, state: Rc<Cell<State>>) -> Result<Page> {
    let background = if dark {
        Color::rgb(0x20, 0x20, 0x20)
    } else {
        Color::rgb(255, 255, 255)
    };
    let view = BlitzView::builder(|| Msg::Frame, |e| Some(Msg::Event(e)))
        .url(url)
        .background(background)
        .build(ui, ui.client_rect())?;
    Ok(Page { view, state })
}

/// Repaints and polls until the page is ready, fails or time runs out; the
/// state says which.
fn settle(stage: &Stage<'_, Msg>, state: &Cell<State>) {
    let start = Instant::now();
    while start.elapsed() < SETTLE && state.get() == State::Loading {
        let _ = stage.ui().capture();
        stage.emit(Msg::Frame);
        std::thread::sleep(Duration::from_millis(10));
    }
    stage.emit(Msg::Frame);
    eprintln!(
        "page: {:?} after {} ms",
        state.get(),
        start.elapsed().as_millis()
    );
}

/// `arg` as a URL: itself when it has a scheme, else a local file's.
fn to_url(arg: Option<&str>) -> String {
    let path = match arg {
        Some(a) if a.starts_with("http:") || a.starts_with("https:") => {
            eprintln!("page: {a}: this example has no fetcher; give it a file or a data: URL");
            std::process::exit(2);
        }
        Some(a) if a.contains("://") || a.starts_with("data:") || a.starts_with("about:") => {
            return a.to_string();
        }
        Some(a) => Path::new(a).to_path_buf(),
        None => {
            let p = std::env::temp_dir().join("xui-blitz-demo.html");
            std::fs::write(&p, DEMO).expect("write the demo page");
            p
        }
    };
    let abs = std::fs::canonicalize(&path).unwrap_or_else(|e| {
        eprintln!("page: {}: {e}", path.display());
        std::process::exit(1);
    });
    url::Url::from_file_path(abs)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| {
            eprintln!("page: not a path: {}", path.display());
            std::process::exit(1);
        })
}

fn main() {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let (mut page, mut shot) = (None, None);
    let (mut width, mut height, mut dpi, mut dark) = (1000.0f32, 800.0f32, 96u32, false);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match arg.as_str() {
            "--screenshot" => shot = Some(value()),
            "--width" => width = value().parse().unwrap_or(width),
            "--height" => height = value().parse().unwrap_or(height),
            "--dpi" => dpi = value().parse().unwrap_or(dpi),
            "--dark" => dark = true,
            flag if flag.starts_with("--") => {
                eprintln!("page: unknown flag {flag}");
                std::process::exit(2);
            }
            _ => page = Some(arg),
        }
    }
    let url = to_url(page.as_deref());
    let state = Rc::new(Cell::new(State::Loading));
    let theme = if dark { Theme::dark() } else { Theme::light() };

    if let Some(out) = shot {
        let watched = Rc::clone(&state);
        let image = render_with(
            Snapshot::new(Dip(width), Dip(height))
                .dpi(dpi)
                .theme(theme)
                .title("page"),
            move |ui| build(ui, &url, dark, state),
            {
                let state = Rc::clone(&watched);
                move |stage| settle(stage, &state)
            },
        );
        let outcome = match watched.get() {
            State::Ready => Ok(()),
            State::Failed => Err("the page could not be opened".to_string()),
            State::Loading => Err("timed out waiting for the page".to_string()),
        };
        match outcome
            .and(image.map_err(|e| e.to_string()))
            .and_then(|i| i.save_png(&out).map_err(|e| e.to_string()))
        {
            Ok(()) => eprintln!("page: wrote {out}"),
            Err(e) => {
                eprintln!("page: screenshot failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let autoclose = std::env::var("XUI_DEMO_AUTOCLOSE_MS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok());
    let backend: Rc<dyn Backend> = Rc::new(WinitBackend::new());
    let result = run_app(
        backend,
        PlatformSpec::new("xui-blitz").size(Dip(width), Dip(height)),
        move |ui| {
            let app = build(ui, &url, dark, state).expect("create the view");
            let close = autoclose.map(|ms| ui.set_timer(ms));
            ui.on_timer(move |id| (Some(id) == close).then_some(Msg::Quit));
            app
        },
    );
    if let Err(e) = result {
        eprintln!("page failed: {e}");
        std::process::exit(1);
    }
}
