//! A small Markdown viewer on the portable canvas backend. Opens the file
//! given as the first argument, or the crate's sample document, and prints
//! the destination of every link clicked. Select text with the mouse or
//! Ctrl+A; Ctrl+C or a right-click copies it. Images are read from disk
//! next to the file (PNG or JPEG; web addresses are not fetched).
//!
//! ```text
//! cargo run -p xui-markdown --example viewer -- README.md
//! ```
//!
//! `XUI_DEMO_AUTOCLOSE_MS` makes it quit itself after that many milliseconds,
//! for smoke runs. The window follows the platform's light or dark theme.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use xui_canvas::WinitBackend;
use xui_core::Rect;
use xui_core::app::{App, Ui, run_app};
use xui_core::arrange::{Handle, LayoutExt, build as create, column};
use xui_core::backend::{PlatformSpec, Result};
use xui_core::image::Image;
use xui_core::units::Dip;
use xui_markdown::MarkdownView;

const SAMPLE: &str = include_str!("../tests/sample.md");

enum Msg {
    Link(String),
    /// An image the document shows, by its source as written.
    Image(String),
    /// A right-click: a real app shows a context menu here.
    Context,
    Autoclose,
}

struct Viewer {
    view: Rc<MarkdownView<Msg>>,
    /// The folder relative image sources are read from.
    base: PathBuf,
}

impl App for Viewer {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        match msg {
            Msg::Link(href) => println!("link: {href}"),
            // A real app reads and decodes on a worker thread and sends the
            // result back through `Ui::proxy`.
            Msg::Image(src) => match load(&self.base, &src) {
                Some(image) => {
                    self.view.set_image(&src, image);
                }
                None => println!("image not loaded: {src}"),
            },
            // Stands in for a context menu's Copy: copies the selection.
            Msg::Context => {
                if self.view.copy() {
                    println!("copied: {:?}", self.view.selected_text());
                }
            }
            Msg::Autoclose => ui.quit(),
        }
    }
}

/// Reads and decodes the image at `src`, a path relative to `base` or
/// absolute. Anything with a scheme (`https:`, `data:`) is left alone.
fn load(base: &Path, src: &str) -> Option<Image> {
    if src.contains("://") || src.starts_with("data:") {
        return None;
    }
    let bytes = std::fs::read(base.join(src)).ok()?;
    Image::decode(&bytes).ok()
}

fn build(ui: &mut Ui<Msg>, text: String, base: PathBuf) -> Result<Viewer> {
    let handle = Handle::new();
    ui.root(
        column().children((create(move |ui: &Ui<Msg>| {
            Ok(MarkdownView::new(ui, Rect::default())?
                .markdown(&text)
                .on_link(|href| Some(Msg::Link(href.to_owned())))
                .on_image(|src| Some(Msg::Image(src.to_owned())))
                .on_context(|_| Some(Msg::Context)))
        })
        .bind(&handle)
        .fill(1),)),
    )?;
    let view = handle.get();
    view.focus();
    Ok(Viewer { view, base })
}

fn main() -> Result<()> {
    let path = std::env::args().nth(1);
    let (title, text) = match &path {
        Some(path) => {
            let text = std::fs::read_to_string(path).unwrap_or_else(|error| {
                eprintln!("{path}: {error}");
                std::process::exit(1);
            });
            (format!("{path} - Markdown"), text)
        }
        None => ("Markdown".to_owned(), SAMPLE.to_owned()),
    };
    let base = path
        .as_deref()
        .and_then(|path| Path::new(path).parent())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    run_app(
        Rc::new(WinitBackend::new()),
        PlatformSpec::new(&title).size(Dip(820.0), Dip(720.0)),
        move |ui| {
            let viewer = build(ui, text, base).expect("the viewer's widgets built");
            let autoclose = std::env::var("XUI_DEMO_AUTOCLOSE_MS")
                .ok()
                .and_then(|value| value.parse::<u32>().ok());
            if let Some(millis) = autoclose {
                // The view's own layout timer ticks through this mapping
                // too, so only this timer closes the window.
                let close = ui.set_timer(millis);
                ui.on_timer(move |fired| (fired == close).then_some(Msg::Autoclose));
            }
            viewer
        },
    )
}
