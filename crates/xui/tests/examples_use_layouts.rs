//! The examples are the code people copy, so they place widgets with layouts
//! only: no example but the `absolute()` demo may build a `Rect`.

use std::fs;
use std::path::{Path, PathBuf};

/// The one example whose point is free placement.
const ALLOWED: &[&str] = &["absolute.rs"];

/// Every `.rs` file under `dir`, recursively.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("examples directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_example_positions_widgets_with_rects() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files = Vec::new();
    sources(&dir, &mut files);
    assert!(!files.is_empty(), "no examples found in {}", dir.display());
    let offenders: Vec<String> = files
        .iter()
        .filter(|path| {
            let name = path.file_name().and_then(|name| name.to_str());
            !name.is_some_and(|name| ALLOWED.contains(&name))
        })
        .flat_map(|path| {
            let text = fs::read_to_string(path).expect("readable example");
            text.lines()
                .enumerate()
                .filter(|(_, line)| line.contains("Rect::new") || line.contains("Rect {"))
                .map(|(number, line)| format!("{}:{}: {}", path.display(), number + 1, line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "examples must lay widgets out with rows, columns and grids (absolute() only in \
         absolute.rs):\n{}",
        offenders.join("\n")
    );
}
