#![forbid(unsafe_code)]

//! The clipboard seam, and the in-process store that keeps formatting across
//! a copy and paste inside the application.
//!
//! The system clipboard only carries plain text. A copy also remembers the
//! rich [`Fragment`] in a thread-local, tagged with the plain text it wrote; a
//! paste uses the fragment only while the clipboard still holds exactly that
//! text, so a copy made in another program wins.

use std::cell::RefCell;

use crate::model::Fragment;

/// Text-only clipboard access.
pub trait Clipboard {
    /// The clipboard's current text, or `None` when it holds none.
    fn text(&self) -> Option<String>;

    /// Replaces the clipboard's text.
    fn set_text(&self, text: &str);
}

/// A clipboard held in memory, for tests and backends without one.
#[derive(Debug, Default)]
pub struct MemoryClipboard(RefCell<Option<String>>);

impl Clipboard for MemoryClipboard {
    fn text(&self) -> Option<String> {
        self.0.borrow().clone()
    }

    fn set_text(&self, text: &str) {
        *self.0.borrow_mut() = Some(text.to_owned());
    }
}

thread_local! {
    /// The last copied fragment and the plain text it was tagged with.
    static RICH: RefCell<Option<(String, Fragment)>> = const { RefCell::new(None) };
}

/// Remembers `fragment` as the rich form of the clipboard text `plain`.
pub(super) fn store(plain: &str, fragment: Fragment) {
    RICH.with(|slot| *slot.borrow_mut() = Some((plain.to_owned(), fragment)));
}

/// The remembered fragment, if the clipboard text is still the one it was
/// stored for.
pub(super) fn matching(plain: &str) -> Option<Fragment> {
    RICH.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|(tag, _)| tag == plain)
            .map(|(_, fragment)| fragment.clone())
    })
}
