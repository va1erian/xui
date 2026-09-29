#![forbid(unsafe_code)]

//! The [`FileSystem`] seam the file dialog reads through.
//!
//! The dialog never calls [`std::fs`] directly: it asks a `FileSystem` for a
//! directory's entries, a path's kind and the home/roots it can start from.
//! [`StdFileSystem`] is the `std::fs`/`std::env` implementation used by default;
//! a target with a partial filesystem (LazyOS while its syscalls are
//! incomplete) supplies its own. Every call can fail and the dialog shows the
//! failure inline instead of panicking.

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// One directory entry. Only `name` and `is_dir` are required: a filesystem
/// that exposes no metadata leaves `size`/`modified` as `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The entry's name, as the filesystem gave it (possibly non-UTF-8).
    pub name: OsString,
    /// Whether the entry is a directory.
    pub is_dir: bool,
    /// The size in bytes, when the filesystem reports one.
    pub size: Option<u64>,
    /// The last-modified time, when the filesystem reports one.
    pub modified: Option<SystemTime>,
}

/// A read-only filesystem view the file dialog navigates.
///
/// The dialog never writes. Object-safe, so a dialog holds an
/// `Rc<dyn FileSystem>`; the default is [`StdFileSystem`].
pub trait FileSystem {
    /// The entries directly inside `dir`, in any order (the dialog sorts them).
    fn list(&self, dir: &Path) -> io::Result<Vec<Entry>>;

    /// Whether `path` is a directory.
    fn is_dir(&self, path: &Path) -> bool;

    /// Whether `path` exists (a file or a directory).
    fn exists(&self, path: &Path) -> bool;

    /// The user's home directory, or `None` when the filesystem has none.
    fn home(&self) -> Option<PathBuf>;

    /// The filesystem's top-level locations (drive letters, `/`), possibly
    /// empty. The dialog starts here only when it has no initial directory and
    /// no home.
    fn roots(&self) -> Vec<PathBuf>;
}

/// The [`FileSystem`] over [`std::fs`]/[`std::env`], available wherever the
/// standard library is.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn list(&self, dir: &Path) -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            // Metadata can fail (a broken symlink, a denied entry); the dialog
            // only needs the name and the directory flag, so fall back to the
            // cheap file type.
            let metadata = entry.metadata().ok();
            let is_dir = match &metadata {
                Some(metadata) => metadata.is_dir(),
                None => entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false),
            };
            entries.push(Entry {
                name,
                is_dir,
                size: metadata.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
                modified: metadata.and_then(|m| m.modified().ok()),
            });
        }
        Ok(entries)
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn home(&self) -> Option<PathBuf> {
        for key in ["HOME", "USERPROFILE"] {
            if let Some(value) = std::env::var_os(key).filter(|value| !value.is_empty()) {
                return Some(PathBuf::from(value));
            }
        }
        None
    }

    #[cfg(windows)]
    fn roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for letter in b'A'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            if root.exists() {
                roots.push(root);
            }
        }
        roots
    }

    #[cfg(not(windows))]
    fn roots(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("/")]
    }
}

/// Resolves `.` and `..` lexically, without touching the filesystem, so a typed
/// path is compared and joined consistently and a `..` above a root is a no-op.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let at_root = matches!(
                    out.components().next_back(),
                    Some(Component::RootDir | Component::Prefix(_))
                );
                if out.as_os_str().is_empty() {
                    out.push("..");
                } else if !at_root {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
pub(crate) use memory::{MemoryFileSystem, dir_entry, file_entry};

#[cfg(test)]
mod memory {
    #![forbid(unsafe_code)]

    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use std::ffi::OsString;
    use std::io;
    use std::path::{Path, PathBuf};

    use super::{Entry, FileSystem, normalize};

    /// An in-memory [`FileSystem`] for tests: fixed listings, explicit
    /// unreadable directories, an optional home and roots, and no real I/O.
    #[derive(Default)]
    pub(crate) struct MemoryFileSystem {
        listings: RefCell<HashMap<PathBuf, Vec<Entry>>>,
        dirs: HashSet<PathBuf>,
        files: HashSet<PathBuf>,
        unreadable: HashSet<PathBuf>,
        home: Option<PathBuf>,
        roots: Vec<PathBuf>,
    }

    impl MemoryFileSystem {
        /// An empty filesystem with no home and no roots.
        pub(crate) fn new() -> MemoryFileSystem {
            MemoryFileSystem::default()
        }

        /// Registers `path` as a directory whose listing is `entries`.
        pub(crate) fn dir(
            mut self,
            path: impl Into<PathBuf>,
            entries: Vec<Entry>,
        ) -> MemoryFileSystem {
            let path = normalize(&path.into());
            self.dirs.insert(path.clone());
            self.listings.borrow_mut().insert(path, entries);
            self
        }

        /// Registers `path` as an existing file.
        pub(crate) fn file(mut self, path: impl Into<PathBuf>) -> MemoryFileSystem {
            let path = normalize(&path.into());
            self.files.insert(path);
            self
        }

        /// Makes `path` fail to list, as an unreadable directory.
        pub(crate) fn unreadable(mut self, path: impl Into<PathBuf>) -> MemoryFileSystem {
            self.unreadable.insert(normalize(&path.into()));
            self
        }
    }

    impl FileSystem for MemoryFileSystem {
        fn list(&self, dir: &Path) -> io::Result<Vec<Entry>> {
            let dir = normalize(dir);
            if self.unreadable.contains(&dir) {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
            }
            self.listings
                .borrow()
                .get(&dir)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such directory"))
        }

        fn is_dir(&self, path: &Path) -> bool {
            self.dirs.contains(&normalize(path))
        }

        fn exists(&self, path: &Path) -> bool {
            let path = normalize(path);
            self.dirs.contains(&path) || self.files.contains(&path)
        }

        fn home(&self) -> Option<PathBuf> {
            self.home.clone()
        }

        fn roots(&self) -> Vec<PathBuf> {
            self.roots.clone()
        }
    }

    /// A directory entry with plain metadata.
    pub(crate) fn dir_entry(name: impl Into<OsString>) -> Entry {
        Entry {
            name: name.into(),
            is_dir: true,
            size: None,
            modified: None,
        }
    }

    /// A file entry with plain metadata.
    pub(crate) fn file_entry(name: impl Into<OsString>) -> Entry {
        Entry {
            name: name.into(),
            is_dir: false,
            size: Some(0),
            modified: None,
        }
    }
}
