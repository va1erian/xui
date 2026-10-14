//! Helpers shared by the `xui-form` tool's test files: each runs the binary
//! as a process on files in a scratch directory.

// Each test binary uses its own part of the helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh scratch directory for `test`.
pub fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("xui-form-cli")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is created");
    dir
}

/// Runs `xui-form` with `args`.
pub fn xui_form(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xui-form"))
        .args(args)
        .output()
        .expect("xui-form runs")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Writes `text` to `dir/name` and returns the path as a string.
pub fn file(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("the file is written");
    path.to_string_lossy().into_owned()
}

pub const HELLO: &str = r#"Form(title: "Hello", size: (240, 120), root: Column(gap: 8, padding: 16, children: [
    Edit(name: "name_edit", placeholder: "Your name"),
    Button(name: "greet_button", text: "Greet"),
    Label(name: "result_label"),
]))
"#;

/// The files in `dir`, by name.
pub fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory reads")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}
