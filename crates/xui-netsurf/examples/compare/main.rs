//! Shows one HTML file laid out by litehtml (left) and by NetSurf (right),
//! side by side, both painted by `xui-litehtml`'s painter with the same text
//! shaper, so what differs is the layout.
//!
//! ```text
//! cargo run --example compare                      # the built-in page, in a window
//! cargo run --example compare -- page.html
//! cargo run --example compare -- --screenshot out.png [page.html]
//! ```
//!
//! `--screenshot` renders offscreen (no window or display needed) once both
//! panes have finished, writes the PNG and exits. `--width`/`--height` set the
//! window size and `--dpi` the screenshot's dpi. Set `XUI_DEMO_AUTOCLOSE_MS` to have the window quit itself.

use std::cell::Cell;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use xui_canvas::WinitBackend;
use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{LayoutExt, column, label, row};
use xui_core::backend::{Backend, PlatformSpec, Result};
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::{Dip, Insets};
use xui_litehtml::{HtmlView, HtmlViewEvent};
use xui_netsurf::{NetSurfView, NetSurfViewEvent};

const DEMO: &str = include_str!("demo.html");
/// The height of the caption strip above each pane, in device pixels at 96 dpi.
const CAPTION: i32 = 24;
/// The gap between the panes.
const GAP: i32 = 6;
/// How long `--screenshot` waits for both panes.
const SETTLE: Duration = Duration::from_secs(20);

enum Msg {
    HtmlFrame,
    NetSurfNews,
    Poll,
    Quit,
}

struct Compare {
    html: HtmlView<Msg>,
    netsurf: NetSurfView<Msg>,
    state: Rc<Cell<Panes>>,
}

/// Where the two panes are, as the screenshot waits for them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panes {
    Loading,
    Ready,
    Failed,
}

impl App for Compare {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::HtmlFrame => self.html.invalidate(),
            Msg::NetSurfNews | Msg::Poll => {
                for event in self.netsurf.update() {
                    match event {
                        NetSurfViewEvent::Failed(why) => eprintln!("compare: netsurf: {why}"),
                        NetSurfViewEvent::UrlChanged(url) => eprintln!("compare: netsurf at {url}"),
                        _ => {}
                    }
                }
            }
            Msg::Quit => ui.quit(),
        }
        self.state.set(if self.netsurf.has_failed() {
            Panes::Failed
        } else if self.html.is_ready() && self.netsurf.is_ready() {
            Panes::Ready
        } else {
            Panes::Loading
        });
    }
}

/// Builds both panes over the window's client area.
fn build(ui: &Ui<Msg>, html: String, url: &str, state: Rc<Cell<Panes>>) -> Result<Compare> {
    let client = ui.client_rect();
    let scale = ui.dpi() as f32 / 96.0;
    let caption = (CAPTION as f32 * scale) as i32;
    let mid = client.left + client.width() / 2;
    let left = Rect::new(client.left, client.top, mid - GAP / 2, client.bottom);
    let right = Rect::new(mid + GAP / 2, client.top, client.right, client.bottom);
    let below = |r: Rect| Rect::new(r.left, r.top + caption, r.right, r.bottom);
    let html = HtmlView::new(
        ui,
        below(left),
        html,
        || Msg::HtmlFrame,
        |e| {
            if let HtmlViewEvent::LinkClicked(href) = e {
                eprintln!("compare: litehtml link: {href}");
            }
            None
        },
    )?;
    let netsurf = NetSurfView::new(ui, below(right), url, || Msg::NetSurfNews)?;
    // The captions sit 6 units into their pane, so the gap between them is
    // the panes' gap plus that indent.
    ui.root(
        column()
            .padding(Insets::new(Dip(6.0), Dip(4.0), Dip(0.0), Dip(0.0)))
            .child(
                row()
                    .gap(GAP + 6)
                    .children((label("litehtml").fill(1), label("NetSurf").fill(1)))
                    .height(CAPTION - 4),
            ),
    )?;
    Ok(Compare {
        html,
        netsurf,
        state,
    })
}

/// Repaints and polls until both panes have a finished page, or one fails or
/// time runs out; the panes' state says which.
fn settle(stage: &Stage<'_, Msg>, state: &Cell<Panes>) {
    let start = Instant::now();
    while start.elapsed() < SETTLE {
        // A paint is what starts litehtml's render and sends NetSurf its size.
        let _ = stage.ui().capture();
        stage.emit(Msg::Poll);
        match state.get() {
            Panes::Loading => std::thread::sleep(Duration::from_millis(25)),
            Panes::Ready => {
                // One more round so the newest frames are on screen.
                std::thread::sleep(Duration::from_millis(200));
                stage.emit(Msg::Poll);
                let _ = stage.ui().capture();
                return;
            }
            Panes::Failed => return,
        }
    }
}

fn write_png(image: &Image, path: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image.pixels())?;
    Ok(())
}

/// The page to show: its HTML for litehtml and a `file:` URL for NetSurf.
fn page(path: Option<&str>) -> std::io::Result<(String, String)> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => {
            let p = std::env::temp_dir().join("xui-netsurf-compare.html");
            std::fs::write(&p, DEMO)?;
            p
        }
    };
    let html = std::fs::read_to_string(&path)?;
    let abs = std::fs::canonicalize(&path)?;
    Ok((html, file_url(&abs)))
}

/// `path` as a `file:` URL, percent-encoding every byte that is not safe in
/// a path (so a `#` or `?` in a file name stays part of the path).
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for &b in path.to_string_lossy().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

fn main() {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let (mut file, mut shot) = (None, None);
    let (mut width, mut height) = (1200.0f32, 820.0f32);
    let mut dpi = 96u32;
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match arg.as_str() {
            "--screenshot" => shot = Some(value()),
            "--width" => width = value().parse().unwrap_or(width),
            "--height" => height = value().parse().unwrap_or(height),
            "--dpi" => dpi = value().parse().unwrap_or(dpi),
            flag if flag.starts_with("--") => {
                eprintln!("compare: unknown flag {flag}");
                std::process::exit(2);
            }
            _ => file = Some(arg),
        }
    }
    let (html, url) = page(file.as_deref()).unwrap_or_else(|e| {
        eprintln!("compare: {e}");
        std::process::exit(1);
    });
    let state = Rc::new(Cell::new(Panes::Loading));

    if let Some(out) = shot {
        let watched = Rc::clone(&state);
        let image = render_with(
            Snapshot::new(Dip(width), Dip(height))
                .dpi(dpi)
                .title("compare"),
            move |ui| build(ui, html, &url, state),
            {
                let state = Rc::clone(&watched);
                move |stage| settle(stage, &state)
            },
        );
        let outcome = match watched.get() {
            Panes::Ready => Ok(()),
            Panes::Failed => Err("NetSurf could not open the page".to_string()),
            Panes::Loading => Err("timed out waiting for both panes".to_string()),
        };
        match outcome
            .and(image.map_err(|e| e.to_string()))
            .and_then(|i| write_png(&i, Path::new(&out)).map_err(|e| e.to_string()))
        {
            Ok(()) => eprintln!("compare: wrote {out}"),
            Err(e) => {
                eprintln!("compare: screenshot failed: {e}");
                std::process::exit(1);
            }
        }
        // NetSurf's engine thread is still running; leave without joining it.
        std::process::exit(0);
    }

    let autoclose = std::env::var("XUI_DEMO_AUTOCLOSE_MS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok());
    let backend: Rc<dyn Backend> = Rc::new(WinitBackend::new());
    let result = run_app(
        backend,
        PlatformSpec::new("litehtml vs NetSurf").size(Dip(width), Dip(height)),
        move |ui| {
            let app = build(ui, html, &url, state).expect("create the panes");
            let close = autoclose.map(|ms| ui.set_timer(ms));
            ui.on_timer(move |id| (Some(id) == close).then_some(Msg::Quit));
            app
        },
    );
    if let Err(e) = result {
        eprintln!("compare failed: {e}");
        std::process::exit(1);
    }
}
