#![cfg(windows)]

//! The native file picker (Common Item Dialog), driven the way a user would:
//! a helper thread waits for the dialog, answers it, and closes it if it is
//! still open afterwards. The picker is modal and pumps its own messages, so
//! `file_dialog` blocks the test thread until the helper answers. Run inside
//! the sandbox (`scripts\sandbox\run.ps1`): the dialog is real.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use xui_core::backend::{
    Backend, FileDialogMode, FileDialogOutcome, FileDialogRequest, FileFilter, PlatformSpec,
};
use xui_d2d::D2dBackend;

#[path = "dialog/driver.rs"]
mod driver;

use driver::{Answer, answer};

fn request(mode: FileDialogMode, title: &str) -> FileDialogRequest {
    FileDialogRequest {
        mode,
        title: title.to_owned(),
        initial_dir: Some(std::env::temp_dir()),
        suggested_name: None,
        filters: vec![FileFilter::new("Levels", &["msl"])],
        require_existing: true,
    }
}

/// Held while a dialog is up: the tests run on parallel threads, and several
/// modal dialogs in one process compete for activation and focus.
static ONE_DIALOG: Mutex<()> = Mutex::new(());

fn show(request: &FileDialogRequest, how: Answer) -> FileDialogOutcome {
    let _one = ONE_DIALOG.lock().unwrap_or_else(PoisonError::into_inner);
    let backend = D2dBackend::new();
    backend.init();
    let window = backend.open_window(&PlatformSpec::new("owner")).unwrap();
    let driver = answer(&request.title, how);
    let outcome = backend.file_dialog(window, request);
    driver
        .join()
        .unwrap()
        .expect("the dialog could not be driven");
    backend.close_window(window);
    outcome
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

/// Asserts the picker chose `expected`. The picker answers with the long form of
/// a path (`runneradmin`) where the temp directory may be the 8.3 short one
/// (`RUNNER~1`), so the folders are compared as the file system resolves them.
fn assert_chosen(outcome: FileDialogOutcome, expected: &Path) {
    let FileDialogOutcome::Chosen(chosen) = outcome else {
        panic!("expected a chosen file, got {outcome:?}");
    };
    let resolve = |path: &Path| {
        let folder = path.parent().expect("a file path has a folder");
        (
            std::fs::canonicalize(folder).expect("the folder exists"),
            path.file_name().map(|name| name.to_owned()),
        )
    };
    assert_eq!(resolve(&chosen), resolve(expected));
}

#[test]
fn opening_returns_the_typed_existing_path() {
    let file = scratch("xui-d2d-open.msl");
    std::fs::write(&file, b"level").unwrap();
    let outcome = show(
        &request(FileDialogMode::Open, "xui-d2d open test"),
        Answer::Type(file.display().to_string()),
    );
    assert_chosen(outcome, &file);
}

#[test]
fn cancelling_is_reported_as_cancelled() {
    let outcome = show(
        &request(FileDialogMode::Open, "xui-d2d cancel test"),
        Answer::Cancel,
    );
    assert_eq!(outcome, FileDialogOutcome::Cancelled);
}

#[test]
fn saving_uses_the_suggested_name_and_the_first_filters_extension() {
    let mut save = request(FileDialogMode::Save, "xui-d2d save test");
    save.require_existing = false;
    save.suggested_name = Some("xui-d2d-saved".to_owned());
    let saved = scratch("xui-d2d-saved.msl");
    // A leftover from an earlier run would raise the overwrite prompt.
    if let Err(error) = std::fs::remove_file(&saved) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");
    }
    let outcome = show(&save, Answer::Accept("xui-d2d-saved".to_owned()));
    assert_chosen(outcome, &saved);
}

#[test]
fn a_dialog_over_no_window_declines_so_the_portable_picker_can_take_over() {
    let backend = D2dBackend::new();
    let none = xui_core::backend::WindowId::from_raw(999);
    let outcome = backend.file_dialog(none, &request(FileDialogMode::Open, "unused"));
    assert_eq!(outcome, FileDialogOutcome::Declined);
}
