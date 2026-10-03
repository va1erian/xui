#![forbid(unsafe_code)]

//! Semantic colour tokens: the complete light/dark palette every control
//! paints from. See the README's *Theming* section for the model.

use crate::Color;
use crate::backend::Rgba;

/// Semantic colours used by the bundled controls.
///
/// Every colour is opaque except [`Theme::scrim`], which carries the alpha a
/// modal backdrop needs to blend over the content behind it. Palettes are
/// sampled from Windows 11's own light/dark apps (Explorer chrome and address
/// bar, Settings pages and cards, WinUI text/accent values on 24H2); each
/// constructor documents the source per token group. Per-control structs
/// (`ListViewTheme::from_theme`, …) are derived, overridable views over
/// these tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Whether this is the dark variant (drives `SetWindowTheme` and DWM).
    pub is_dark: bool,
    /// Window/panel background (Settings page: light `#F3F3F3`, dark `#202020`).
    pub background: Color,
    /// Slightly raised surface: toolbar, header, status bar (Explorer command
    /// bar: light `#F9F9F9`, dark `#2B2B2B`).
    pub surface: Color,
    /// Further raised surface: menus, popups, cards (light `#FFFFFF`, dark
    /// `#2D2D2D`).
    pub raised: Color,
    /// Primary text (WinUI TextPrimary: light `#1B1B1B`, dark `#FFFFFF`).
    pub text: Color,
    /// De-emphasised text (WinUI TextSecondary: light `#4A4846`, dark `#C7C7C7`).
    pub text_secondary: Color,
    /// Disabled text (WinUI disabled: light `#8A8886`, dark `#767676`).
    pub text_disabled: Color,
    /// Text drawn on top of [`Theme::accent`].
    pub text_on_accent: Color,
    /// Accent used for highlighted rows and active buttons (WinUI
    /// AccentDefault: light `#005FB8`, dark `#4CC2FF`).
    pub accent: Color,
    /// Caution fill, e.g. a paused progress bar (WinUI SystemFillColorCaution:
    /// light `#9D5D00`, dark `#FCE100`).
    pub warning: Color,
    /// Critical/danger fill, e.g. an error progress bar (WinUI
    /// SystemFillColorCritical: light `#C42B1C`, dark `#FF99A4`).
    pub danger: Color,
    /// Focused selection background (Explorer selected row: light `#B3D4F0`,
    /// dark `#2B4A67`).
    pub selection: Color,
    /// Unfocused selection background (Explorer grey: light `#D6D6D6`, dark
    /// `#3A3A3A`).
    pub selection_unfocused: Color,
    /// Hover fill (Explorer hover: light `#E3E3E3`, dark `#333333`).
    pub hover: Color,
    /// Pressed fill (Explorer pressed: light `#CFCFCF`, dark `#292929`).
    pub pressed: Color,
    /// Widget border / separators (WinUI CardStroke: light `#CFCFCF`, dark
    /// `#303030`).
    pub border: Color,
    /// Focused border (accent outline: matches [`Theme::accent`]).
    pub border_focused: Color,
    /// Base colour of the drop shadow an elevated, transient surface (menus,
    /// combo popups, tooltips) may cast. WinUI draws these shadows in black at
    /// a low opacity (light/dark `#000000`); the platform's own window shadow
    /// is used where it exists, so the portable painter draws none.
    pub shadow: Color,
    /// Input background: edits, address bar (light `#FFFFFF`, dark `#1F1F1F`).
    pub input_background: Color,
    /// Border around inputs: edits, combo fields, spin fields, check boxes
    /// (WinUI `ControlStrokeColorDefault`: light `#8A8A8A`, dark `#3F3F3F`).
    /// Distinct from [`Theme::border`] (card stroke), which is too faint to
    /// outline a field on the dark surface.
    pub input_border: Color,
    /// Scrollbar thumb (WinUI thumb: light `#8B8986`, dark `#605E5C`).
    pub scrollbar: Color,
    /// Scrollbar track (matches [`Theme::background`]): the scrollbar's own
    /// unfilled track is meant to blend into the page, since the thumb alone
    /// carries the affordance.
    pub scrollbar_track: Color,
    /// Slider/progress-bar groove (WinUI `ControlStrokeColorDefault`: light
    /// `#B4B4B4`, dark `#3F3F3F`). Unlike [`Theme::scrollbar_track`], a
    /// slider's or progress bar's unfilled groove is drawn over the plain
    /// background and must stay visible against it (#160).
    pub track: Color,
    /// The modal backdrop a [`Dialog`](crate::widget::Dialog) or
    /// [`TaskDialog`](crate::widget::TaskDialog) paints over the window while
    /// it is open: black at 40% alpha in light mode and 55% in dark mode, so
    /// the content stays visible but clearly recedes. Translucent rather than
    /// an opaque dimmed [`Theme::background`] so it blends with whatever is
    /// behind it on a compositing backend.
    pub scrim: Rgba,
    /// The window background's bottom colour: the background is a vertical
    /// gradient from [`Theme::background`] at the top to this at the bottom
    /// (equal to it for a flat theme).
    pub background_end: Color,
    /// The bottom of a card's (panel, group box, dialog) vertical gradient,
    /// from [`Theme::surface`] at the top (equal to it for a flat theme).
    pub surface_end: Color,
    /// A 1px highlight along the top edge of cards and raised controls, so
    /// they read as bevelled (transparent for a flat theme).
    pub bevel: Rgba,
    /// How far (0-255) a control's face darkens from its top to its bottom
    /// edge: buttons, check boxes, selected rows (0 for a flat theme).
    pub shade: u8,
    /// Corner radius of cards and group boxes in pixels (0: square, framed
    /// like a plain panel).
    pub corner_radius: u8,
    /// Opacity (0-255) of the accent glow around checked and selected
    /// indicators (0: none).
    pub glow: u8,
}

impl Theme {
    /// The light palette.
    pub const fn light() -> Theme {
        Theme {
            is_dark: false,
            background: Color::hex(0xF3_F3_F3),
            surface: Color::hex(0xF9_F9_F9),
            raised: Color::hex(0xFF_FF_FF),
            text: Color::hex(0x1B_1B_1B),
            text_secondary: Color::hex(0x4A_48_46),
            text_disabled: Color::hex(0x8A_88_86),
            text_on_accent: Color::hex(0xFF_FF_FF),
            accent: Color::hex(0x00_5F_B8),
            warning: Color::hex(0x9D_5D_00),
            danger: Color::hex(0xC4_2B_1C),
            selection: Color::hex(0xB3_D4_F0),
            selection_unfocused: Color::hex(0xD6_D6_D6),
            hover: Color::hex(0xE3_E3_E3),
            pressed: Color::hex(0xCF_CF_CF),
            border: Color::hex(0xCF_CF_CF),
            border_focused: Color::hex(0x00_5F_B8),
            shadow: Color::hex(0x00_00_00),
            input_background: Color::hex(0xFF_FF_FF),
            input_border: Color::hex(0x8A_8A_8A),
            scrollbar: Color::hex(0x8B_89_86),
            scrollbar_track: Color::hex(0xF3_F3_F3),
            track: Color::hex(0xB4_B4_B4),
            scrim: Rgba::with_alpha(0x00, 0x00, 0x00, 0x66),
            background_end: Color::hex(0xF3_F3_F3),
            surface_end: Color::hex(0xF9_F9_F9),
            bevel: Rgba::with_alpha(0, 0, 0, 0),
            shade: 0,
            corner_radius: 0,
            glow: 0,
        }
    }

    /// The dark palette.
    pub const fn dark() -> Theme {
        Theme {
            is_dark: true,
            background: Color::hex(0x20_20_20),
            surface: Color::hex(0x2B_2B_2B),
            raised: Color::hex(0x2D_2D_2D),
            text: Color::hex(0xFF_FF_FF),
            text_secondary: Color::hex(0xC7_C7_C7),
            text_disabled: Color::hex(0x76_76_76),
            text_on_accent: Color::hex(0x00_00_00),
            accent: Color::hex(0x4C_C2_FF),
            warning: Color::hex(0xFC_E1_00),
            danger: Color::hex(0xFF_99_A4),
            selection: Color::hex(0x2B_4A_67),
            selection_unfocused: Color::hex(0x3A_3A_3A),
            hover: Color::hex(0x33_33_33),
            pressed: Color::hex(0x29_29_29),
            border: Color::hex(0x30_30_30),
            border_focused: Color::hex(0x4C_C2_FF),
            shadow: Color::hex(0x00_00_00),
            input_background: Color::hex(0x1F_1F_1F),
            input_border: Color::hex(0x3F_3F_3F),
            scrollbar: Color::hex(0x60_5E_5C),
            scrollbar_track: Color::hex(0x20_20_20),
            track: Color::hex(0x3F_3F_3F),
            scrim: Rgba::with_alpha(0x00, 0x00, 0x00, 0x8C),
            background_end: Color::hex(0x20_20_20),
            surface_end: Color::hex(0x2B_2B_2B),
            bevel: Rgba::with_alpha(0, 0, 0, 0),
            shade: 0,
            corner_radius: 0,
            glow: 0,
        }
    }
}

impl Default for Theme {
    /// The light theme.
    fn default() -> Theme {
        Theme::light()
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;

    /// The original palettes stay flat: every decoration token is off, so
    /// apps that never asked for gradients paint exactly as before.
    #[test]
    fn light_and_dark_are_flat() {
        for theme in [Theme::light(), Theme::dark()] {
            assert_eq!(theme.background_end, theme.background);
            assert_eq!(theme.surface_end, theme.surface);
            assert_eq!(theme.bevel.a, 0);
            assert_eq!((theme.shade, theme.corner_radius, theme.glow), (0, 0, 0));
        }
    }

    #[test]
    fn light_and_dark_differ_in_kind() {
        assert!(!Theme::light().is_dark);
        assert!(Theme::dark().is_dark);
        assert_ne!(Theme::light(), Theme::dark());
    }

    #[test]
    fn tracks_follow_background() {
        assert_eq!(Theme::light().scrollbar_track, Theme::light().background);
        assert_eq!(Theme::dark().scrollbar_track, Theme::dark().background);
    }

    /// A slider/progress groove sits on the plain background and must read
    /// against it, unlike the scrollbar's own (intentionally blended) track
    /// (#160).
    #[test]
    fn the_slider_track_stays_visible_on_the_background() {
        for theme in [Theme::light(), Theme::dark()] {
            assert_ne!(
                theme.track, theme.background,
                "the groove must differ from the background it's painted on"
            );
        }
    }

    #[test]
    fn focused_borders_match_accent() {
        assert_eq!(Theme::light().border_focused, Theme::light().accent);
        assert_eq!(Theme::dark().border_focused, Theme::dark().accent);
    }

    #[test]
    fn status_fills_differ_between_variants() {
        assert_ne!(Theme::light().warning, Theme::dark().warning);
        assert_ne!(Theme::light().danger, Theme::dark().danger);
    }

    /// A field must stand out from every surface it can sit on, and its border
    /// must outline it against its own fill (#119).
    #[test]
    fn dark_inputs_read_as_fields() {
        let theme = Theme::dark();
        for surface in [theme.background, theme.surface, theme.raised] {
            assert_ne!(
                theme.input_background, surface,
                "the dark input fill must differ from every surface"
            );
        }
        assert!(
            theme.input_border.contrast_ratio(theme.input_background) > 1.3,
            "the dark input border must be visible over the field fill"
        );
        assert!(
            theme.input_border.contrast_ratio(theme.surface) > 1.3,
            "the dark input border must be visible over the surrounding surface"
        );
    }

    #[test]
    fn the_shadow_is_black_in_both_variants() {
        for theme in [Theme::light(), Theme::dark()] {
            assert_eq!(theme.shadow, crate::Color::rgb(0, 0, 0));
        }
    }

    /// The modal scrim is translucent black, and the dark variant dims harder
    /// so the same content recedes further against the darker background.
    #[test]
    fn the_scrim_is_translucent_black_in_both_variants() {
        use crate::backend::Rgba;
        let light = Theme::light().scrim;
        let dark = Theme::dark().scrim;
        assert_eq!(light, Rgba::with_alpha(0, 0, 0, 102));
        assert_eq!(dark, Rgba::with_alpha(0, 0, 0, 140));
        assert!(light.a > 0 && light.a < 255, "the scrim blends, not opaque");
        assert!(dark.a > light.a, "dark mode dims more than light");
    }
}
