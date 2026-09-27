#![warn(missing_docs)]

//! The Win32 backend for xui: [`Win32Backend`], the Win32 implementation of
//! [`xui_core::backend::Backend`], which runs the portable `xui-core` widgets
//! with native window chrome and Direct2D/DirectWrite painting (GDI fallback).
//! This crate has no widget API of its own — applications depend on `xui-core`
//! (or the `xui` umbrella crate) and pass a [`Win32Backend`] to
//! `xui_core::run_app`. See the workspace docs, *Backends*.
//!
//! The crate is deliberately split so that `unsafe` is confined to [`sys`]:
//! every other module starts with `#![forbid(unsafe_code)]` and talks to
//! Win32 only through the safe functions that [`sys`] exposes.
//!
//! # Shape of the API
//!
//! * [`Window`] wraps an `HWND` and a [`WindowHandler`]; messages arrive as a
//!   typed [`Message`] instead of raw `(u32, WPARAM, LPARAM)` triples. This is
//!   the platform layer [`Win32Backend`] is built on; it is also reachable
//!   directly for interop.
//! * [`gdi`] provides RAII handles ([`gdi::Font`], [`gdi::Brush`],
//!   [`gdi::Pen`], [`gdi::Bitmap`]) and a double-buffered [`gdi::Paint`]
//!   context, so no manual `DeleteObject` bookkeeping is required.
//! * [`d2d`] is the Direct2D/DirectWrite layer [`Win32Backend`] paints
//!   portable widgets through.

// The crate is Win32-only. On other targets it compiles to an empty crate so
// that dependants (e.g. a cross-platform workspace) can still `cargo check`.
#![cfg(windows)]

pub mod backend;
pub mod capture;
mod color;
mod error;
mod geometry;
mod hwnd;
mod layout;
mod message;
mod theme;
mod units;
mod window;

mod controls;
pub mod clipboard;
pub mod d2d;
pub mod gdi;
pub mod gl;
pub mod imaging;
pub mod looper;
mod sys;

pub use backend::Win32Backend;
pub use capture::RgbaImage;
pub use color::Color;
pub use error::{CaptureError, Error, ImagingError, Result, Win32Error};
pub use geometry::{Point, Rect, Size};
/// The OpenGL binding a canvas GL widget draws with, re-exported so an
/// implementor names the exact version this crate links against.
pub use glow;
pub use hwnd::Hwnd;
pub use layout::{Anchor, Dock, DockLayout, Insets, Stack, StackDirection, StackSlot};
pub use message::{
    Command, CommandNotification, HitTest, Key, LResult, Message, MinMaxInfo, Modifiers,
    MouseButton, Notify, TimerId,
};
pub use theme::{SystemTheme, Theme, Themed, is_theme_change};
pub use units::{Dip, Px, dip};
pub use window::{
    Backdrop, CursorShape, Icon, MonitorInfo, Placement, ShowState, Window, WindowClass,
    WindowExStyle, WindowHandler, WindowStyle, monitor_of, monitor_work_areas, monitors,
};
pub use xui_core::property::{Properties, Property, Value};

pub use looper::{quit, run, run_modal};

/// Everything a frontend typically needs, in one `use`.
///
/// Each module owns its own list in `prelude`, so adding a public item is a
/// one-line change in the module that defines it.
pub mod prelude {
    pub use crate::capture::prelude::*;
    pub use crate::color::prelude::*;
    pub use crate::error::prelude::*;
    pub use crate::geometry::prelude::*;
    pub use crate::hwnd::prelude::*;
    pub use crate::imaging::prelude::*;
    pub use crate::layout::prelude::*;
    pub use crate::looper::prelude::*;
    pub use crate::message::prelude::*;
    pub use crate::theme::prelude::*;
    pub use crate::units::prelude::*;
    pub use crate::window::prelude::*;

    pub use crate::{clipboard, gdi, looper};
}

/// Performs one-time process initialisation: per-monitor-v2 DPI awareness.
/// Idempotent; safe to call before creating any window.
pub fn init() {
    sys::dpi::set_per_monitor_v2();
}
