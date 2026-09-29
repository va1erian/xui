#![forbid(unsafe_code)]

//! The sorted listing of one folder and the [`IconModel`] view over it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use xui_core::icon::{IconRef, Lucide};
use xui_core::widget::IconModel;

use super::format_size;
use super::sort::sort_entries;
use crate::platform::{Kind, Platform, RawEntry};

/// One listed item, with its display strings resolved once at load time so the
/// paint path never allocates.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The item's raw name, used for selection remapping and deletion.
    pub name: OsString,
    /// The name as shown, with a non-UTF-8 name lossily converted.
    pub display: String,
    /// The second tile line: the size for a file, `"Folder"` for a directory.
    pub detail: String,
    /// What the item is.
    pub kind: Kind,
    /// The file's size in bytes, when it has one.
    pub size: Option<u64>,
}

impl Entry {
    /// Resolves a raw entry into its display form.
    pub fn from_raw(raw: RawEntry) -> Entry {
        let display = raw.name.to_string_lossy().into_owned();
        let detail = match raw.meta.kind {
            Kind::Dir => "Folder".to_string(),
            Kind::File | Kind::Symlink => raw
                .meta
                .size
                .map(format_size)
                .unwrap_or_else(|| "File".to_string()),
        };
        Entry {
            name: raw.name,
            display,
            detail,
            kind: raw.meta.kind,
            size: raw.meta.size,
        }
    }
}

/// A folder's contents: its entries (folders first, then files, each group
/// sorted case-insensitively) and, when the folder could not be read, an error
/// message with an empty entry list.
#[derive(Clone, Debug)]
pub struct Listing {
    /// The folder this listing is of.
    pub dir: PathBuf,
    /// The entries, folders first.
    pub entries: Vec<Entry>,
    /// The read error, when the folder could not be listed.
    pub error: Option<String>,
}

impl Listing {
    /// Lists `dir` through `platform`. A read failure yields an empty listing
    /// carrying the error text rather than an empty view with no explanation.
    pub fn load(platform: &dyn Platform, dir: &Path) -> Listing {
        match platform.list(dir) {
            Ok(raw) => {
                let mut entries: Vec<Entry> = raw.into_iter().map(Entry::from_raw).collect();
                sort_entries(&mut entries);
                Listing {
                    dir: dir.to_path_buf(),
                    entries,
                    error: None,
                }
            }
            Err(error) => Listing {
                dir: dir.to_path_buf(),
                entries: Vec::new(),
                error: Some(error.to_string()),
            },
        }
    }

    /// An empty listing for `dir`, used before the first load.
    pub fn empty(dir: &Path) -> Listing {
        Listing {
            dir: dir.to_path_buf(),
            entries: Vec::new(),
            error: None,
        }
    }

    /// The index of the entry named `name`, if present.
    pub fn index_of(&self, name: &OsStr) -> Option<usize> {
        self.entries.iter().position(|entry| entry.name == name)
    }

    /// The names of the selected entries, in listing order. Used to carry a
    /// selection across a refresh by identity rather than by index.
    pub fn names_of(&self, selection: &[usize]) -> Vec<OsString> {
        selection
            .iter()
            .filter_map(|index| self.entries.get(*index).map(|entry| entry.name.clone()))
            .collect()
    }

    /// The indices of `names` that still exist, ascending and deduplicated.
    pub fn indices_of(&self, names: &[OsString]) -> Vec<usize> {
        let mut indices: Vec<usize> = names
            .iter()
            .filter_map(|name| self.index_of(name))
            .collect();
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    /// The selected entries, in listing order.
    pub fn selected(&self, selection: &[usize]) -> Vec<&Entry> {
        selection
            .iter()
            .filter_map(|index| self.entries.get(*index))
            .collect()
    }
}

/// A shared [`Listing`] as an [`IconModel`], so the app and the view read the
/// same entries without copying them.
#[derive(Clone)]
pub struct SharedListing(pub Rc<Listing>);

impl SharedListing {
    /// Wraps `listing`.
    pub fn new(listing: Rc<Listing>) -> SharedListing {
        SharedListing(listing)
    }
}

impl IconModel for SharedListing {
    fn items(&self) -> usize {
        self.0.entries.len()
    }

    fn icon(&self, item: usize) -> Option<IconRef> {
        self.0.entries.get(item).map(|entry| icon_for(entry).into())
    }

    fn line(&self, item: usize, line: usize) -> Option<&str> {
        let entry = self.0.entries.get(item)?;
        match line {
            0 => Some(&entry.display),
            1 => Some(&entry.detail),
            _ => None,
        }
    }
}

/// The icon for an entry: [`Lucide::Folder`] for a directory, otherwise a
/// Lucide outline picked from the file's extension (falling back to
/// [`Lucide::File`]).
pub fn icon_for(entry: &Entry) -> Lucide {
    match entry.kind {
        Kind::Dir => Lucide::Folder,
        Kind::File | Kind::Symlink => icon_for_name(&entry.name),
    }
}

/// Picks a file icon from the (ASCII, case-insensitive) extension. A name with
/// no extension or an unknown one gets [`Lucide::File`].
fn icon_for_name(name: &OsStr) -> Lucide {
    let Some(extension) = Path::new(name).extension().and_then(OsStr::to_str) else {
        return Lucide::File;
    };
    match extension.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" => Lucide::Image,
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "h" | "cpp" | "hpp" | "go" | "java"
        | "json" | "toml" | "yaml" | "yml" | "sh" | "ps1" | "bat" => Lucide::FileCode,
        "zip" | "rar" | "7z" | "tar" | "gz" | "xz" | "bz2" => Lucide::Package,
        _ => Lucide::File,
    }
}
