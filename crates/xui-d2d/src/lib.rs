#![cfg(windows)]
#![warn(missing_docs)]

//! A lightweight Windows backend for xui.
//!
//! Where a native-control backend hands each widget to a Win32 control, this
//! one only borrows what Windows is best at: the **window** (one `HWND` per xui
//! window), the **message loop and input**, and **drawing and text** through
//! Direct2D and DirectWrite. Every widget is a painted node of a scene xui
//! keeps itself, exactly as on the software backend, so the same widgets look
//! and behave the same and none of them costs a window handle.
//!
//! Every module under `backend/` forbids `unsafe` (the crate root cannot, since
//! `sys` needs it); the raw Win32, Direct2D and DirectWrite calls are isolated in
//! `sys` behind a safe API.

mod backend;
mod sys;

pub use backend::D2dBackend;
