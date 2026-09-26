#![forbid(unsafe_code)]

//! The portable description of a top-level window a backend creates.

/// Whether a window shows the platform's system title bar and border.
///
/// Set with [`PlatformSpec::decorations`]. [`Decorations::None`] removes the
/// system title bar so the application can draw its own. A backend that
/// supports an extended title bar (Win32) keeps the native window buttons and
/// resize borders, while one that does not leaves the whole frame to the
/// application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Decorations {
    /// The system title bar, border and window buttons (the default).
    #[default]
    System,
    /// No system title bar: the application draws its own.
    None,
}

/// The material a backend draws behind a window's client area.
///
/// Set with [`PlatformSpec::backdrop`]. A backend without the material, or one
/// where the platform rejects it, falls back to the opaque theme background —
/// exactly as on Windows when DWM declines the request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backdrop {
    /// An opaque theme background (the default).
    #[default]
    Opaque,
    /// A translucent, blurred material (Windows Acrylic).
    Acrylic,
    /// A desktop-tinted material (Windows Mica).
    Mica,
}

/// The portable part of a top-level window's spec.
#[derive(Clone, Debug, PartialEq)]
pub struct PlatformSpec {
    /// The window title.
    pub title: String,
    /// The initial client width as a design value.
    pub width: Dip,
    /// The initial client height as a design value.
    pub height: Dip,
    /// Whether the user may resize the window.
    pub resizable: bool,
    /// Whether the window shows the platform's system title bar.
    pub decorations: Decorations,
    /// The height of a custom caption band reserved at the top of the client
    /// area, as a design value. Zero (the default) when the application draws
    /// no caption. Read the band a backend actually reserved with
    /// [`Backend::caption_inset`](super::Backend::caption_inset).
    pub caption_inset: Dip,
    /// The material behind the client area.
    pub backdrop: Backdrop,
}

use crate::units::Dip;

impl PlatformSpec {
    /// A resizable window of a default size.
    pub fn new(title: impl Into<String>) -> PlatformSpec {
        PlatformSpec {
            title: title.into(),
            width: Dip(640.0),
            height: Dip(480.0),
            resizable: true,
            decorations: Decorations::System,
            caption_inset: Dip(0.0),
            backdrop: Backdrop::Opaque,
        }
    }

    /// Sets the initial client size.
    pub fn size(mut self, width: Dip, height: Dip) -> PlatformSpec {
        self.width = width;
        self.height = height;
        self
    }

    /// Hides the system title bar so the application draws its own.
    pub fn decorations(mut self, decorations: Decorations) -> PlatformSpec {
        self.decorations = decorations;
        self
    }

    /// Reserves a custom caption band of `height` at the top of the client
    /// area, so a title bar can sit in the title area.
    pub fn caption_inset(mut self, height: Dip) -> PlatformSpec {
        self.caption_inset = height;
        self
    }

    /// Selects the material behind the client area.
    pub fn backdrop(mut self, backdrop: Backdrop) -> PlatformSpec {
        self.backdrop = backdrop;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Backdrop, Decorations, PlatformSpec};
    use crate::units::dip;

    #[test]
    fn a_new_window_is_decorated_and_opaque_by_default() {
        let spec = PlatformSpec::new("t");
        assert_eq!(spec.decorations, Decorations::System);
        assert_eq!(spec.caption_inset, dip(0.0));
        assert_eq!(spec.backdrop, Backdrop::Opaque);
    }

    #[test]
    fn builder_sets_the_portable_chrome_options() {
        let spec = PlatformSpec::new("t")
            .decorations(Decorations::None)
            .caption_inset(dip(36.0))
            .backdrop(Backdrop::Mica);
        assert_eq!(spec.decorations, Decorations::None);
        assert_eq!(spec.caption_inset, dip(36.0));
        assert_eq!(spec.backdrop, Backdrop::Mica);
    }
}
