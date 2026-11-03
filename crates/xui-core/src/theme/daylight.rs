#![forbid(unsafe_code)]

//! The Daylight palette: Midnight's decoration on warm off-white surfaces.

use super::Theme;
use crate::Color;
use crate::backend::Rgba;

impl Theme {
    /// Daylight: Midnight's design in a light palette. Warm off-white surfaces
    /// that darken toward the bottom, rounded cards with a 1px white highlight,
    /// glossy control faces and a soft green accent glow, so the light mode
    /// has the same glossy buttons, rounded groups and glowing indicators as
    /// the dark [`Theme::nightfall`] ([`Theme::light`] stays flat).
    pub const fn daylight() -> Theme {
        Theme {
            is_dark: false,
            background: Color::hex(0xF6_F4_F0),
            background_end: Color::hex(0xE8_E4_DC),
            surface: Color::hex(0xFD_FC_FA),
            surface_end: Color::hex(0xF0_ED_E7),
            raised: Color::hex(0xFF_FF_FF),
            text: Color::hex(0x1C_1A_17),
            text_secondary: Color::hex(0x5A_54_48),
            text_disabled: Color::hex(0xA6_A0_93),
            text_on_accent: Color::hex(0xE4_E8_F5),
            accent: Color::hex(0x2C_70_4A),
            warning: Color::hex(0x9D_5D_00),
            danger: Color::hex(0xC4_2B_1C),
            selection: Color::hex(0xD5_DB_E8),
            selection_unfocused: Color::hex(0xE2_DE_D6),
            hover: Color::hex(0xEA_E6_DE),
            pressed: Color::hex(0xD9_D4_CA),
            border: Color::hex(0xCF_C9_BC),
            border_focused: Color::hex(0x2C_70_4A),
            shadow: Color::hex(0x00_00_00),
            input_background: Color::hex(0xFF_FF_FF),
            input_border: Color::hex(0xA0_99_8A),
            scrollbar: Color::hex(0xB0_A8_99),
            scrollbar_track: Color::hex(0xF6_F4_F0),
            track: Color::hex(0xCD_C7_BA),
            scrim: Rgba::with_alpha(0x14, 0x10, 0x0A, 0x66),
            // Faint: a stronger white line shows on the coloured faces (radio
            // dots, check boxes, swatches).
            bevel: Rgba::with_alpha(0xFF, 0xFF, 0xFF, 0x28),
            shade: 30,
            // Gentle, so the light ink on a glossed accent face keeps its
            // contrast.
            gloss: 24,
            corner_radius: 8,
            glow: 0x58,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;
    use crate::theme::look;

    /// Daylight carries every decoration Nightfall does, so the two modes draw
    /// the same glossy, rounded controls.
    #[test]
    fn daylight_is_decorated_like_nightfall() {
        let (day, night) = (Theme::daylight(), Theme::nightfall());
        assert!(!day.is_dark);
        assert!(look::decorated(&day));
        assert_ne!(day.background_end, day.background);
        assert_ne!(day.surface_end, day.surface);
        assert!(day.bevel.a > 0 && day.shade > 0 && day.gloss > 0 && day.glow > 0);
        assert_eq!(day.corner_radius, night.corner_radius);
    }

    /// Text stays readable on every surface it is drawn on, top and bottom of
    /// each gradient, and fields stand out from the cards they sit on.
    #[test]
    fn daylight_text_and_fields_read() {
        let t = Theme::daylight();
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
        assert!(t.text_on_accent.contrast_ratio(t.accent) >= 4.5);
        assert!(t.input_border.contrast_ratio(t.surface) > 1.9);
        assert!(t.input_border.contrast_ratio(t.input_background) > 2.5);
        assert_ne!(t.track, t.background);
        assert_eq!(t.scrollbar_track, t.background);
        assert_eq!(t.border_focused, t.accent);
    }
}
