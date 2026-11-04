#![forbid(unsafe_code)]

//! A native child window a node owns, for a GPU API to present into.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// A window a GPU API can make a swap chain on: its handles, and safe to hand
/// to the thread that renders.
pub trait NativeWindow: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + ?Sized> NativeWindow for T {}

/// A node's own native child window, from
/// [`Backend::native_surface`](super::Backend::native_surface).
///
/// The backend keeps it on the node's bounds and visibility and passes its
/// pointer input to the node as if it fell on the node itself. What is drawn
/// in it is the holder's: the backend paints nothing there, and its frames
/// reach the screen without passing through the software composite. A clone
/// keeps the OS window alive; once the node gives the surface up the backend
/// hides it, and it is destroyed when the last clone goes.
pub type NativeSurface = Arc<dyn NativeWindow>;
