//! The OpenGL boundary of the shared seam: the offscreen framebuffer whose
//! `unsafe` calls render a widget into a texture and read the pixels back. All
//! `unsafe` in this crate lives here.

mod offscreen;

pub(crate) use offscreen::{Offscreen, supports_framebuffers};
