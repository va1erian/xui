//! Guards the software-core contract: every windowing dependency is optional
//! and reachable only through the default-on `winit-backend` feature, and the
//! modules that use them are feature-gated. With `--no-default-features` the
//! crate then builds over `xui-core`, `tiny-skia` and `cosmic-text` alone.
//!
//! Works offline: the manifest and sources are read as text, no `cargo tree`.

use std::path::{Path, PathBuf};

const LIB_DIR: &str = env!("CARGO_MANIFEST_DIR");

/// The dependencies that must never enter a `--no-default-features` build.
const WINDOWED: &[&str] = &[
    "winit",
    "softbuffer",
    "glutin",
    "glow",
    "arboard",
    "xui-gpu",
    "windows",
];

fn manifest() -> String {
    std::fs::read_to_string(Path::new(LIB_DIR).join("Cargo.toml"))
        .expect("read the crate manifest")
        .replace("\r\n", "\n")
}

fn source(relative: &str) -> String {
    std::fs::read_to_string(Path::new(LIB_DIR).join(relative))
        .unwrap_or_else(|_| panic!("read {relative}"))
        .replace("\r\n", "\n")
}

/// The `winit-backend` feature's dependency list.
fn feature_dependencies(manifest: &str) -> String {
    let mut lines = manifest.lines();
    let start = lines
        .position(|line| line.trim_start().starts_with("winit-backend = ["))
        .expect("a `winit-backend` feature");
    let mut block = String::new();
    for line in manifest.lines().skip(start) {
        block.push_str(line);
        block.push('\n');
        if line.contains(']') {
            break;
        }
    }
    block
}

/// The manifest line that declares `name` as a dependency.
fn dependency_line<'a>(manifest: &'a str, name: &str) -> &'a str {
    let prefix = format!("{name} = ");
    manifest
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .unwrap_or_else(|| panic!("no dependency line for `{name}`"))
}

#[test]
fn the_windowed_feature_is_the_default() {
    let manifest = manifest();
    assert!(
        manifest.contains("default = [\"winit-backend\"]"),
        "`winit-backend` must be on by default so existing users see no change"
    );
}

#[test]
fn every_windowed_dependency_is_optional_and_feature_gated() {
    let manifest = manifest();
    let feature = feature_dependencies(&manifest);
    for name in WINDOWED {
        let line = dependency_line(&manifest, name);
        assert!(
            line.contains("optional = true"),
            "`{name}` must be optional so it leaves a no-default-features build: {line}"
        );
        assert!(
            feature.contains(&format!("\"dep:{name}\"")),
            "the `winit-backend` feature must enable `dep:{name}`"
        );
    }
}

#[test]
fn the_windowed_modules_are_feature_gated() {
    let lib = source("src/lib.rs");
    for module in ["backend", "clipboard", "gl", "sys"] {
        let gated = format!("#[cfg(feature = \"winit-backend\")]\nmod {module};");
        assert!(
            lib.contains(&gated),
            "`{module}` must be compiled only with `winit-backend`"
        );
    }
    assert!(
        lib.contains("#[cfg(feature = \"winit-backend\")]\npub use backend::WinitBackend;"),
        "`WinitBackend` must not exist without `winit-backend`"
    );
}

#[test]
fn the_software_core_stays_declared() {
    let lib = source("src/lib.rs");
    for item in [
        "pub use canvas::SkiaCanvas;",
        "pub use offscreen::OffscreenBackend;",
        "pub use text::{add_font, measure as measure_text, set_default_family, set_default_font};",
    ] {
        assert!(
            lib.contains(item),
            "the software core must stay unconditional: `{item}`"
        );
    }
}

/// The always-compiled sources, skipping `lib.rs` (which does the gating), the
/// feature-gated directories, `clipboard.rs`, `gl_fallback.rs` (the gated GL
/// shim) and test files.
fn software_core_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect(&Path::new(LIB_DIR).join("src"), &mut files);
    files
        .into_iter()
        .filter(|path| {
            let text = path.to_string_lossy().replace('\\', "/");
            !text.ends_with("/lib.rs")
                && !text.contains("/backend/")
                && !text.contains("/gl/")
                && !text.contains("/sys/")
                && !text.contains("/tests/")
                && !text.ends_with("/clipboard.rs")
                && !text.ends_with("/gl_fallback.rs")
                && !text.ends_with("_tests.rs")
        })
        .collect()
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_always_compiled_sources_do_not_import_a_windowed_crate_or_module() {
    let files = software_core_files();
    assert!(!files.is_empty());
    for file in files {
        let source = std::fs::read_to_string(&file).expect("read source");
        for forbidden in [
            "use winit",
            "use softbuffer",
            "use glutin",
            "use glow",
            "use arboard",
            "xui_gpu::",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file:?} references `{forbidden}` but is always compiled"
            );
        }
    }
}
