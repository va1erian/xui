//! Operates a real Common Item Dialog from a helper thread, the way a user
//! would: waits for it to be ready, answers it, and fails with the window that
//! blocked it if it stays open. Calling Win32 across threads is the one place
//! the dialog tests need `unsafe`, so it is confined to this module.
#![allow(unsafe_code)]

use std::io::Write;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::TDM_CLICK_BUTTON;
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_CLICK, EnumChildWindows, EnumThreadWindows, FindWindowW, GW_OWNER, GetClassNameW,
    GetDlgItem, GetParent, GetWindow, GetWindowTextW, GetWindowThreadProcessId, IDCANCEL, IDNO,
    IDOK, IsWindow, IsWindowVisible, MESSAGEBOX_RESULT, PostMessageW, SendMessageW, WM_COMMAND,
    WM_SETTEXT,
};
use windows::core::{BOOL, HSTRING, w};

/// What the helper does once the dialog is up.
pub enum Answer {
    /// Type `text` into the file-name box, then press the default button.
    Type(String),
    /// Press the default button once the file-name box holds this suggested
    /// name.
    Accept(String),
    /// Press Cancel.
    Cancel,
}

/// How long the dialog may take to appear, to become ready, and to close once
/// answered. Generous: a coverage-instrumented CI runner is slow.
const APPEAR: Duration = Duration::from_secs(20);
const READY: Duration = Duration::from_secs(20);
const CLOSE: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(50);

fn find_dialog(title: &str) -> Option<HWND> {
    let deadline = Instant::now() + APPEAR;
    while Instant::now() < deadline {
        // SAFETY: a lookup by class and title; the title outlives the call.
        let found = unsafe { FindWindowW(w!("#32770"), &HSTRING::from(title)) };
        if let Ok(hwnd) = found
            && !hwnd.is_invalid()
        {
            return Some(hwnd);
        }
        thread::sleep(POLL);
    }
    None
}

fn class_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 64];
    // SAFETY: the buffer outlives the call, which writes at most its length.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    utf16(&buffer[..len.max(0) as usize])
}

fn text_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    // SAFETY: the buffer outlives the call, which writes at most its length.
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    utf16(&buffer[..len.max(0) as usize])
}

/// Text up to its first NUL: some controls count a terminator in the length
/// they report.
fn utf16(text: &[u16]) -> String {
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

unsafe extern "system" fn collect(hwnd: HWND, found: LPARAM) -> BOOL {
    // SAFETY: `found` is the `Vec` that `descendants` or `popups` lends for the
    // enumeration, which calls back on this thread before it returns.
    unsafe { &mut *(found.0 as *mut Vec<HWND>) }.push(hwnd);
    true.into()
}

/// Every window under `parent`, at any depth.
fn descendants(parent: HWND) -> Vec<HWND> {
    let mut found: Vec<HWND> = Vec::new();
    // SAFETY: `collect` only pushes into `found`, which outlives the call.
    unsafe {
        let _ = EnumChildWindows(Some(parent), Some(collect), LPARAM(&raw mut found as isize));
    }
    found
}

/// The other visible top-level windows on the dialog's thread: a prompt or
/// error box it raised. The dialog and the windows that own it are not
/// popups.
fn popups(dialog: HWND) -> Vec<HWND> {
    let mut found: Vec<HWND> = Vec::new();
    // SAFETY: queries on window handles; `collect` only pushes into `found`,
    // which outlives the enumeration.
    unsafe {
        let mut skip = vec![dialog];
        while let Ok(owner) = GetWindow(skip[skip.len() - 1], GW_OWNER) {
            skip.push(owner);
        }
        let thread = GetWindowThreadProcessId(dialog, None);
        let _ = EnumThreadWindows(thread, Some(collect), LPARAM(&raw mut found as isize));
        found.retain(|&hwnd| !skip.contains(&hwnd) && IsWindowVisible(hwnd).as_bool());
    }
    found
}

/// A popup as a failure names it: its class, title and any text in it.
fn describe(hwnd: HWND) -> String {
    let texts: Vec<String> = descendants(hwnd)
        .into_iter()
        .map(text_of)
        .filter(|text| !text.trim().is_empty())
        .collect();
    format!("{} {:?} saying {texts:?}", class_of(hwnd), text_of(hwnd))
}

/// The edit control of the dialog's file-name box: the one edit whose parent
/// is a combo box. Open nests it in a `ComboBoxEx32`, Save deeper in the
/// dialog's view, and the file-type combo is a drop-down list without one.
fn name_edit(dialog: HWND) -> Option<HWND> {
    descendants(dialog).into_iter().find(|&hwnd| {
        // SAFETY: a parent lookup on a window handle.
        let parent = unsafe { GetParent(hwnd) };
        class_of(hwnd) == "Edit" && parent.is_ok_and(|parent| class_of(parent) == "ComboBox")
    })
}

fn button(dialog: HWND, id: MESSAGEBOX_RESULT) -> Option<HWND> {
    // SAFETY: a child lookup on a dialog handle.
    unsafe { GetDlgItem(Some(dialog), id.0) }.ok()
}

fn type_name(edit: HWND, text: &str) {
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
}

/// What readiness looks at, kept so a timeout can say what was missing.
#[derive(Debug)]
struct Readiness {
    visible: bool,
    name: Option<String>,
    ok_enabled: Option<bool>,
}

impl Readiness {
    /// The dialog's state, and its file-name edit if it has one yet.
    fn of(dialog: HWND) -> (Self, Option<HWND>) {
        let edit = name_edit(dialog);
        // SAFETY: state queries on window handles.
        let (visible, ok_enabled) = unsafe {
            (
                IsWindowVisible(dialog).as_bool(),
                button(dialog, IDOK).map(|ok| IsWindowEnabled(ok).as_bool()),
            )
        };
        let name = edit.map(text_of);
        let state = Readiness {
            visible,
            name,
            ok_enabled,
        };
        (state, edit)
    }

    fn ready(&self, name: Option<&str>) -> bool {
        self.visible
            && self.ok_enabled == Some(true)
            && name.is_none_or(|name| self.name.as_deref() == Some(name))
    }
}

/// Waits until the dialog is shown, `IDOK` is enabled and (given `name`) the
/// file-name box holds `name`, on two polls in a row so a box the dialog is
/// still filling in does not count. With `retype`, `name` is typed again
/// whenever the box does not hold it.
fn wait_until_ready(dialog: HWND, name: Option<&str>, retype: bool) -> Result<(), String> {
    let deadline = Instant::now() + READY;
    let mut ready_before = false;
    loop {
        let (state, edit) = Readiness::of(dialog);
        let ready = state.ready(name);
        if ready && ready_before {
            return Ok(());
        }
        ready_before = ready;
        if Instant::now() >= deadline {
            return Err(format!(
                "the dialog was not ready after {READY:?} (wanted the name {name:?}): {state:?}"
            ));
        }
        if retype
            && let (Some(edit), Some(name)) = (edit, name)
            && state.name.as_deref() != Some(name)
        {
            type_name(edit, name);
        }
        thread::sleep(POLL);
    }
}

fn click(dialog: HWND, id: MESSAGEBOX_RESULT) -> Result<(), String> {
    let button = button(dialog, id).ok_or(format!("button {} was not found", id.0))?;
    // Posted, not sent: a press that raises a prompt (Save over an existing
    // file) runs the prompt's modal loop inside the click, and a sent click
    // would hold this thread there until someone answered the prompt.
    // SAFETY: a posted message to a window handle; a stale handle is refused.
    unsafe { PostMessageW(Some(button), BM_CLICK, WPARAM(0), LPARAM(0)) }
        .map_err(|error| format!("button {} could not be pressed: {error}", id.0))
}

fn gone_within(window: HWND, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        // SAFETY: a liveness query on a window handle.
        if !unsafe { IsWindow(Some(window)) }.as_bool() {
            return true;
        }
        thread::sleep(POLL);
    }
    false
}

/// Asks a prompt to close as Cancel, or as No when it has no Cancel (the
/// overwrite prompt). A message box takes the press as `WM_COMMAND`; a task
/// dialog hosts its buttons without control ids and takes it only as
/// `TDM_CLICK_BUTTON`. Each ignores the form it does not use and a button it
/// does not have, and every request is posted, so none waits on the popup.
fn decline(popup: HWND) {
    for id in [IDCANCEL, IDNO] {
        let id = WPARAM(id.0 as usize);
        // SAFETY: posted messages to a window handle; a stale handle is refused.
        unsafe {
            let _ = PostMessageW(Some(popup), WM_COMMAND, id, LPARAM(0));
            let _ = PostMessageW(Some(popup), TDM_CLICK_BUTTON.0 as u32, id, LPARAM(0));
        }
    }
}

/// Fails a dialog that is still open: names the popups blocking it, declines
/// each (its Cancel button, or No on a yes/no prompt such as the overwrite
/// one), and only once they are gone cancels the dialog, so the dialog never
/// handles a press from inside a nested modal loop. If the dialog still does
/// not close, the test thread would block in `file_dialog` forever: the reason
/// goes straight to stderr (libtest's capture would never be printed) and the
/// process aborts rather than hang.
fn give_up(dialog: HWND, reason: &str) -> String {
    let blockers = popups(dialog);
    let reason = if blockers.is_empty() {
        format!("{reason}; no popup was blocking it")
    } else {
        let names: Vec<String> = blockers.iter().copied().map(describe).collect();
        format!("{reason}; blocked by {}", names.join(", "))
    };
    let declined = blockers.iter().all(|&popup| {
        decline(popup);
        gone_within(popup, CLOSE)
    });
    if declined {
        let _ = click(dialog, IDCANCEL);
    }
    if !(declined && gone_within(dialog, CLOSE)) {
        let line = format!("{reason}; it could not be closed, so the test binary aborts\n");
        let _ = std::io::stderr().write_all(line.as_bytes());
        std::process::abort();
    }
    reason
}

fn drive(dialog: HWND, answer: Answer) -> Result<(), String> {
    match answer {
        Answer::Type(text) => {
            wait_until_ready(dialog, None, false)?;
            wait_until_ready(dialog, Some(&text), true)?;
            click(dialog, IDOK)
        }
        Answer::Accept(name) => {
            wait_until_ready(dialog, Some(&name), false)?;
            click(dialog, IDOK)
        }
        Answer::Cancel => {
            wait_until_ready(dialog, None, false)?;
            click(dialog, IDCANCEL)
        }
    }
}

/// Answers the dialog titled `title` from a helper thread.
pub fn answer(title: &str, answer: Answer) -> JoinHandle<Result<(), String>> {
    let title = title.to_owned();
    thread::spawn(move || {
        let dialog =
            find_dialog(&title).ok_or_else(|| format!("no dialog titled {title:?} appeared"))?;
        if let Err(reason) = drive(dialog, answer) {
            return Err(give_up(dialog, &reason));
        }
        if gone_within(dialog, CLOSE) {
            Ok(())
        } else {
            Err(give_up(
                dialog,
                "the dialog stayed open after it was answered",
            ))
        }
    })
}
