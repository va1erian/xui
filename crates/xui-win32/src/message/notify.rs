#![forbid(unsafe_code)]

//! Notifications routed from child windows (`WM_NOTIFY`) and timer ids.

use crate::hwnd::Hwnd;

/// A `SetTimer` identifier. Shared with the backend contract so a timer id is
/// portable.
pub use xui_core::backend::TimerId;

/// A notification routed from a child window (`WM_NOTIFY`), left for the
/// caller to interpret from the raw `NMHDR`.
#[derive(Clone, Copy, Debug)]
pub struct Notify {
    /// The control's id.
    pub id: usize,
    /// The raw `NMHDR.code`.
    pub code: u32,
    /// The control that sent it.
    pub hwnd: Hwnd,
}
