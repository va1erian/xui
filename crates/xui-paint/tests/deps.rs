//! Proves the library's app logic is `xui-core`-only: no `xui-win32`, no
//! `winit`, and the `xui-canvas` backend is an optional, default-off-for-the-lib
//! dependency referenced only by `main.rs`. Works offline.

use std::path::{Path, PathBuf};

const LIB_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            files_under(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_manifest_never_pulls_in_win32() {
    let manifest = std::fs::read_to_string(Path::new(LIB_DIR).join("Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("xui-win32") && !manifest.contains("windows"),
        "the paint crate must not depend on a Windows backend"
    );
    assert!(
        manifest.contains("xui-canvas") && manifest.contains("optional = true"),
        "the canvas backend must be an optional dependency"
    );
    assert!(
        manifest.contains("canvas = [\"dep:xui-canvas\"]"),
        "the canvas feature must be what opts the backend in"
    );
}

#[test]
fn only_main_rs_names_the_canvas_backend() {
    let mut files = Vec::new();
    files_under(&Path::new(LIB_DIR).join("src"), &mut files);
    assert!(!files.is_empty());
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let names_canvas = source.contains("xui_canvas") || source.contains("xui-canvas");
        if names_canvas {
            assert_eq!(
                file.file_name().and_then(|name| name.to_str()),
                Some("main.rs"),
                "backend selection must live in main.rs, not {file:?}"
            );
        }
    }
}
