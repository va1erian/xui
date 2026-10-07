//! The fonts pages are laid out and drawn with.
//!
//! Blitz shapes text with Parley and draws the glyphs itself, so it needs the
//! font files, not the backend's text shaper. With the `system-fonts` feature
//! (on by default) it finds the platform's installed fonts; a host with none
//! to find (LazyOS) registers its own with [`register_font`]. Either way
//! [`set_font_families`] says which family draws each CSS generic family, so
//! `font-family: sans-serif` does not fall to whatever face comes first.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

use parley::FontContext;
use parley::fontique::{Blob, Collection, CollectionOptions, GenericFamily, SourceCache};

/// The families a host draws each CSS generic family with.
///
/// An empty name leaves that generic family as the font system has it (the
/// platform's choice with `system-fonts`, else the first family registered).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontFamilies {
    /// For `sans-serif`, `system-ui` and `ui-sans-serif`.
    pub sans_serif: String,
    /// For `serif` and `ui-serif`.
    pub serif: String,
    /// For `monospace` and `ui-monospace`.
    pub monospace: String,
}

#[derive(Default)]
struct Registry {
    faces: Vec<Arc<Vec<u8>>>,
    families: FontFamilies,
}

static REGISTRY: RwLock<Option<Registry>> = RwLock::new(None);
/// Bumped by every change, so an engine knows its font context is stale.
static VERSION: AtomicU64 = AtomicU64::new(0);

/// Adds a font file (TrueType, OpenType or a collection) to every view's
/// fonts; it can then be named in `font-family` and in [`FontFamilies`].
/// Takes effect on the next page load.
pub fn register_font(data: Vec<u8>) {
    with_registry(|r| r.faces.push(Arc::new(data)));
}

/// Sets the families every view draws CSS generic families with; takes
/// effect on the next page load.
pub fn set_font_families(families: FontFamilies) {
    with_registry(|r| r.families = families);
}

fn with_registry(f: impl FnOnce(&mut Registry)) {
    let mut guard = REGISTRY.write().unwrap_or_else(PoisonError::into_inner);
    f(guard.get_or_insert_with(Registry::default));
    VERSION.fetch_add(1, Ordering::Release);
}

/// The registry's version: a font context built at one version is current
/// until it changes.
pub(crate) fn version() -> u64 {
    VERSION.load(Ordering::Acquire)
}

/// A font context with the system's fonts (when the feature is on), the
/// registered ones and the generic family mapping. Finding system fonts is
/// slow, so an engine builds one and clones it for each document.
pub(crate) fn font_context() -> FontContext {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: cfg!(feature = "system-fonts"),
    });
    let guard = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
    if let Some(registry) = guard.as_ref() {
        for face in &registry.faces {
            collection.register_fonts(Blob::new(Arc::clone(face) as _), None);
        }
        let generics: [(&str, &[GenericFamily]); 3] = [
            (
                &registry.families.sans_serif,
                &[
                    GenericFamily::SansSerif,
                    GenericFamily::SystemUi,
                    GenericFamily::UiSansSerif,
                ],
            ),
            (
                &registry.families.serif,
                &[GenericFamily::Serif, GenericFamily::UiSerif],
            ),
            (
                &registry.families.monospace,
                &[GenericFamily::Monospace, GenericFamily::UiMonospace],
            ),
        ];
        for (name, generic) in generics {
            if name.is_empty() {
                continue;
            }
            match collection.family_id(name) {
                Some(id) => {
                    for &g in generic {
                        collection.set_generic_families(g, std::iter::once(id));
                    }
                }
                None => log::warn!("xui-blitz: no font family named {name:?}"),
            }
        }
    }
    FontContext {
        collection,
        source_cache: SourceCache::new_shared(),
    }
}
