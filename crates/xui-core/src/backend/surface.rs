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
/// The backend keeps it on the node's bounds and passes its pointer input to
/// the node as if it fell on the node itself. It is shown only while the node
/// is visible, lies wholly inside its clips and has no popup over it, since a
/// window cannot be clipped as a painter is and a popup is drawn under it; the
/// node's painter shows meanwhile. What is drawn in it is the holder's: its
/// frames reach the screen without passing through the software composite.
///
/// A clone keeps the OS window alive. Once the node gives the surface up (or
/// is removed, or its window closes) the backend hides it and refuses its
/// handles from then on (`HandleError::Unavailable`), so a holder makes no new
/// swap chain on it; one made before keeps working until the window it sits in
/// closes, and then fails to present, and should be dropped.
pub type NativeSurface = Arc<dyn NativeWindow>;
