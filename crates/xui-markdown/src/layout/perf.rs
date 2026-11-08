#![forbid(unsafe_code)]

//! Layout timings against the real cosmic-text shaper, for the targets in
//! `docs/plans/markdown-view.md`. Ignored by default: run them in release.
//!
//! ```text
//! cargo test -p xui-markdown --release --lib perf -- --ignored --nocapture
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use xui_core::backend::Backend;

use super::Layout;
use crate::model::{Document, Inline};
use crate::style::MarkdownStyle;

fn source() -> String {
    if let Ok(path) = std::env::var("XUI_MARKDOWN_PERF_FILE") {
        return std::fs::read_to_string(path).expect("the perf file");
    }
    let mut text = String::new();
    while text.len() < 100_000 {
        text.push_str(include_str!("../../tests/sample.md"));
        text.push('\n');
    }
    text
}

#[test]
#[ignore = "timing; run in release"]
fn first_screen_scroll_and_full_layout() {
    let doc = Arc::new(Document::parse(&source()));
    let shaper = xui_canvas::OffscreenBackend::new().text_shaper();
    // Warm the font system: each face (regular, bold, italic, code) loads
    // once per process, which is not the view's cost.
    let style = MarkdownStyle::default();
    for (inline, code) in [
        (Inline::NONE, false),
        (Inline::BOLD, false),
        (Inline::ITALIC, false),
        (Inline::BOLD.with(Inline::ITALIC), false),
        (Inline::NONE, true),
    ] {
        shaper.layout(
            "warm",
            &style.spec(style.font(inline, None, code)),
            100.0,
            96,
        );
    }
    let mut layout = Layout::new(doc, MarkdownStyle::default(), shaper, 96);
    layout.resize(1220.0, 96);

    let start = Instant::now();
    layout.ensure(0.0, 800.0);
    println!("first screen: {:?}", start.elapsed());

    let start = Instant::now();
    layout.ensure(0.0, 800.0);
    println!("first screen again (a repaint): {:?}", start.elapsed());

    let start = Instant::now();
    layout.ensure(4000.0, 4800.0);
    println!("a screen further down: {:?}", start.elapsed());

    let start = Instant::now();
    while layout.work(Duration::from_millis(4)) {}
    println!(
        "everything else, in 4 ms slices: {:?} ({} px tall)",
        start.elapsed(),
        layout.height()
    );

    layout.resize(800.0, 96);
    let start = Instant::now();
    layout.ensure(0.0, 800.0);
    println!(
        "first screen after a resize (warm cache): {:?}",
        start.elapsed()
    );
}
