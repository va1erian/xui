#![forbid(unsafe_code)]

//! Unit tests for the pure colour model.

use std::collections::HashSet;

use proptest::prelude::*;

use super::super::model::{
    BASIC_COLORS, Hsl, Hsv, format_cmyk, format_hex, format_hsl, format_hsv, format_rgb,
    parse_cmyk, parse_hex, parse_hsl, parse_hsv, parse_rgb,
};
use crate::Color;

#[test]
fn the_reference_colour_matches_the_screenshot() {
    let hsv = Hsv::from_color(Color::hex(0xEB_40_34));
    assert_eq!(format_hex(hsv.to_color()), "#eb4034");
    assert_eq!(format_rgb(hsv.to_color()), "235, 64, 52");
    assert_eq!(format_cmyk(hsv), "0%, 73%, 78%, 8%");
    assert_eq!(format_hsv(hsv), "4°, 78%, 92%");
    assert_eq!(format_hsl(hsv), "4°, 82%, 56%");
}

#[test]
fn the_reference_strings_parse_back_to_their_own_form() {
    assert_eq!(format_hex(parse_hex("#eb4034").unwrap()), "#eb4034");
    assert_eq!(format_rgb(parse_rgb("235, 64, 52").unwrap()), "235, 64, 52");
    assert_eq!(
        format_cmyk(parse_cmyk("0%, 73%, 78%, 8%").unwrap()),
        "0%, 73%, 78%, 8%"
    );
    assert_eq!(
        format_hsv(parse_hsv("4°, 78%, 92%").unwrap()),
        "4°, 78%, 92%"
    );
    assert_eq!(
        format_hsl(parse_hsl("4°, 82%, 56%").unwrap()),
        "4°, 82%, 56%"
    );
}

proptest! {
    #[test]
    fn rgb_round_trips_through_hsv(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let color = Color::rgb(r, g, b);
        prop_assert_eq!(Hsv::from_color(color).to_color(), color);
    }

    #[test]
    fn hex_round_trips_any_colour(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let color = Color::rgb(r, g, b);
        prop_assert_eq!(parse_hex(&format_hex(color)), Some(color));
    }

    #[test]
    fn hsl_round_trips_through_hsv(h in 0.0f32..360.0, s in 0.0f32..1.0, l in 0.0f32..1.0) {
        let hsv = Hsl { h, s, l }.to_hsv();
        let back = hsv.to_hsl();
        prop_assert!((back.s - s).abs() < 1e-3, "s {} vs {}", back.s, s);
        prop_assert!((back.l - l).abs() < 1e-3, "l {} vs {}", back.l, l);
    }

    #[test]
    fn parsers_never_panic_on_arbitrary_text(text in ".*") {
        let _ = parse_hex(&text);
        let _ = parse_rgb(&text);
        let _ = parse_cmyk(&text);
        let _ = parse_hsv(&text);
        let _ = parse_hsl(&text);
    }
}

#[test]
fn hue_is_kept_when_saturation_or_value_is_zero() {
    assert_eq!(Hsv::new(200.0, 0.0, 0.5).h, 200.0);
    assert_eq!(Hsv::new(200.0, 0.5, 0.0).h, 200.0);
}

#[test]
fn hex_parsing_accepts_and_rejects_the_right_forms() {
    assert_eq!(parse_hex("fff"), Some(Color::rgb(255, 255, 255)));
    assert_eq!(parse_hex("  #AbC  "), Some(Color::rgb(0xAA, 0xBB, 0xCC)));
    assert_eq!(parse_hex("EB4034"), Some(Color::hex(0xEB_40_34)));
    for bad in [
        "", "#", "12", "#1234", "#12345", "#gg0000", "0x123456", "١٢٣", "ffg",
    ] {
        assert_eq!(parse_hex(bad), None, "{bad:?} must be rejected");
    }
}

#[test]
fn the_basic_palette_is_32_distinct_colours() {
    let set: HashSet<(u8, u8, u8)> = BASIC_COLORS.iter().map(|c| (c.r, c.g, c.b)).collect();
    assert_eq!(set.len(), 32);
}

#[test]
fn parsers_reject_out_of_range_and_malformed_input() {
    for bad in [
        "300, 0, 0",
        "1, 2",
        "1, 2, 3, 4",
        "a, b, c",
        "",
        "1;2;3",
        "1, 2, 3px",
    ] {
        assert_eq!(parse_rgb(bad), None, "rgb {bad:?}");
    }
    for bad in ["0%, 73%, 78%", "0%, 73%, 78%, 101%", "0%, 73%, 78%, x", ""] {
        assert_eq!(parse_cmyk(bad), None, "cmyk {bad:?}");
    }
    assert_eq!(parse_hsv("361°, 0%, 0%"), None);
    assert_eq!(parse_hsv("0°, 101%, 0%"), None);
    assert_eq!(parse_hsl("0°, 0%, 200%"), None);
}
