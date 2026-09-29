#![forbid(unsafe_code)]

//! The optional native file-picker hook: the request a widget hands a backend
//! and the outcome it answers with.
//!
//! A backend that can show the platform's own picker (the Win32 Common Item
//! Dialog, or a desktop portal) overrides
//! [`Backend::file_dialog`](super::Backend::file_dialog); every other backend
//! declines by default and the portable
//! [`FileDialog`](crate::widget::FileDialog) runs instead. The application sees
//! only the chosen [`PathBuf`](std::path::PathBuf), never which picker answered.

use std::path::PathBuf;

/// Whether the picker is opening an existing file or choosing where to save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogMode {
    /// Choose an existing file to open.
    Open,
    /// Choose a path to save to.
    Save,
}

/// A named extension filter: a label and the extensions it accepts, each
/// without a leading dot (`["txt", "md"]`). An empty list accepts any file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
    /// The human-readable label shown in a filter selector.
    pub label: String,
    /// The accepted extensions, lower-case and dot-free.
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// A filter with `label` accepting `extensions`.
    pub fn new(label: impl Into<String>, extensions: &[&str]) -> FileFilter {
        FileFilter {
            label: label.into(),
            extensions: extensions.iter().map(|ext| ext.to_lowercase()).collect(),
        }
    }
}

/// What a file picker is asked for: the mode, a title, an optional starting
/// directory and suggested name, the extension filters and whether the result
/// must already exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialogRequest {
    /// Open or save.
    pub mode: FileDialogMode,
    /// The window/picker title.
    pub title: String,
    /// The directory to start in, if the caller has one.
    pub initial_dir: Option<PathBuf>,
    /// A prefilled name (save mode).
    pub suggested_name: Option<String>,
    /// The extension filters, in display order.
    pub filters: Vec<FileFilter>,
    /// Whether an open result must be an existing file.
    pub require_existing: bool,
}

/// How a backend's native picker answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDialogOutcome {
    /// The backend has no native picker; the caller shows its own.
    Declined,
    /// The native picker ran and the user dismissed it.
    Cancelled,
    /// The native picker ran and the user chose `path`.
    Chosen(PathBuf),
}
