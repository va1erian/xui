//! Shaped text kept between frames, and gradient stop collections kept per
//! target: a UI draws the same labels and the same gradients every frame, and
//! building either costs far more than drawing it.
//!
//! A cache is keyed on a hash of what it was built from, and an entry is used
//! only when the inputs compare equal, so a hit allocates nothing and a
//! collision only costs a rebuild. A full cache is emptied: the next frame
//! refills it with what is on screen.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use windows::Win32::Graphics::DirectWrite::{IDWriteFactory, IDWriteTextLayout};
use xui_core::backend::{TextAlign, TextVAlign};

use super::text::{TextParams, shape};

/// Most layouts kept; more than a screenful of distinct text.
const MAX_LAYOUTS: usize = 2048;

/// The inputs a layout was shaped from.
struct Key {
    text: String,
    family: Option<String>,
    size_bits: u32,
    weight: u16,
    italic: bool,
    align: TextAlign,
    valign: TextVAlign,
    wrap: bool,
    width_bits: u32,
    height_bits: u32,
}

impl Key {
    fn matches(&self, text: &str, p: &TextParams<'_>, width: f32, height: f32) -> bool {
        self.text == text
            && self.family.as_deref() == p.family
            && self.size_bits == p.size_px.to_bits()
            && self.weight == p.weight
            && self.italic == p.italic
            && self.align == p.align
            && self.valign == p.valign
            && self.wrap == p.wrap
            && self.width_bits == width.to_bits()
            && self.height_bits == height.to_bits()
    }
}

fn hash_of(text: &str, p: &TextParams<'_>, width: f32, height: f32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    p.family.hash(&mut hasher);
    p.size_px.to_bits().hash(&mut hasher);
    p.weight.hash(&mut hasher);
    p.italic.hash(&mut hasher);
    (p.align as u8).hash(&mut hasher);
    (p.valign as u8).hash(&mut hasher);
    p.wrap.hash(&mut hasher);
    width.to_bits().hash(&mut hasher);
    height.to_bits().hash(&mut hasher);
    hasher.finish()
}

thread_local! {
    static LAYOUTS: RefCell<HashMap<u64, (Key, IDWriteTextLayout)>> =
        RefCell::new(HashMap::new());
}

/// Drops every cached layout.
pub(super) fn clear() {
    LAYOUTS.with(|cache| cache.borrow_mut().clear());
}

/// Like [`shape`], but a layout shaped before from the same inputs is reused.
/// The layout is never changed after it is shaped, so sharing it is safe.
pub(super) fn shape_cached(
    factory: &IDWriteFactory,
    text: &str,
    params: &TextParams<'_>,
    width: f32,
    height: f32,
) -> Option<IDWriteTextLayout> {
    let hash = hash_of(text, params, width, height);
    let found = LAYOUTS.with(|cache| {
        cache
            .borrow()
            .get(&hash)
            .filter(|(key, _)| key.matches(text, params, width, height))
            .map(|(_, layout)| layout.clone())
    });
    if found.is_some() {
        return found;
    }
    let layout = shape(factory, text, params, width, height)?;
    let key = Key {
        text: text.to_owned(),
        family: params.family.map(str::to_owned),
        size_bits: params.size_px.to_bits(),
        weight: params.weight,
        italic: params.italic,
        align: params.align,
        valign: params.valign,
        wrap: params.wrap,
        width_bits: width.to_bits(),
        height_bits: height.to_bits(),
    };
    LAYOUTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_LAYOUTS {
            cache.clear();
        }
        cache.insert(hash, (key, layout.clone()));
    });
    Some(layout)
}

#[cfg(test)]
mod tests {
    use windows::core::Interface;

    use super::super::factory;
    use super::*;

    fn params() -> TextParams<'static> {
        TextParams {
            family: None,
            size_px: 14.0,
            weight: 400,
            italic: false,
            align: TextAlign::Start,
            valign: TextVAlign::Top,
            wrap: false,
        }
    }

    fn raw(text: &str, p: &TextParams<'_>, width: f32) -> Option<usize> {
        factory::with(|f| shape_cached(&f.dwrite, text, p, width, 20.0))
            .flatten()
            .map(|layout| layout.as_raw() as usize)
    }

    #[test]
    fn the_same_inputs_share_a_layout_and_different_ones_do_not() {
        let p = params();
        let first = raw("Cache me", &p, 100.0).expect("a layout");
        assert_eq!(raw("Cache me", &p, 100.0), Some(first));
        assert_ne!(raw("Cache me too", &p, 100.0), Some(first));
        assert_ne!(raw("Cache me", &p, 120.0), Some(first), "another box");
        let bold = TextParams { weight: 700, ..p };
        assert_ne!(raw("Cache me", &bold, 100.0), Some(first), "another weight");
    }

    #[test]
    fn a_full_cache_is_emptied_and_still_answers_correctly() {
        let p = params();
        let first = raw("kept?", &p, 100.0).expect("a layout");
        for i in 0..=MAX_LAYOUTS {
            raw(&format!("filler {i}"), &p, 100.0).expect("a layout");
        }
        let again = raw("kept?", &p, 100.0).expect("a layout");
        assert_ne!(first, again, "emptied: shaped anew");
        assert_eq!(raw("kept?", &p, 100.0), Some(again), "and kept again");
    }

    #[test]
    fn releasing_the_thread_resources_empties_the_layout_cache() {
        let p = params();
        raw("Release me", &p, 100.0).expect("a layout");
        assert!(LAYOUTS.with(|cache| !cache.borrow().is_empty()));
        crate::sys::gfx::release_thread_resources();
        assert!(LAYOUTS.with(|cache| cache.borrow().is_empty()));
        assert!(raw("Release me", &p, 100.0).is_some(), "shaping recovers");
    }
}
