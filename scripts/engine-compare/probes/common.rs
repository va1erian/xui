// Shared by every probe (`#[path]`-included): arguments, the settle loop and
// the report line, so the probes differ only in the view they build.
//
// Usage: probe-<engine> <page.html> <out.png> [width] [height]
// Prints `PROBE:<engine>:ready=<bool>:ms=<n>` once the page is loaded (or 60 s
// pass), then writes the PNG.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use xui_canvas::snapshot::{Snapshot, Stage, render_with};
use xui_core::Dip;
use xui_core::app::{App, Ui};
use xui_core::backend::Result;

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

/// Renders the app `build` makes, polling it with `poll` until `ready` says
/// the page is in, and writes the PNG.
pub fn run<A, F>(engine: &str, args: &Args, build: F, poll: A::Msg, ready: Rc<Cell<bool>>)
where
    A: App + 'static,
    A::Msg: Clone + 'static,
    F: FnOnce(&mut Ui<A::Msg>) -> Result<A>,
{
    let start = Instant::now();
    let watched = Rc::clone(&ready);
    let image = render_with(
        Snapshot::new(Dip(args.width), Dip(args.height)).title("probe"),
        build,
        move |stage: &Stage<'_, A::Msg>| {
            while start.elapsed() < Duration::from_secs(60) && !watched.get() {
                let _ = stage.ui().capture();
                stage.emit(poll.clone());
                std::thread::sleep(Duration::from_millis(5));
            }
            // One more round so the newest frame is the one captured.
            stage.emit(poll.clone());
            let _ = stage.ui().capture();
        },
    );
    println!(
        "PROBE:{engine}:ready={}:ms={}",
        ready.get(),
        start.elapsed().as_millis()
    );
    match image {
        Ok(image) => image.save_png(&args.out).expect("write the PNG"),
        Err(e) => {
            eprintln!("probe: {e}");
            std::process::exit(1);
        }
    }
    // Engine threads may still run; leave without joining them.
    std::process::exit(0);
}
