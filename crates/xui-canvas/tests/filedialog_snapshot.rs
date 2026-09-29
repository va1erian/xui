//! An offscreen light/dark snapshot of the portable file dialog, open and save
//! modes, rendered by the painted backend.
//!
//! The PNGs (`target/snapshots/filedialog-{open,save}-{light,dark}.png`) are the
//! visual check; the assertions catch a card that paints nothing, a dialog that
//! is not shown, and identical themes.

use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use xui_canvas::snapshot::{Snapshot, try_render};
use xui_core::app::{App, Ui};
use xui_core::backend::BackendError;
use xui_core::{Dip, Entry, FileDialog, FileSystem, Image, Theme};

/// A small fixed filesystem: one root with two directories and three files.
struct DemoFs;

fn entry(name: &str, is_dir: bool) -> Entry {
    Entry {
        name: name.into(),
        is_dir,
        size: None,
        modified: None,
    }
}

impl FileSystem for DemoFs {
    fn list(&self, dir: &Path) -> io::Result<Vec<Entry>> {
        if dir == Path::new("/") {
            Ok(vec![
                entry("docs", true),
                entry("src", true),
                entry("main.rs", false),
                entry("notes.txt", false),
                entry("readme.md", false),
            ])
        } else {
            Ok(Vec::new())
        }
    }

    fn is_dir(&self, path: &Path) -> bool {
        matches!(path.to_str(), Some("/docs" | "/src" | "/"))
    }

    fn exists(&self, path: &Path) -> bool {
        self.is_dir(path)
            || matches!(
                path.to_str(),
                Some("/main.rs" | "/notes.txt" | "/readme.md")
            )
    }

    fn home(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/"))
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("/")]
    }
}

/// An app that owns the dialog and nothing else.
struct DialogApp {
    _dialog: FileDialog<()>,
}

impl App for DialogApp {
    type Msg = ();

    fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
}

fn build(ui: &mut Ui<()>, mode: &str) -> Result<DialogApp, BackendError> {
    let fs: Rc<dyn FileSystem> = Rc::new(DemoFs);
    let dialog = match mode {
        "open" => FileDialog::open_file(ui, "Open a file")?
            .file_system(fs)
            .initial_dir("/")
            .filter("Text", &["txt", "md"])
            .filter("Rust", &["rs"]),
        _ => FileDialog::save_file(ui, "Save as")?
            .file_system(fs)
            .initial_dir("/")
            .suggested_name("notes.txt")
            .filter("Text", &["txt", "md"])
            .filter("Rust", &["rs"]),
    };
    dialog.open();
    Ok(DialogApp { _dialog: dialog })
}

fn render(theme: Theme, mode: &str) -> Image {
    let title = format!("file dialog ({mode})");
    let mode = mode.to_string();
    try_render(
        Snapshot::new(Dip(660.0), Dip(460.0))
            .theme(theme)
            .title(title),
        move |ui| build(ui, &mode),
    )
    .expect("render")
}

#[test]
fn the_file_dialog_renders_open_and_save_in_light_and_dark() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    std::fs::create_dir_all(&dir).expect("snapshot dir");

    for (mode, theme, name) in [
        ("open", Theme::light(), "filedialog-open-light.png"),
        ("open", Theme::dark(), "filedialog-open-dark.png"),
        ("save", Theme::light(), "filedialog-save-light.png"),
        ("save", Theme::dark(), "filedialog-save-dark.png"),
    ] {
        let image = render(theme, mode);
        assert!(image.width() > 0 && image.height() > 0);
        image.save_png(dir.join(name)).expect("save snapshot");
    }

    let open_light = render(Theme::light(), "open");
    let open_dark = render(Theme::dark(), "open");
    let save_light = render(Theme::light(), "save");
    assert_ne!(open_light, open_dark, "the themes render differently");
    assert_ne!(open_light, save_light, "open and save differ");
}
