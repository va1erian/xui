//! Headless light/dark renders of the explorer, written to
//! `target/snapshots/xui-explorer-{light,dark}.png` for eyeballing.
#![forbid(unsafe_code)]

use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::units::Dip;
use xui_core::{Image, Theme};
use xui_explorer::platform::{Launcher, Platform};
use xui_explorer::{Explorer, ExplorerWindow, MemPlatform};

/// A launcher that does nothing: the snapshot never activates a file.
struct NoLauncher;

impl Launcher for NoLauncher {
    fn open(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}

/// A small demo tree so the view has folders and files to show.
fn demo_platform() -> Rc<MemPlatform> {
    Rc::new(
        MemPlatform::new()
            .dir("/demo")
            .dir("/demo/Reports")
            .dir("/demo/Images")
            .file("/demo/notes.txt", 1_024)
            .file("/demo/photo.png", 2_500_000)
            .file("/demo/archive.zip", 51_200)
            .file("/demo/main.rs", 4_096),
    )
}

fn render_explorer(theme: Theme) -> Image {
    let platform: Rc<dyn Platform> = demo_platform();
    try_render(
        Snapshot::new(Dip(720.0), Dip(480.0)).theme(theme),
        move |ui| {
            let explorer = Explorer::new(platform, Rc::new(NoLauncher));
            ExplorerWindow::new(ui, explorer, PathBuf::from("/demo"))
        },
    )
    .expect("the explorer renders")
}

#[test]
fn the_explorer_renders_in_light_and_dark() {
    let light = render_explorer(Theme::light());
    let dark = render_explorer(Theme::dark());
    assert!(light.width() > 0 && light.height() > 0);
    assert!(dark.width() > 0 && dark.height() > 0);
    assert_ne!(light, dark, "the two themes render differently");

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    std::fs::create_dir_all(&dir).expect("snapshot dir");
    light
        .save_png(dir.join("xui-explorer-light.png"))
        .expect("save light");
    dark.save_png(dir.join("xui-explorer-dark.png"))
        .expect("save dark");
}
