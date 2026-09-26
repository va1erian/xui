//! Absolute (anchored) layout demo: a fixed-size window is grown a moment
//! after it opens, so a `StretchHorizontal` bar and a `Fill` panel visibly
//! track the parent while the corner labels stay pinned.
//!
//! Run with:
//!
//! ```text
//! cargo run -p xui-win32 --example anchors
//! ```
//!
//! `WIN32UI_DEMO_AUTOCLOSE_MS` makes it quit itself. `WIN32UI_DEMO_SCREENSHOT`
//! names a PNG to write (composited when the `wgc` feature is on) just before
//! it quits, without raising the window.

#[cfg(windows)]
mod anchors {
    use std::cell::Cell;
    use std::fs::File;
    use std::io::BufWriter;
    use std::path::Path;

    use xui_win32::prelude::*;
    use xui_win32::{Anchor, Layout, Size};

    /// Delay before the window grows, so the opening frame is the design size.
    const GROW_MS: u32 = 1200;
    /// Delay before the screenshot/quit, after the grow has been applied.
    const SHOT_MS: u32 = 2600;

    enum Msg {
        Grow,
        Shot,
        Autoclose,
    }

    struct Demo {
        _labels: Vec<Label>,
        _bars: Vec<ProgressBar>,
        screenshot: Option<String>,
        grew: Cell<bool>,
    }

    impl App for Demo {
        type Msg = Msg;

        fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
            match msg {
                Msg::Grow => {
                    if self.grew.replace(true) {
                        return;
                    }
                    let mut placement = ui.placement();
                    placement.normal.right += dip(140.0).to_px(ui.dpi()).value();
                    placement.normal.bottom += dip(90.0).to_px(ui.dpi()).value();
                    let _ = ui.set_placement(&placement);
                }
                Msg::Shot => {
                    if let Some(path) = self.screenshot.take() {
                        write_screenshot(ui, Path::new(&path));
                    }
                    ui.quit();
                }
                Msg::Autoclose => ui.quit(),
            }
        }
    }

    pub(crate) fn main() {
        let screenshot = std::env::var("WIN32UI_DEMO_SCREENSHOT").ok();
        let autoclose = std::env::var("WIN32UI_DEMO_AUTOCLOSE_MS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok());

        let running = xui_win32::run_app(
            WindowSpec::new("xui anchors (#28)").size(dip(520.0), dip(380.0)),
            move |ui| {
                let dpi = ui.dpi();
                let px = |value: f32| dip(value).to_px(dpi).value();
                let design = Size::new(520, 380);

                let mut labels = Vec::new();
                let mut bars = Vec::new();

                // A bar that only stretches horizontally, and a panel that fills
                // both axes. Both grow when the window does.
                if let Ok(bar) = ProgressBar::new(ui).inspect(|bar| {
                    bar.set_bounds(Rect::new(px(20.0), px(120.0), px(220.0), px(150.0)));
                    bar.set_value(60);
                }) {
                    bars.push(bar);
                }
                if let Ok(fill) = ProgressBar::new(ui).inspect(|bar| {
                    bar.set_bounds(Rect::new(px(20.0), px(200.0), px(220.0), px(290.0)));
                    bar.set_value(35);
                }) {
                    bars.push(fill);
                }

                // Created after the bars so the pinned captions stay on top.
                if let Ok(label) =
                    Label::new(ui, Rect::default(), "TopLeft (pinned)").inspect(|label| {
                        label.set_bounds(Rect::new(px(20.0), px(20.0), px(200.0), px(44.0)));
                    })
                {
                    labels.push(label);
                }
                if let Ok(label) =
                    Label::new(ui, Rect::default(), "BottomRight (pinned)").inspect(|label| {
                        label.set_bounds(Rect::new(px(300.0), px(326.0), px(500.0), px(350.0)));
                    })
                {
                    labels.push(label);
                }

                if let [wide, full] = bars.as_slice() {
                    ui.set_layout(
                        Layout::free(design)
                            .item(wide.anchor(Anchor::StretchHorizontal))
                            .item(full.anchor(Anchor::Fill)),
                    );
                }

                let grow = ui.set_timer(GROW_MS).ok();
                let shot = screenshot.as_ref().and_then(|_| ui.set_timer(SHOT_MS).ok());
                let close = autoclose.and_then(|millis| ui.set_timer(millis).ok());
                ui.on_timer(move |fired| {
                    if Some(fired) == grow {
                        Some(Msg::Grow)
                    } else if Some(fired) == shot {
                        Some(Msg::Shot)
                    } else if Some(fired) == close {
                        Some(Msg::Autoclose)
                    } else {
                        None
                    }
                });

                Demo {
                    _labels: labels,
                    _bars: bars,
                    screenshot,
                    grew: Cell::new(false),
                }
            },
        );
        if running.is_err() {
            eprintln!("anchors: could not start the window");
        }
    }

    /// Captures the window and writes a PNG, preferring the composited surface
    /// so the DWM frame is included without raising the window.
    fn write_screenshot<M: 'static>(ui: &Ui<M>, path: &Path) {
        #[cfg(feature = "wgc")]
        let result = ui.capture_composited();
        #[cfg(not(feature = "wgc"))]
        let result = ui.capture();
        match result {
            Ok(image) => {
                let file = match File::create(path) {
                    Ok(file) => file,
                    Err(error) => {
                        eprintln!("anchors: screenshot failed: {error}");
                        return;
                    }
                };
                let mut encoder =
                    png::Encoder::new(BufWriter::new(file), image.width, image.height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                if let Ok(mut writer) = encoder.write_header()
                    && writer.write_image_data(&image.pixels).is_ok()
                {
                    eprintln!("anchors: wrote screenshot to {}", path.display());
                }
            }
            Err(error) => eprintln!("anchors: screenshot failed: {error}"),
        }
    }
}

#[cfg(windows)]
fn main() {
    anchors::main();
}

#[cfg(not(windows))]
fn main() {}
