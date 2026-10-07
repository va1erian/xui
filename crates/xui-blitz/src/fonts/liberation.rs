//! The Liberation fonts (2.1.5, SIL Open Font License 1.1, `assets/liberation/`)
//! shipped with the view: Sans, Serif and Mono, each regular, bold, italic and
//! bold italic.
//!
//! They are metric-compatible with Arial, Times New Roman and Courier New, so a
//! page breaks its lines where a browser on Windows does, and the same way on
//! every platform. They are stored as WOFF2 (the same glyphs, compressed:
//! 1.7 MB instead of 4.3 MB) and decoded once per process.

use std::sync::{Arc, OnceLock};

/// The family names the faces register under.
pub(crate) const SANS: &str = "Liberation Sans";
pub(crate) const SERIF: &str = "Liberation Serif";
pub(crate) const MONO: &str = "Liberation Mono";

/// Families pages name that Liberation stands in for, metric for metric,
/// where the system has none of its own (Arial is Helvetica's metric twin).
pub(crate) const ALIASES: [(&str, &str); 6] = [
    ("Arial", SANS),
    ("Helvetica", SANS),
    ("Times New Roman", SERIF),
    ("Times", SERIF),
    ("Courier New", MONO),
    ("Courier", MONO),
];

macro_rules! faces {
    ($($family:expr => $file:literal),+ $(,)?) => {
        [$(($family, include_bytes!(concat!("../../assets/liberation/", $file)) as &[u8])),+]
    };
}

const WOFF2: [(&str, &[u8]); 12] = faces![
    SANS => "LiberationSans-Regular.woff2",
    SANS => "LiberationSans-Bold.woff2",
    SANS => "LiberationSans-Italic.woff2",
    SANS => "LiberationSans-BoldItalic.woff2",
    SERIF => "LiberationSerif-Regular.woff2",
    SERIF => "LiberationSerif-Bold.woff2",
    SERIF => "LiberationSerif-Italic.woff2",
    SERIF => "LiberationSerif-BoldItalic.woff2",
    MONO => "LiberationMono-Regular.woff2",
    MONO => "LiberationMono-Bold.woff2",
    MONO => "LiberationMono-Italic.woff2",
    MONO => "LiberationMono-BoldItalic.woff2",
];

/// One face as TrueType, with its family.
pub(crate) type Face = (&'static str, Arc<Vec<u8>>);

/// Every face.
pub(crate) fn faces() -> &'static [Face] {
    static FACES: OnceLock<Vec<Face>> = OnceLock::new();
    FACES.get_or_init(|| {
        WOFF2
            .iter()
            .map(|&(family, woff2)| {
                (
                    family,
                    Arc::new(blitz_dom::decode_font_bytes(woff2).into_owned()),
                )
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_decodes_to_truetype() {
        for (family, ttf) in faces() {
            assert_eq!(&ttf[..4], &[0, 1, 0, 0], "{family}");
        }
    }
}
