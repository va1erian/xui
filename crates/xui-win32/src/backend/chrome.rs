#![forbid(unsafe_code)]

//! Maps the portable window-chrome options ([`PlatformSpec::decorations`],
//! [`PlatformSpec::caption_inset`], [`PlatformSpec::backdrop`]) onto the crate's
//! extended title bar and DWM backdrop support.
//!
//! A window without system decorations keeps the native minimize/maximize/close
//! buttons and resize borders (the extended title bar removes only the caption),
//! so a custom title bar drags and still has working window buttons.

use xui_core::Theme;
use xui_core::backend::{Backdrop, Decorations, PlatformSpec};

use crate::sys;
use crate::window::{Backdrop as Material, Window};

/// Applies `spec`'s chrome options to a freshly created top-level `window`.
pub(super) fn apply(spec: &PlatformSpec, window: &Window) {
    // The portable backend starts every window on the light theme; a later
    // `set_theme` re-applies the material for the dark variant.
    let theme = Theme::light();
    let material = match spec.backdrop {
        Backdrop::Opaque => Material::None,
        Backdrop::Acrylic => Material::Acrylic,
        Backdrop::Mica => Material::Mica,
    };
    let mut backdrop_active = sys::apply_backdrop(window.hwnd(), material, theme.is_dark);
    // A requested caption band is only realisable by removing the system
    // caption, so either option turns on the extended title bar.
    if spec.decorations == Decorations::None || spec.caption_inset.value() > 0.0 {
        backdrop_active &= sys::nc::enable_extended(window.hwnd());
        sys::apply_extended_colors(window.hwnd(), &theme, backdrop_active);
    }
    crate::theme::set_backdrop_active(window.hwnd(), backdrop_active);
}
