#![forbid(unsafe_code)]

//! The Midnight palette: a navy dark theme with vertical gradients, bevelled
//! cards and controls, and an accent glow on selections.

use super::Theme;
use crate::Color;
use crate::backend::Rgba;

impl Theme {
    /// Midnight: navy surfaces that darken toward the bottom, rounded cards
    /// with a 1px top highlight, and a soft glow around checked indicators.
    ///
    /// Widgets draw on their container instead of painting a box of their own
    /// where the backend composites parents first (see
    /// [`Canvas::composites_parents`](crate::Canvas::composites_parents)), so
    /// labels and indicators sit directly on the window or card gradient.
    pub const fn midnight() -> Theme {
        Theme {
            is_dark: true,
            background: Color::hex(0x23_2A_40),
            background_end: Color::hex(0x1A_1F_30),
            surface: Color::hex(0x2A_32_50),
            surface_end: Color::hex(0x24_2B_44),
            raised: Color::hex(0x2E_37_58),
            text: Color::hex(0xE4_E8_F5),
            text_secondary: Color::hex(0x9A_A3_C2),
            text_disabled: Color::hex(0x5F_67_86),
            text_on_accent: Color::hex(0xFF_FF_FF),
            accent: Color::hex(0x4C_BF_82),
            warning: Color::hex(0xF2_C1_4E),
            danger: Color::hex(0xFF_8A_95),
            selection: Color::hex(0x31_45_74),
            selection_unfocused: Color::hex(0x33_3C_5C),
            hover: Color::hex(0x35_3F_63),
            pressed: Color::hex(0x27_2E_4A),
            border: Color::hex(0x3A4364),
            border_focused: Color::hex(0x4C_BF_82),
            shadow: Color::hex(0x00_00_00),
            input_background: Color::hex(0x18_1C_2C),
            input_border: Color::hex(0x47_52_7A),
            scrollbar: Color::hex(0x5A_65_90),
            scrollbar_track: Color::hex(0x23_2A_40),
            track: Color::hex(0x3E_48_70),
            scrim: Rgba::with_alpha(0x05, 0x08, 0x14, 0x99),
            bevel: Rgba::with_alpha(0xFF, 0xFF, 0xFF, 0x14),
            shade: 28,
            corner_radius: 8,
            glow: 0x70,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;

    #[test]
    fn midnight_is_a_dark_gradient_theme() {
        let theme = Theme::midnight();
        assert!(theme.is_dark);
        assert_ne!(theme.background_end, theme.background);
        assert_ne!(theme.surface_end, theme.surface);
        assert!(theme.bevel.a > 0 && theme.shade > 0 && theme.glow > 0);
    }

    /// Text stays readable on every surface it is drawn on, top and bottom of
    /// each gradient.
    #[test]
    fn midnight_text_reads_on_every_surface() {
        let t = Theme::midnight();
        for surface in [
            t.background,
            t.background_end,
            t.surface,
            t.surface_end,
            t.raised,
        ] {
            assert!(t.text.contrast_ratio(surface) > 7.0, "body text");
            assert!(
                t.text_secondary.contrast_ratio(surface) > 4.5,
                "secondary text"
            );
        }
        assert!(t.text_on_accent.contrast_ratio(t.accent) > 2.0);
    }

    /// Inputs, grooves and borders stand out from the cards they sit on.
    #[test]
    fn midnight_controls_read_against_cards() {
        let t = Theme::midnight();
        assert!(t.input_border.contrast_ratio(t.surface) > 1.3);
        assert!(t.input_border.contrast_ratio(t.input_background) > 1.3);
        assert_ne!(t.track, t.background);
        assert_ne!(t.input_background, t.surface);
        assert_eq!(t.border_focused, t.accent);
    }
}
