#![cfg(windows)]
// The driver below calls Win32 to operate the picker from another thread; that
// is the one place a test needs `unsafe`, so it is confined to `driver`.

//! The native file picker (Common Item Dialog), driven the way a user would:
//! a helper thread waits for the dialog, answers it, and closes it if it is
//! still open afterwards. The picker is modal and pumps its own messages, so
//! `file_dialog` blocks the test thread until the helper answers. Run inside
//! the sandbox (`scripts\sandbox\run.ps1`): the dialog is real.

use std::path::{Path, PathBuf};

use xui_core::backend::{
    Backend, FileDialogMode, FileDialogOutcome, FileDialogRequest, FileFilter, PlatformSpec,
};
use xui_d2d::D2dBackend;

mod driver {
    #![allow(unsafe_code)]

    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        BM_CLICK, FindWindowExW, FindWindowW, GetDlgItem, IsWindow, SendMessageW, WM_CLOSE,
        WM_SETTEXT,
    };
    use windows::core::{HSTRING, w};

    /// What the helper does once the dialog is up.
    pub enum Answer {
        /// Type `text` into the file-name box, then press the default button.
        Type(String),
        /// Press the default button as the dialog stands (a suggested name is
        /// already in the box).
        Accept,
        /// Press Cancel.
        Cancel,
    }

    const IDOK: i32 = 1;
    const IDCANCEL: i32 = 2;

    fn find_dialog(title: &str) -> Option<HWND> {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            // SAFETY: a lookup by class and title; the title outlives the call.
            let found = unsafe { FindWindowW(w!("#32770"), &HSTRING::from(title)) };
            if let Ok(hwnd) = found
                && !hwnd.is_invalid()
            {
                return Some(hwnd);
            }
            thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// The edit control of the dialog's file-name combo box.
    fn name_edit(dialog: HWND) -> Option<HWND> {
        // SAFETY: plain child-window lookups on a live dialog.
        unsafe {
            let ex = FindWindowExW(Some(dialog), None, w!("ComboBoxEx32"), None).ok()?;
            let combo = FindWindowExW(Some(ex), None, w!("ComboBox"), None).ok()?;
            FindWindowExW(Some(combo), None, w!("Edit"), None).ok()
        }
    }

    fn click(dialog: HWND, id: i32) -> Result<(), String> {
        // SAFETY: a lookup and a message to a live dialog of this process.
        unsafe {
            let button =
                GetDlgItem(Some(dialog), id).map_err(|_| format!("button {id} was not found"))?;
            SendMessageW(button, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        Ok(())
    }

    fn type_name(dialog: HWND, text: &str) -> Result<(), String> {
        let edit = name_edit(dialog).ok_or("the file-name box was not found")?;
        let text = HSTRING::from(text);
        // SAFETY: the text outlives the call, which copies it.
        unsafe {
            SendMessageW(
                edit,
                WM_SETTEXT,
                Some(WPARAM(0)),
                Some(LPARAM(text.as_ptr() as isize)),
            )
        };
        Ok(())
    }

    /// Whether the dialog still exists after the answer, giving it a moment to
    /// close; if it does, it is closed here so the test fails instead of
    /// hanging on (say) a modal error box it raised.
    fn closed_after_answer(dialog: HWND) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            // SAFETY: a liveness query on a window handle.
            if !unsafe { IsWindow(Some(dialog)) }.as_bool() {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        // SAFETY: a message to a window that still exists.
        unsafe { SendMessageW(dialog, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0))) };
        false
    }

    /// Answers the dialog titled `title` from a helper thread.
    pub fn answer(title: &str, answer: Answer) -> JoinHandle<Result<(), String>> {
        let title = title.to_owned();
        thread::spawn(move || {
            let dialog = find_dialog(&title)
                .ok_or_else(|| format!("no dialog titled {title:?} appeared"))?;
            // Give the dialog a moment to build its controls.
            thread::sleep(Duration::from_millis(500));
            let answered = match answer {
                Answer::Type(text) => type_name(dialog, &text).and_then(|()| click(dialog, IDOK)),
                Answer::Accept => click(dialog, IDOK),
                Answer::Cancel => click(dialog, IDCANCEL),
            };
            if let Err(reason) = answered {
                closed_after_answer(dialog);
                return Err(reason);
            }
            if closed_after_answer(dialog) {
                Ok(())
            } else {
                Err("the dialog stayed open after it was answered".into())
            }
        })
    }
}

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

fn show(request: &FileDialogRequest, how: Answer) -> FileDialogOutcome {
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
    let outcome = show(&save, Answer::Accept);
    assert_chosen(outcome, &scratch("xui-d2d-saved.msl"));
}

#[test]
fn a_dialog_over_no_window_declines_so_the_portable_picker_can_take_over() {
    let backend = D2dBackend::new();
    let none = xui_core::backend::WindowId::from_raw(999);
    let outcome = backend.file_dialog(none, &request(FileDialogMode::Open, "unused"));
    assert_eq!(outcome, FileDialogOutcome::Declined);
}
