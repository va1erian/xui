//! The fonts pages are laid out and drawn with.
//!
//! Blitz shapes text with Parley and draws the glyphs itself, so it needs the
//! font files, not the backend's text shaper. With `bundled-fonts` (on by
//! default) the view ships the Liberation fonts (`liberation.rs`) and draws
//! CSS `sans-serif`, `serif` and `monospace` with them, so pages look the same
//! everywhere, LazyOS included; they also stand in for Arial, Times New Roman
//! and Courier New where the system lacks those. With `system-fonts` (on by
//! default) the platform's installed fonts are found too. A host adds fonts
//! with [`register_font`] and picks the generic families with
//! [`set_font_families`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

use parley::FontContext;
use parley::fontique::{Blob, Collection, CollectionOptions, GenericFamily, SourceCache};

#[cfg(feature = "bundled-fonts")]
mod liberation;

/// The families a host draws each CSS generic family with.
///
/// An empty name leaves that generic family to the bundled Liberation family
/// (with `bundled-fonts`), else to the font system.
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
/// registered ones, the bundled ones and the generic family mapping. Finding
/// system fonts is slow, so an engine builds one and clones it for each
/// document.
pub(crate) fn font_context() -> FontContext {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: cfg!(feature = "system-fonts"),
    });
    let guard = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
    let host = guard
        .as_ref()
        .map(|r| r.families.clone())
        .unwrap_or_default();
    if let Some(registry) = guard.as_ref() {
        for face in &registry.faces {
            collection.register_fonts(Blob::new(Arc::clone(face) as _), None);
        }
    }
    drop(guard);
    let bundled = bundle(&mut collection);

    // The `ui-*` and `system-ui` families stay the platform's unless the host
    // names a family or there is no platform to ask.
    let ui = !host.sans_serif.is_empty() || !cfg!(feature = "system-fonts");
    let pick = |host: &str, bundled: Option<&'static str>| -> Option<String> {
        if host.is_empty() {
            bundled.map(str::to_string)
        } else {
            Some(host.to_string())
        }
    };
    let generics: [(Option<String>, &[GenericFamily]); 3] = [
        (
            pick(&host.sans_serif, bundled.map(|b| b.0)),
            if ui {
                &[
                    GenericFamily::SansSerif,
                    GenericFamily::SystemUi,
                    GenericFamily::UiSansSerif,
                ]
            } else {
                &[GenericFamily::SansSerif]
            },
        ),
        (
            pick(&host.serif, bundled.map(|b| b.1)),
            if ui {
                &[GenericFamily::Serif, GenericFamily::UiSerif]
            } else {
                &[GenericFamily::Serif]
            },
        ),
        (
            pick(&host.monospace, bundled.map(|b| b.2)),
            if ui {
                &[GenericFamily::Monospace, GenericFamily::UiMonospace]
            } else {
                &[GenericFamily::Monospace]
            },
        ),
    ];
    for (name, generic) in generics {
        let Some(name) = name else {
            continue;
        };
        match collection.family_id(&name) {
            Some(id) => {
                for &g in generic {
                    collection.set_generic_families(g, std::iter::once(id));
                }
            }
            None => log::warn!("xui-blitz: no font family named {name:?}"),
        }
    }
    FontContext {
        collection,
        source_cache: SourceCache::new_shared(),
    }
}

/// Registers the bundled fonts, and their aliases for the families the
/// collection does not have; their sans, serif and mono family names.
#[cfg(feature = "bundled-fonts")]
fn bundle(collection: &mut Collection) -> Option<(&'static str, &'static str, &'static str)> {
    let faces = liberation::faces();
    for (_, ttf) in faces {
        collection.register_fonts(Blob::new(Arc::clone(ttf) as _), None);
    }
    for (alias, target) in liberation::ALIASES {
        if collection.family_id(alias).is_some() {
            continue;
        }
        let rename = parley::fontique::FontInfoOverride {
            family_name: Some(alias),
            ..Default::default()
        };
        for (_, ttf) in faces.iter().filter(|(family, _)| *family == target) {
            collection.register_fonts(Blob::new(Arc::clone(ttf) as _), Some(rename));
        }
    }
    Some((liberation::SANS, liberation::SERIF, liberation::MONO))
}

#[cfg(not(feature = "bundled-fonts"))]
fn bundle(_: &mut Collection) -> Option<(&'static str, &'static str, &'static str)> {
    None
}
