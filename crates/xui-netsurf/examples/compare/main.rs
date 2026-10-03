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
use xui_core::Dip;
use xui_core::app::{App, Ui, run_app};
use xui_core::backend::{Backend, PlatformSpec, Result};
use xui_core::geometry::Rect;
use xui_core::image::Image;
use xui_core::widget::Label;
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
    _captions: [Label<Msg>; 2],
    ready: Rc<Cell<bool>>,
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
        self.ready
            .set(self.html.is_ready() && (self.netsurf.is_ready() || self.netsurf.has_failed()));
    }
}

/// Builds both panes over the window's client area.
fn build(ui: &Ui<Msg>, html: String, url: &str, ready: Rc<Cell<bool>>) -> Result<Compare> {
    let client = ui.client_rect();
    let scale = ui.dpi() as f32 / 96.0;
    let caption = (CAPTION as f32 * scale) as i32;
    let mid = client.left + client.width() / 2;
    let left = Rect::new(client.left, client.top, mid - GAP / 2, client.bottom);
    let right = Rect::new(mid + GAP / 2, client.top, client.right, client.bottom);
    let below = |r: Rect| Rect::new(r.left, r.top + caption, r.right, r.bottom);
    let label = |r: Rect, text| {
        Label::new(
            ui,
            Rect::new(r.left + 6, r.top + 4, r.right, r.top + caption),
            text,
        )
    };
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
    Ok(Compare {
        html,
        netsurf,
        _captions: [label(left, "litehtml")?, label(right, "NetSurf")?],
        ready,
    })
}

/// Repaints and polls until both panes have a finished page.
fn settle(stage: &Stage<'_, Msg>, ready: &Cell<bool>) {
    let start = Instant::now();
    while start.elapsed() < SETTLE {
        // A paint is what starts litehtml's render and sends NetSurf its size.
        let _ = stage.ui().capture();
        stage.emit(Msg::Poll);
        if ready.get() {
            // One more round so the newest frames are on screen.
            std::thread::sleep(Duration::from_millis(200));
            stage.emit(Msg::Poll);
            let _ = stage.ui().capture();
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    eprintln!("compare: timed out waiting for both panes");
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
    Ok((html, format!("file://{}", abs.display())))
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
    let ready = Rc::new(Cell::new(false));

    if let Some(out) = shot {
        let flag = Rc::clone(&ready);
        let image = render_with(
            Snapshot::new(Dip(width), Dip(height))
                .dpi(dpi)
                .title("compare"),
            move |ui| build(ui, html, &url, ready),
            move |stage| settle(stage, &flag),
        );
        match image
            .map_err(|e| e.to_string())
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
            let app = build(ui, html, &url, ready).expect("create the panes");
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
