// Shared by every probe (`#[path]`-included): arguments, the settle loop and
// the report line, so the probes differ only in the view they build.
//
// Usage: probe-<engine> <page.html> <out.png> [width] [height]
// Prints `PROBE:<engine>:ready=<true|failed|false>:ms=<n>` once the page is
// loaded, has failed or 60 s have passed (`false`), and writes the PNG only
// for a loaded page; otherwise it exits with status 1.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::backend::Result;

/// Where a probe's page is. (The baseline and litehtml probes never fail.)
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Loading,
    Ready,
    Failed,
}

pub struct Args {
    pub page: String,
    pub out: String,
    pub width: f32,
    pub height: f32,
}

pub fn args() -> Args {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 2 {
        eprintln!("usage: probe <page.html> <out.png> [width] [height]");
        std::process::exit(2);
    }
    Args {
        page: a[0].clone(),
        out: a[1].clone(),
        width: a.get(2).and_then(|v| v.parse().ok()).unwrap_or(1000.0),
        height: a.get(3).and_then(|v| v.parse().ok()).unwrap_or(1400.0),
    }
}

/// `path` as a `file:` URL.
#[allow(dead_code)]
pub fn file_url(path: &str) -> String {
    let abs = std::fs::canonicalize(Path::new(path)).expect("the page exists");
    let mut url = String::from("file://");
    for &b in abs.to_string_lossy().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Renders the app `build` makes, polling it with `poll` until `state` says
/// the page is in or failed, and writes the PNG of a loaded page.
pub fn run<A, F>(engine: &str, args: &Args, build: F, poll: A::Msg, state: Rc<Cell<Load>>)
where
    A: App + 'static,
    A::Msg: Clone + 'static,
    F: FnOnce(&mut Ui<A::Msg>) -> Result<A>,
{
    let start = Instant::now();
    let watched = Rc::clone(&state);
    let image = render_with(
        Snapshot::new(Dip(args.width), Dip(args.height)).title("probe"),
        build,
        move |stage: &Stage<'_, A::Msg>| {
            while start.elapsed() < Duration::from_secs(60) && watched.get() == Load::Loading {
                let _ = stage.ui().capture();
                stage.emit(poll.clone());
                std::thread::sleep(Duration::from_millis(5));
            }
            // One more round so the newest frame is the one captured.
            stage.emit(poll.clone());
            let _ = stage.ui().capture();
        },
    );
    let ready = match state.get() {
        Load::Ready => "true",
        Load::Failed => "failed",
        Load::Loading => "false",
    };
    println!(
        "PROBE:{engine}:ready={ready}:ms={}",
        start.elapsed().as_millis()
    );
    if state.get() != Load::Ready {
        std::process::exit(1);
    }
    match image
        .map_err(|e| e.to_string())
        .and_then(|i| i.save_png(&args.out).map_err(|e| e.to_string()))
    {
        Ok(()) => {}
        Err(e) => {
            eprintln!("probe: {e}");
            std::process::exit(1);
        }
    }
    // Engine threads may still run; leave without joining them.
    std::process::exit(0);
}
