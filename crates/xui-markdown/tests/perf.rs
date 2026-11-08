//! Timings for the performance targets in `docs/plans/markdown-view.md`.
//! Ignored by default: run them in release, where the numbers mean something.
//!
//! ```text
//! cargo test -p xui-markdown --release --test perf -- --ignored --nocapture
//! ```
//!
//! `XUI_MARKDOWN_PERF_FILE` names a document to time instead of the generated
//! one.

use std::time::Instant;

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::{Dip, Rect, Theme};
use xui_markdown::{Document, MarkdownView};

struct Host {
    _view: MarkdownView<()>,
}

impl App for Host {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

/// About 100 KB mixing every construct the view draws.
fn generated() -> String {
    let section = include_str!("sample.md");
    let mut text = String::new();
    while text.len() < 100_000 {
        text.push_str(section);
        text.push('\n');
    }
    text
}

fn source() -> String {
    match std::env::var("XUI_MARKDOWN_PERF_FILE") {
        Ok(path) => std::fs::read_to_string(path).expect("the perf file"),
        Err(_) => generated(),
    }
}

#[test]
#[ignore = "timing; run in release"]
fn parse_and_first_paint() {
    let text = source();
    let start = Instant::now();
    let doc = Document::parse(&text);
    let parse = start.elapsed();
    println!(
        "parse: {} bytes, {} blocks in {parse:?}",
        text.len(),
        doc.blocks().len()
    );

    // Every render opens a window, loads fonts and rasterises a frame, and
    // each label below includes that. The empty document and the one-screen
    // sample are the baselines: the document's figure less the sample's is
    // what its length costs a first paint.
    let empty = paint(Document::default());
    let sample = paint(Document::parse(include_str!("sample.md")));
    let full = paint(doc);
    println!("first paint in a new window, empty document: {empty:?}");
    println!("first paint in a new window, the one-screen sample: {sample:?}");
    println!("first paint in a new window, this document: {full:?}");
}

/// How long rendering `doc` once in a fresh 1280 x 800 window takes, best of
/// three (the first run also loads fonts).
fn paint(doc: Document) -> std::time::Duration {
    let doc = std::sync::Arc::new(doc);
    (0..3)
        .map(|_| {
            let start = Instant::now();
            try_render(
                Snapshot::new(Dip(1280.0), Dip(800.0)).theme(Theme::light()),
                |ui| {
                    let view =
                        MarkdownView::new(ui, Rect::new(0, 0, 1280, 800))?.document(doc.clone());
                    Ok(Host { _view: view })
                },
            )
            .expect("render");
            start.elapsed()
        })
        .min()
        .unwrap_or_default()
}
