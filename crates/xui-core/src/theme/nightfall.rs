#![forbid(unsafe_code)]

//! The Nightfall palette: Midnight's decoration on near-neutral greys.

use super::Theme;
use crate::Color;
use crate::backend::Rgba;

impl Theme {
    /// Nightfall: [`Theme::midnight`]'s gradients, bevels, gloss and glow on
    /// greys with only a trace of blue, close to Windows 11's dark surfaces
    /// (`#202020` to `#2D2D2D`), with the green accent of
    /// [`Theme::daylight`] lightened so it reads on dark controls. It is the
    /// dark half of the Daylight pair.
    pub const fn nightfall() -> Theme {
        Theme {
            is_dark: true,
            background: Color::hex(0x26_27_2A),
            background_end: Color::hex(0x1C_1D_20),
            surface: Color::hex(0x2D_2E_33),
            surface_end: Color::hex(0x27_28_2B),
            raised: Color::hex(0x30_31_35),
            text: Color::hex(0xEC_ED_F0),
            text_secondary: Color::hex(0xA8_AA_B0),
            text_disabled: Color::hex(0x6A_6C_73),
            // Dark ink: the light green accent is too bright for light text.
            text_on_accent: Color::hex(0x14_18_24),
            accent: Color::hex(0x60_93_77),
            warning: Color::hex(0xF2_C1_4E),
            danger: Color::hex(0xFF_8A_95),
            selection: Color::hex(0x37_3D_48),
            selection_unfocused: Color::hex(0x35_36_3A),
            hover: Color::hex(0x38_39_3E),
            pressed: Color::hex(0x2A_2B_2F),
            border: Color::hex(0x3C_3D_42),
            border_focused: Color::hex(0x60_93_77),
            shadow: Color::hex(0x00_00_00),
            input_background: Color::hex(0x1A_1B_1E),
            input_border: Color::hex(0x4C_4E_55),
            scrollbar: Color::hex(0x63_65_6D),
            scrollbar_track: Color::hex(0x26_27_2A),
            track: Color::hex(0x44_46_4D),
            scrim: Rgba::with_alpha(0x00, 0x00, 0x00, 0x99),
            bevel: Rgba::with_alpha(0xFF, 0xFF, 0xFF, 0x24),
            shade: 34,
            gloss: 30,
            corner_radius: 8,
            glow: 0x70,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;
    use crate::Color;

    /// Nightfall keeps Midnight's decoration, so only the colours differ.
    #[test]
    fn nightfall_is_decorated_like_midnight() {
        let (night, midnight) = (Theme::nightfall(), Theme::midnight());
        assert!(night.is_dark);
        assert_ne!(night.background_end, night.background);
        assert_ne!(night.surface_end, night.surface);
        assert_eq!(
            (night.bevel, night.shade, night.gloss),
            (midnight.bevel, midnight.shade, midnight.gloss)
        );
        assert_eq!(
            (night.corner_radius, night.glow),
            (midnight.corner_radius, midnight.glow)
        );
    }

    /// The surfaces are near-neutral: no channel strays more than a few steps
    /// from the others, where Midnight's navy leans well into blue.
    #[test]
    fn nightfall_surfaces_are_barely_tinted() {
        let spread = |c: Color| c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
        let t = Theme::nightfall();
        for surface in [
            t.background,
            t.background_end,
            t.surface,
            t.surface_end,
            t.raised,
            t.hover,
            t.pressed,
            t.border,
            t.input_background,
        ] {
            assert!(spread(surface) <= 6, "{surface:?} is near-neutral");
        }
        assert!(spread(Theme::midnight().background) > 20);
    }

    /// Text stays readable on every surface it is drawn on, top and bottom of
    /// each gradient, and controls stand out from the cards they sit on.
    #[test]
    fn nightfall_text_and_controls_read() {
        let t = Theme::nightfall();
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
        assert!(t.input_border.contrast_ratio(t.surface) > 1.3);
        assert!(t.input_border.contrast_ratio(t.input_background) > 1.3);
        assert_ne!(t.track, t.background);
        assert_ne!(t.input_background, t.surface);
        assert_eq!(t.scrollbar_track, t.background);
        assert_eq!(t.border_focused, t.accent);
    }
}
