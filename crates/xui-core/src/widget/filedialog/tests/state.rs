//! The pure [`FileState`] tests, driven by the in-memory filesystem: no window,
//! no real I/O.

use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;

use super::super::fs::{MemoryFileSystem, dir_entry, file_entry};
use super::super::state::{Accept, Config, FileState};
use crate::backend::{FileDialogMode, FileFilter};
use crate::icon::Lucide;

/// A non-UTF-8 file name, built the platform's way.
#[cfg(unix)]
fn non_utf8_name() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(vec![b'f', 0xFF, b'o'])
}

/// A non-UTF-8 file name, built the platform's way.
#[cfg(windows)]
fn non_utf8_name() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    OsString::from_wide(&[b'f' as u16, 0xD800, b'o' as u16])
}

fn config(
    mode: FileDialogMode,
    fs: MemoryFileSystem,
    initial_dir: Option<&str>,
    suggested_name: &str,
    filters: Vec<FileFilter>,
    require_existing: bool,
) -> Config {
    Config {
        mode,
        fs: Rc::new(fs),
        initial_dir: initial_dir.map(PathBuf::from),
        suggested_name: suggested_name.to_string(),
        filters,
        require_existing,
    }
}

#[test]
fn an_empty_directory_has_no_rows_and_asks_for_a_name() {
    let fs = MemoryFileSystem::new().dir("/", vec![]);
    let mut state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        Vec::new(),
        true,
    ));

    assert!(state.row_labels().is_empty());
    assert_eq!(state.accept(), Accept::Error("Enter a file name".into()));
    assert!(!state.error().is_empty());
}

#[test]
fn an_unreadable_directory_keeps_the_previous_listing() {
    let fs = MemoryFileSystem::new()
        .dir("/", vec![dir_entry("a"), file_entry("b.txt")])
        .dir("/locked", vec![])
        .unreadable("/locked");
    let mut state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        Vec::new(),
        true,
    ));

    assert!(!state.navigate_to("/locked"), "the listing failed");
    assert!(
        state.error().contains("Could not read"),
        "an inline error appears: {}",
        state.error()
    );
    assert_eq!(
        state.row_labels(),
        vec!["a/".to_string(), "b.txt".to_string()],
        "the previous directory and its entries stay current"
    );
    assert_eq!(state.path_text(), "/", "the path bar did not move");
}

#[test]
fn a_missing_home_starts_empty_and_stays_usable() {
    let fs = MemoryFileSystem::new().dir("/", vec![file_entry("x")]);
    let mut state = FileState::new(config(FileDialogMode::Open, fs, None, "", Vec::new(), true));

    assert!(!state.error().is_empty(), "no start location is reported");
    assert!(state.row_labels().is_empty());
    assert_eq!(state.accept(), Accept::Error("Enter a file name".into()));

    state.set_path_text("/");
    assert!(state.navigate_typed(), "typing a path recovers");
    assert_eq!(state.row_labels(), vec!["x".to_string()]);
}

#[test]
fn a_non_utf8_name_displays_lossily_and_round_trips() {
    let name = non_utf8_name();
    let path = PathBuf::from("/").join(&name);
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry(name.clone())])
        .file(path.clone());
    let mut state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        Vec::new(),
        true,
    ));

    let label = state.row_labels().remove(0);
    assert!(
        label.contains('\u{FFFD}'),
        "the display is lossy: {label:?}"
    );
    let chosen = match state.accept() {
        Accept::Chosen(path) => path,
        other => panic!("expected a chosen path, got {other:?}"),
    };
    assert_eq!(
        chosen.file_name(),
        Some(name.as_os_str()),
        "the returned path keeps the original name"
    );
    assert_eq!(chosen, path);
}

#[test]
fn save_confirms_over_an_existing_file_but_not_over_a_new_one() {
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry("x.txt")])
        .file("/x.txt");
    let mut state = FileState::new(config(
        FileDialogMode::Save,
        fs,
        Some("/"),
        "x.txt",
        Vec::new(),
        false,
    ));

    let path = PathBuf::from("/x.txt");
    assert_eq!(state.accept(), Accept::Overwrite(path.clone()));
    assert!(state.is_confirming_overwrite());
    assert_eq!(state.accept(), Accept::Chosen(path));
    assert!(!state.is_confirming_overwrite());

    state.set_name("new.txt");
    assert_eq!(state.accept(), Accept::Chosen(PathBuf::from("/new.txt")));
    assert!(!state.is_confirming_overwrite());
}

#[test]
fn the_extension_filter_is_case_insensitive() {
    let fs = MemoryFileSystem::new().dir(
        "/",
        vec![
            file_entry("a.txt"),
            file_entry("b.TXT"),
            file_entry("c.md"),
            file_entry("noext"),
            file_entry(".hidden"),
            file_entry("d.tar.gz"),
            file_entry("e."),
        ],
    );
    let state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        vec![FileFilter::new("Text", &["txt"])],
        false,
    ));

    assert_eq!(
        state.row_labels(),
        vec!["a.txt".to_string(), "b.TXT".to_string()],
        "case-insensitive, and odd/absent extensions never match"
    );
}

#[test]
fn a_root_parent_is_a_no_op() {
    let fs = MemoryFileSystem::new().dir("/", vec![file_entry("x")]);
    let mut state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        Vec::new(),
        true,
    ));

    assert!(!state.row_labels().contains(&"..".to_string()));
    state.navigate_parent();
    assert_eq!(state.row_labels(), vec!["x".to_string()]);
    assert!(state.error().is_empty(), "the no-op leaves no error");
}

#[test]
fn relative_and_dotdot_paths_are_resolved() {
    let fs = MemoryFileSystem::new()
        .dir("/", vec![dir_entry("a"), file_entry("b.txt")])
        .dir("/a", vec![dir_entry("sub")])
        .dir("/a/sub", vec![])
        .file("/b.txt");
    let mut state = FileState::new(config(
        FileDialogMode::Save,
        fs,
        Some("/a"),
        "",
        Vec::new(),
        false,
    ));

    state.set_name("../b.txt");
    assert_eq!(
        state.accept(),
        Accept::Overwrite(PathBuf::from("/b.txt")),
        ".. is resolved before the existence check"
    );

    state.cancel_overwrite();
    state.set_name("sub/");
    assert_eq!(
        state.accept(),
        Accept::Navigate(PathBuf::from("/a/sub")),
        "a trailing separator names a directory"
    );

    state.set_name("..");
    assert_eq!(
        state.accept(),
        Accept::Navigate(PathBuf::from("/")),
        "a typed .. enters the parent"
    );
}

#[test]
fn save_mode_navigates_into_a_relected_directory() {
    let fs = MemoryFileSystem::new()
        .dir("/", vec![dir_entry("docs")])
        .dir("/docs", vec![]);
    let mut state = FileState::new(config(
        FileDialogMode::Save,
        fs,
        Some("/"),
        "",
        Vec::new(),
        false,
    ));

    assert_eq!(state.accept(), Accept::Navigate(PathBuf::from("/docs")));
}

#[test]
fn moving_the_selection_picks_a_different_file() {
    let fs = MemoryFileSystem::new()
        .dir("/", vec![file_entry("a.txt"), file_entry("b.txt")])
        .file("/a.txt")
        .file("/b.txt");
    let mut state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        Vec::new(),
        true,
    ));

    assert_eq!(state.selection(), 0);
    state.select(0);
    assert_eq!(state.accept(), Accept::Chosen(PathBuf::from("/a.txt")));
    state.select(1);
    assert_eq!(state.accept(), Accept::Chosen(PathBuf::from("/b.txt")));
}

#[test]
fn open_requires_an_existing_file() {
    let fs = MemoryFileSystem::new().dir("/", vec![]);
    let mut state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/"),
        "",
        Vec::new(),
        true,
    ));

    state.set_name("missing.txt");
    assert!(matches!(state.accept(), Accept::Error(_)));
    assert!(state.error().contains("no such file"));
}

#[test]
fn rows_get_a_parent_folder_or_file_icon_in_label_order() {
    let fs = MemoryFileSystem::new().dir("/a", vec![dir_entry("sub"), file_entry("f.txt")]);
    let state = FileState::new(config(
        FileDialogMode::Open,
        fs,
        Some("/a"),
        "",
        Vec::new(),
        false,
    ));

    assert_eq!(state.row_labels(), vec!["..", "sub/", "f.txt"]);
    assert_eq!(
        state.row_icons(),
        vec![Lucide::ChevronUp, Lucide::Folder, Lucide::File]
    );
}
