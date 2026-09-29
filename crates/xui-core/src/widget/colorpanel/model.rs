#![forbid(unsafe_code)]

//! The pure colour model behind [`ColorPanel`](super::ColorPanel): an HSV
//! triple as the source of truth, conversions to and from RGB, HSL and CMYK,
//! hex parsing/formatting and the 32-colour basic palette.
//!
//! Nothing here allocates in a conversion; the string parsers and formatters
//! build a small `String`/token list for the editable boxes, which are not a
//! paint hot path.

use crate::Color;

/// The 32 colours of the panel's **Simple** grid, eight columns by four rows:
/// a neutral ramp, the saturated primaries and secondaries, dark shades and
/// light tints. Every entry is distinct.
pub const BASIC_COLORS: [Color; 32] = [
    // Neutrals: black to white.
    Color::hex(0x00_00_00),
    Color::hex(0x40_40_40),
    Color::hex(0x80_80_80),
    Color::hex(0xBF_BF_BF),
    Color::hex(0xFF_FF_FF),
    Color::hex(0x59_59_59),
    Color::hex(0xA6_A6_A6),
    Color::hex(0xE6_E6_E6),
    // Saturated primaries and secondaries.
    Color::hex(0xFF_00_00),
    Color::hex(0xFF_80_00),
    Color::hex(0xFF_FF_00),
    Color::hex(0x00_FF_00),
    Color::hex(0x00_FF_FF),
    Color::hex(0x00_00_FF),
    Color::hex(0x80_00_FF),
    Color::hex(0xFF_00_FF),
    // Dark shades.
    Color::hex(0x80_00_00),
    Color::hex(0x80_40_00),
    Color::hex(0x80_80_00),
    Color::hex(0x00_80_00),
    Color::hex(0x00_80_80),
    Color::hex(0x00_00_80),
    Color::hex(0x40_00_80),
    Color::hex(0x80_00_80),
    // Light tints.
    Color::hex(0xFF_80_80),
    Color::hex(0xFF_BF_80),
    Color::hex(0xFF_FF_80),
    Color::hex(0x80_FF_80),
    Color::hex(0x80_FF_FF),
    Color::hex(0x80_80_FF),
    Color::hex(0xBF_80_FF),
    Color::hex(0xFF_80_FF),
];

/// A hue (`0..360`), saturation (`0..1`) and value (`0..1`) triple.
///
/// The panel keeps this, not an RGB value, as its source of truth: hue is
/// preserved while saturation or value is zero, so dragging through black or
/// white does not snap the hue back to red.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    /// Hue in degrees, `0..360`.
    pub h: f32,
    /// Saturation, `0..1`.
    pub s: f32,
    /// Value (brightness), `0..1`.
    pub v: f32,
}

impl Hsv {
    /// A triple; the channels are clamped to their ranges.
    pub const fn new(h: f32, s: f32, v: f32) -> Hsv {
        Hsv {
            h: clamp(h, 0.0, 360.0),
            s: clamp(s, 0.0, 1.0),
            v: clamp(v, 0.0, 1.0),
        }
    }

    /// The HSV triple for `color`.
    pub fn from_color(color: Color) -> Hsv {
        let r = color.r as f32 / 255.0;
        let g = color.g as f32 / 255.0;
        let b = color.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta <= f32::EPSILON {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };
        Hsv {
            h: if hue < 0.0 { hue + 360.0 } else { hue },
            s: if max <= f32::EPSILON {
                0.0
            } else {
                delta / max
            },
            v: max,
        }
    }

    /// The colour this triple describes. Hue `360` wraps to `0`.
    pub fn to_color(self) -> Color {
        let h = self.h.rem_euclid(360.0);
        let s = clamp(self.s, 0.0, 1.0);
        let v = clamp(self.v, 0.0, 1.0);
        let c = v * s;
        let hh = h / 60.0;
        let x = c * (1.0 - ((hh % 2.0) - 1.0).abs());
        let (r, g, b) = match hh as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        let channel = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Color::rgb(channel(r), channel(g), channel(b))
    }

    /// The HSL view of this triple (used for the editable `HSL` box).
    pub(crate) fn to_hsl(self) -> Hsl {
        let l = self.v * (1.0 - self.s / 2.0);
        let saturation = if l <= 0.0 || l >= 1.0 {
            0.0
        } else {
            (self.v - l) / l.min(1.0 - l)
        };
        Hsl {
            h: self.h,
            s: saturation,
            l,
        }
    }

    /// The CMYK view of this triple (used for the editable `CMYK` box).
    pub(crate) fn to_cmyk(self) -> Cmyk {
        let color = self.to_color();
        let r = color.r as f32 / 255.0;
        let g = color.g as f32 / 255.0;
        let b = color.b as f32 / 255.0;
        let k = 1.0 - r.max(g).max(b);
        let denominator = 1.0 - k;
        let convert = |channel: f32| {
            if denominator <= f32::EPSILON {
                0.0
            } else {
                (1.0 - channel - k) / denominator
            }
        };
        Cmyk {
            c: percent(convert(r)),
            m: percent(convert(g)),
            y: percent(convert(b)),
            k: percent(k),
        }
    }
}

/// An HSL triple; a derived view of [`Hsv`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hsl {
    /// Hue in degrees, `0..360`.
    pub h: f32,
    /// Saturation, `0..1`.
    pub s: f32,
    /// Lightness, `0..1`.
    pub l: f32,
}

impl Hsl {
    /// The HSV triple for this HSL view.
    pub fn to_hsv(self) -> Hsv {
        let v = self.l + self.s * self.l.min(1.0 - self.l);
        let s = if v <= f32::EPSILON {
            0.0
        } else {
            2.0 * (1.0 - self.l / v)
        };
        Hsv::new(self.h, s, v)
    }
}

/// A CMYK view with integer percentages, rounded to the nearest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cmyk {
    /// Cyan percent, `0..=100`.
    pub c: u8,
    /// Magenta percent, `0..=100`.
    pub m: u8,
    /// Yellow percent, `0..=100`.
    pub y: u8,
    /// Key (black) percent, `0..=100`.
    pub k: u8,
}

impl Cmyk {
    /// The HSV triple for this CMYK view.
    pub fn to_hsv(self) -> Hsv {
        let c = self.c as f32 / 100.0;
        let m = self.m as f32 / 100.0;
        let y = self.y as f32 / 100.0;
        let k = self.k as f32 / 100.0;
        let channel = |value: f32| ((1.0 - value) * (1.0 - k) * 255.0).round() as u8;
        Hsv::from_color(Color::rgb(channel(c), channel(m), channel(y)))
    }
}

/// Parses `#rgb`, `#rrggbb`, with or without `#`, case-insensitively, trimmed.
pub(crate) fn parse_hex(text: &str) -> Option<Color> {
    let text = text.trim();
    let digits = text.strip_prefix('#').unwrap_or(text).trim();
    if !digits.is_ascii() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        3 => {
            let mut channels = [0u8; 3];
            for (index, byte) in digits.bytes().enumerate() {
                channels[index] = hex(byte)? * 17;
            }
            Some(Color::rgb(channels[0], channels[1], channels[2]))
        }
        6 => Some(Color::rgb(
            two_hex(digits, 0)?,
            two_hex(digits, 2)?,
            two_hex(digits, 4)?,
        )),
        _ => None,
    }
}

/// Formats `color` as lowercase `#rrggbb`.
pub(crate) fn format_hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Parses `r, g, b` with channels in `0..=255` (commas or spaces between).
pub(crate) fn parse_rgb(text: &str) -> Option<Color> {
    let channels = tokens::<3>(text)?;
    Some(Color::rgb(
        integer(channels[0], 255)? as u8,
        integer(channels[1], 255)? as u8,
        integer(channels[2], 255)? as u8,
    ))
}

/// Formats `color` as `r, g, b`.
pub(crate) fn format_rgb(color: Color) -> String {
    format!("{}, {}, {}", color.r, color.g, color.b)
}

/// Parses `c%, m%, y%, k%` with percentages in `0..=100`.
pub(crate) fn parse_cmyk(text: &str) -> Option<Hsv> {
    let fields = tokens::<4>(text)?;
    let cmyk = Cmyk {
        c: percentage(fields[0])?,
        m: percentage(fields[1])?,
        y: percentage(fields[2])?,
        k: percentage(fields[3])?,
    };
    Some(cmyk.to_hsv())
}

/// Formats `hsv` as `c%, m%, y%, k%`.
pub(crate) fn format_cmyk(hsv: Hsv) -> String {
    let cmyk = hsv.to_cmyk();
    format!("{}%, {}%, {}%, {}%", cmyk.c, cmyk.m, cmyk.y, cmyk.k)
}

/// Parses `h°, s%, v%` with hue in `0..=360` and percentages in `0..=100`.
pub(crate) fn parse_hsv(text: &str) -> Option<Hsv> {
    let fields = tokens::<3>(text)?;
    Some(Hsv::new(
        degrees(fields[0])? as f32,
        percentage(fields[1])? as f32 / 100.0,
        percentage(fields[2])? as f32 / 100.0,
    ))
}

/// Formats `hsv` as `h°, s%, v%`.
pub(crate) fn format_hsv(hsv: Hsv) -> String {
    format!(
        "{}°, {}%, {}%",
        hue_degrees(hsv.h),
        percent(hsv.s),
        percent(hsv.v)
    )
}

/// Parses `h°, s%, l%` with hue in `0..=360` and percentages in `0..=100`.
pub(crate) fn parse_hsl(text: &str) -> Option<Hsv> {
    let fields = tokens::<3>(text)?;
    Some(
        Hsl {
            h: degrees(fields[0])? as f32,
            s: percentage(fields[1])? as f32 / 100.0,
            l: percentage(fields[2])? as f32 / 100.0,
        }
        .to_hsv(),
    )
}

/// Formats `hsv` as `h°, s%, l%`.
pub(crate) fn format_hsl(hsv: Hsv) -> String {
    let hsl = hsv.to_hsl();
    format!(
        "{}°, {}%, {}%",
        hue_degrees(hsl.h),
        percent(hsl.s),
        percent(hsl.l)
    )
}

/// The hue rounded to whole degrees and wrapped into `0..360`.
fn hue_degrees(hue: f32) -> i64 {
    (hue.rem_euclid(360.0).round() as i64).rem_euclid(360)
}

/// A `0..1` fraction as a rounded `0..=100` percent.
fn percent(value: f32) -> u8 {
    (value * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Splits on commas or whitespace into exactly `N` non-empty tokens.
fn tokens<const N: usize>(text: &str) -> Option<[&str; N]> {
    let mut out = [""; N];
    let mut count = 0;
    for piece in text.split(|c: char| c == ',' || c.is_whitespace()) {
        if piece.is_empty() {
            continue;
        }
        if count == N {
            return None;
        }
        out[count] = piece;
        count += 1;
    }
    (count == N).then_some(out)
}

/// A plain non-negative decimal integer no greater than `max`.
fn integer(token: &str, max: u32) -> Option<u32> {
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let value: u32 = token.parse().ok()?;
    (value <= max).then_some(value)
}

/// An integer `0..=100` with an optional trailing `%`.
fn percentage(token: &str) -> Option<u8> {
    integer(token.strip_suffix('%').unwrap_or(token), 100).map(|value| value as u8)
}

/// An integer `0..=360` with an optional trailing `°`.
fn degrees(token: &str) -> Option<u32> {
    integer(token.strip_suffix('°').unwrap_or(token), 360)
}

/// The value of one ASCII hex digit.
fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|digit| digit as u8)
}

/// The byte at `start..start + 2` of `text` read as hex.
fn two_hex(text: &str, start: usize) -> Option<u8> {
    u8::from_str_radix(text.get(start..start + 2)?, 16).ok()
}

/// Clamps a float, treating NaN as the lower bound.
const fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
