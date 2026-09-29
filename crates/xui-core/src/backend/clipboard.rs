#![forbid(unsafe_code)]

//! The in-process text clipboard a backend falls back to.
//!
//! [`Backend::clipboard_text`](super::Backend::clipboard_text) and
//! [`Backend::set_clipboard_text`](super::Backend::set_clipboard_text) default
//! to this store, so a backend with no OS clipboard of its own — the headless
//! and offscreen backends, and any minimal implementation — still supports
//! copy and paste within the process. A backend with a real clipboard (Win32,
//! canvas) overrides both.
//!
//! The store is thread-local, so tests on different threads never see each
//! other's text.

use std::cell::RefCell;

thread_local! {
    /// The fallback clipboard. `None` means the clipboard is empty, which is
    /// also what [`get`] reports for an empty string.
    static TEXT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The fallback clipboard's current text, or `None` when it is empty.
pub(crate) fn get() -> Option<String> {
    TEXT.with(|slot| slot.borrow().clone())
        .filter(|text| !text.is_empty())
}

/// Replaces the fallback clipboard's text.
pub(crate) fn set(text: &str) {
    TEXT.with(|slot| *slot.borrow_mut() = Some(text.to_string()));
}
