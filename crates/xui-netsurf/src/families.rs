#![forbid(unsafe_code)]

//! Which installed font a CSS `font-family` is drawn with.
//!
//! The shaper knows only the families its host registered, and a CSS name it
//! does not know (`system-ui`, `Arial`, `Times New Roman`, even the generic
//! `sans-serif`) falls back to whatever face the font system picks first,
//! often a monospace one. So a page's family is sorted into one of three
//! classes, by the first family it names or else by NetSurf's generic family,
//! and each class is drawn with the family the host set for it.

use std::sync::RwLock;

/// The families a host draws each class of CSS family with.
///
/// An empty name means the shaper's default family (the host's UI font).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFamilies {
    /// For `sans-serif`, `system-ui` and every family not recognised as
    /// serif or monospace.
    pub sans_serif: String,
    /// For `serif`, `Times`, `Georgia` and the like.
    pub serif: String,
    /// For `monospace`, `Courier` and the like.
    pub monospace: String,
}

impl Default for FontFamilies {
    fn default() -> Self {
        FontFamilies {
            sans_serif: String::new(),
            serif: String::new(),
            // The one generic name the canvas resolves itself.
            monospace: "monospace".to_string(),
        }
    }
}

static FAMILIES: RwLock<Option<FontFamilies>> = RwLock::new(None);

/// Sets the families every view draws pages with; takes effect on the next
/// layout. Without it, sans-serif and serif text use the default family.
pub fn set_font_families(families: FontFamilies) {
    *FAMILIES.write().unwrap_or_else(|e| e.into_inner()) = Some(families);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Sans,
    Serif,
    Mono,
}

/// Families whose name says monospace (lower case, matched as substrings).
const MONO_HINTS: &[&str] = &["mono", "courier", "consolas", "menlo", "monaco", "fixed"];
/// Families whose name says serif.
const SERIF_HINTS: &[&str] = &[
    "times",
    "georgia",
    "garamond",
    "palatino",
    "cambria",
    "book",
    "roman",
    "baskerville",
];

fn classify(named: &str) -> Option<Class> {
    let name = named.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    if MONO_HINTS.iter().any(|h| name.contains(h)) {
        return Some(Class::Mono);
    }
    if name.contains("sans") {
        return Some(Class::Sans);
    }
    if name == "serif" || SERIF_HINTS.iter().any(|h| name.contains(h)) {
        return Some(Class::Serif);
    }
    None
}

/// The family to shape with for the first family a page named and NetSurf's
/// generic one (0 sans-serif, 1 serif, 2 monospace, 3 cursive, 4 fantasy).
pub(crate) fn resolve(named: Option<&str>, generic: i32) -> String {
    let class = named.and_then(classify).unwrap_or(match generic {
        1 => Class::Serif,
        2 => Class::Mono,
        _ => Class::Sans,
    });
    let families = FAMILIES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    match class {
        Class::Sans => families.sans_serif,
        Class::Serif => families.serif,
        Class::Mono => families.monospace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sort_into_classes() {
        assert_eq!(classify("system-ui"), None);
        assert_eq!(classify("Arial"), None);
        assert_eq!(classify("sans-serif"), Some(Class::Sans));
        assert_eq!(classify("\"Open Sans\""), Some(Class::Sans));
        assert_eq!(classify("Times New Roman"), Some(Class::Serif));
        assert_eq!(classify("serif"), Some(Class::Serif));
        assert_eq!(classify("Courier New"), Some(Class::Mono));
        assert_eq!(classify("monospace"), Some(Class::Mono));
        assert_eq!(classify(""), None);
    }

    #[test]
    fn unknown_names_follow_the_generic_family() {
        // Defaults (no host families set in this test binary's other tests).
        assert_eq!(
            resolve(Some("system-ui"), 0),
            FontFamilies::default().sans_serif
        );
        assert_eq!(resolve(Some("Fancy"), 2), "monospace");
        assert_eq!(resolve(None, 2), "monospace");
        assert_eq!(resolve(Some("Courier"), 0), "monospace");
    }
}
