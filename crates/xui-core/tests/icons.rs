//! The public icon surface: the generated [`Lucide`] set matches the vendored
//! SVGs and the legacy [`Icon`]/[`Glyph`] vocabularies convert into [`IconRef`].

use std::collections::HashSet;
use std::path::Path;

use xui_core::{Glyph, Icon, IconRef, Lucide};

/// The number of `.svg` files vendored under `assets/lucide/`.
fn vendored_svgs() -> usize {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/lucide");
    std::fs::read_dir(&dir)
        .expect("assets/lucide")
        .filter(|entry| {
            entry.as_ref().is_ok_and(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "svg")
            })
        })
        .count()
}

#[test]
fn every_variant_matches_a_vendored_svg() {
    assert_eq!(
        Lucide::ALL.len(),
        vendored_svgs(),
        "one `Lucide` variant per vendored SVG"
    );
}

#[test]
fn every_variant_is_listed_once() {
    let mut seen = HashSet::new();
    for icon in Lucide::ALL {
        assert!(seen.insert(*icon), "{icon:?} is listed twice");
    }
}

#[test]
fn legacy_icons_and_glyphs_convert_to_icon_refs() {
    assert_eq!(IconRef::from(Icon::Plus), IconRef::Lucide(Lucide::Plus));
    assert_eq!(IconRef::from(Icon::Close), IconRef::Lucide(Lucide::X));
    assert_eq!(IconRef::from(Glyph::Play), IconRef::Glyph(Glyph::Play));

    let from_lucide: IconRef = Lucide::FolderOpen.into();
    assert_eq!(from_lucide, IconRef::Lucide(Lucide::FolderOpen));
}
