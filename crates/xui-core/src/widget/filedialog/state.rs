#![forbid(unsafe_code)]

//! The file dialog's pure state: navigation, filtering, selection and accept
//! logic, with no [`Ui`](crate::app::Ui) and no window.
//!
//! Keeping it UI-free is what makes the required tests run headlessly against
//! an in-memory [`FileSystem`](super::FileSystem). The widget translates key
//! and pointer events into the methods here and maps the [`Accept`] decision to
//! the app's `Msg`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::fs::{Entry, FileSystem, normalize};
use crate::backend::{FileDialogMode, FileFilter};

/// A row the entry list can show, by index into the current listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    /// The `..` parent row, shown first when the current directory has a
    /// parent.
    Parent,
    /// A subdirectory.
    Dir(usize),
    /// A file.
    File(usize),
}

/// What the dialog decides when the user accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Accept {
    /// Nothing to do (a parentless `..`).
    None,
    /// Enter `dir` and re-list it.
    Navigate(PathBuf),
    /// Return `path` to the application.
    Chosen(PathBuf),
    /// `path` exists in save mode: the caller shows the overwrite prompt, then
    /// accepting again confirms it.
    Overwrite(PathBuf),
    /// Stay put and show `message` inline.
    Error(String),
}

/// The fixed request the widget builds the state from (and rebuilds on every
/// open, so a reopened dialog never shows the previous use's state).
pub(crate) struct Config {
    pub(crate) mode: FileDialogMode,
    pub(crate) fs: Rc<dyn FileSystem>,
    pub(crate) initial_dir: Option<PathBuf>,
    pub(crate) suggested_name: String,
    pub(crate) filters: Vec<FileFilter>,
    pub(crate) require_existing: bool,
}

/// Navigation, filtering and accept state for one open of the dialog.
pub(crate) struct FileState {
    mode: FileDialogMode,
    fs: Rc<dyn FileSystem>,
    dir: PathBuf,
    entries: Vec<Entry>,
    /// The filename field's text, which also filters the files.
    name: String,
    /// The path bar's editable text.
    path_text: String,
    filters: Vec<FileFilter>,
    active_filter: usize,
    require_existing: bool,
    selection: usize,
    error: Option<String>,
    pending_overwrite: Option<PathBuf>,
    /// Set by [`FileState::set_name`] when an edit emptied the field, so the
    /// name field's Backspace does not also jump to the parent on that key.
    just_cleared: bool,
}

impl FileState {
    /// Builds the state and lists a starting directory: the caller's initial
    /// directory, else the filesystem's home, else its first root, else nothing
    /// (the dialog stays usable by typing a path).
    pub(crate) fn new(config: Config) -> FileState {
        let start = config
            .initial_dir
            .clone()
            .or_else(|| config.fs.home())
            .or_else(|| config.fs.roots().into_iter().next())
            .unwrap_or_default();
        let mut state = FileState {
            mode: config.mode,
            fs: config.fs,
            dir: start.clone(),
            entries: Vec::new(),
            name: config.suggested_name,
            path_text: start.display().to_string(),
            filters: config.filters,
            active_filter: 0,
            require_existing: config.require_existing,
            selection: 0,
            error: None,
            pending_overwrite: None,
            just_cleared: false,
        };
        state.refresh();
        state
    }

    /// The mode.
    pub(crate) fn mode(&self) -> FileDialogMode {
        self.mode
    }

    /// The path bar's text.
    pub(crate) fn path_text(&self) -> String {
        self.path_text.clone()
    }

    /// Replaces the path bar's text.
    pub(crate) fn set_path_text(&mut self, text: &str) {
        self.path_text = text.to_string();
    }

    /// The filename/filter field's text.
    pub(crate) fn name(&self) -> String {
        self.name.clone()
    }

    /// Whether the filename field is empty.
    pub(crate) fn name_empty(&self) -> bool {
        self.name.trim().is_empty()
    }

    /// Replaces the filename field's text, resetting the selection because the
    /// visible rows may have changed. Records whether an edit emptied the
    /// field, for the Backspace-to-parent rule.
    pub(crate) fn set_name(&mut self, text: &str) {
        self.just_cleared = !self.name.is_empty() && text.is_empty();
        self.name = text.to_string();
        self.selection = 0;
    }

    /// Takes the "an edit just emptied the field" flag.
    pub(crate) fn take_just_cleared(&mut self) -> bool {
        std::mem::take(&mut self.just_cleared)
    }

    /// The inline error text, empty when there is none.
    pub(crate) fn error(&self) -> String {
        self.error.clone().unwrap_or_default()
    }

    /// Whether an overwrite confirmation is pending.
    pub(crate) fn is_confirming_overwrite(&self) -> bool {
        self.pending_overwrite.is_some()
    }

    /// The overwrite prompt to show, or `None` when none is pending.
    pub(crate) fn overwrite_prompt(&self) -> Option<String> {
        self.pending_overwrite
            .as_ref()
            .map(|path| format!("{} already exists. Overwrite?", path.display()))
    }

    /// The active extension filter's label, for the filter selector.
    pub(crate) fn filter_label(&self) -> String {
        self.filters
            .get(self.active_filter)
            .map(|filter| filter.label.clone())
            .unwrap_or_default()
    }

    /// Whether the filter selector is worth showing (more than one filter).
    pub(crate) fn has_filter_choice(&self) -> bool {
        self.filters.len() > 1
    }

    /// The visible rows: `..` when there is a parent, then directories and
    /// files matching the typed name, the files also matching the active
    /// extension filter.
    pub(crate) fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        if self.parent_dir().is_some() {
            rows.push(Row::Parent);
        }
        let needle = self.name.trim().to_lowercase();
        for (index, entry) in self.entries.iter().enumerate() {
            let label = entry.name.to_string_lossy().to_lowercase();
            if !needle.is_empty() && !label.starts_with(&needle) {
                continue;
            }
            if entry.is_dir {
                rows.push(Row::Dir(index));
            } else if self.matches_active_filter(&entry.name) {
                rows.push(Row::File(index));
            }
        }
        rows
    }

    /// The label rows are drawn with: `..` for the parent, a trailing `/` on a
    /// directory, the lossy name otherwise. The returned [`PathBuf`]s are never
    /// built from these labels.
    pub(crate) fn row_labels(&self) -> Vec<String> {
        self.rows()
            .into_iter()
            .map(|row| match row {
                Row::Parent => "..".to_string(),
                Row::Dir(index) => format!("{}/", self.entries[index].name.to_string_lossy()),
                Row::File(index) => self.entries[index].name.to_string_lossy().into_owned(),
            })
            .collect()
    }

    /// The selected row index, clamped into range.
    pub(crate) fn selection(&self) -> usize {
        self.selection.min(self.rows().len().saturating_sub(1))
    }

    /// Selects a row without raising an event.
    pub(crate) fn select(&mut self, index: usize) {
        let len = self.rows().len();
        self.selection = index.min(len.saturating_sub(1));
    }

    /// Moves the selection by `delta`, clamped into the visible rows. The
    /// filename field's Up/Down keys use it so the list is drivable without
    /// leaving the field.
    pub(crate) fn move_selection(&mut self, delta: isize) {
        let len = self.rows().len();
        if len == 0 {
            self.selection = 0;
            return;
        }
        let next = self.selection as isize + delta;
        self.selection = next.clamp(0, len as isize - 1) as usize;
    }

    /// Cycles to the next extension filter.
    pub(crate) fn cycle_filter(&mut self) {
        if self.filters.len() > 1 {
            self.active_filter = (self.active_filter + 1) % self.filters.len();
            self.selection = 0;
        }
    }

    /// Re-lists the current directory, clearing the entries on failure.
    fn refresh(&mut self) {
        match self.fs.list(&self.dir) {
            Ok(entries) => {
                self.entries = sort_entries(entries);
                self.error = None;
            }
            Err(error) => {
                self.entries.clear();
                self.error = Some(format!("Could not read {}: {error}", self.dir.display()));
            }
        }
    }

    /// Navigates to `dir`, replacing the current listing only on success, so a
    /// failed listing leaves the previous directory and its entries current.
    pub(crate) fn navigate_to(&mut self, dir: impl Into<PathBuf>) -> bool {
        let dir = normalize(&dir.into());
        match self.fs.list(&dir) {
            Ok(entries) => {
                self.dir = dir;
                self.entries = sort_entries(entries);
                self.error = None;
                self.selection = 0;
                self.name.clear();
                self.path_text = self.dir.display().to_string();
                self.pending_overwrite = None;
                true
            }
            Err(error) => {
                self.error = Some(format!("Could not read {}: {error}", dir.display()));
                false
            }
        }
    }

    /// Navigates to the parent of the current directory; a root is a no-op.
    pub(crate) fn navigate_parent(&mut self) {
        if let Some(parent) = self.parent_dir() {
            let _ = self.navigate_to(parent);
        }
    }

    /// Navigates to the path bar's typed text (absolute or relative to the
    /// current directory).
    pub(crate) fn navigate_typed(&mut self) -> bool {
        let text = self.path_text.trim();
        if text.is_empty() {
            return false;
        }
        let path = Path::new(text);
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.dir.join(path)
        };
        self.navigate_to(joined)
    }

    /// Accepts the current input: the typed name or the selected row, applying
    /// open/save semantics.
    pub(crate) fn accept(&mut self) -> Accept {
        if let Some(path) = self.pending_overwrite.take() {
            self.error = None;
            return Accept::Chosen(path);
        }
        let typed = self.name.trim();
        if !typed.is_empty() {
            let path = Path::new(typed);
            let joined = if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.dir.join(path)
            };
            return self.decide(normalize(&joined));
        }
        let rows = self.rows();
        let Some(row) = rows.get(self.selection).copied() else {
            return self.fail("Enter a file name");
        };
        match row {
            Row::Parent => match self.parent_dir() {
                Some(parent) => {
                    self.error = None;
                    Accept::Navigate(parent)
                }
                None => Accept::None,
            },
            Row::Dir(index) => {
                let path = self.entry_path(index);
                self.error = None;
                Accept::Navigate(path)
            }
            Row::File(index) => self.decide(self.entry_path(index)),
        }
    }

    /// Cancels a pending overwrite, returning to the dialog.
    pub(crate) fn cancel_overwrite(&mut self) {
        self.pending_overwrite = None;
        self.error = None;
    }

    /// The absolute path of an entry, keeping its original (possibly non-UTF-8)
    /// name.
    fn entry_path(&self, index: usize) -> PathBuf {
        normalize(&self.dir.join(&self.entries[index].name))
    }

    /// Applies the mode's rules to `path`: a directory navigates, an open that
    /// must exist rejects a missing file, a save over an existing file asks to
    /// overwrite, anything else is chosen.
    fn decide(&mut self, path: PathBuf) -> Accept {
        if self.fs.is_dir(&path) {
            self.error = None;
            return Accept::Navigate(path);
        }
        let exists = self.fs.exists(&path);
        match self.mode {
            FileDialogMode::Open if self.require_existing && !exists => {
                self.fail(format!("{}: no such file", path.display()))
            }
            FileDialogMode::Save if exists => {
                self.error = None;
                self.pending_overwrite = Some(path.clone());
                Accept::Overwrite(path)
            }
            _ => {
                self.error = None;
                Accept::Chosen(path)
            }
        }
    }

    fn fail(&mut self, message: impl Into<String>) -> Accept {
        let message = message.into();
        self.error = Some(message.clone());
        Accept::Error(message)
    }

    /// The current directory's parent, or `None` at a root or with no parent.
    fn parent_dir(&self) -> Option<PathBuf> {
        let parent = self.dir.parent()?;
        if parent.as_os_str().is_empty() {
            None
        } else {
            Some(parent.to_path_buf())
        }
    }

    /// Whether `name` matches the active extension filter. No filter (or an
    /// empty one) accepts anything; otherwise the extension is the text after a
    /// final dot that is neither the first nor the last character, compared
    /// case-insensitively.
    fn matches_active_filter(&self, name: &OsStr) -> bool {
        let Some(filter) = self.filters.get(self.active_filter) else {
            return true;
        };
        if filter.extensions.is_empty() {
            return true;
        }
        let Some(extension) = extension_of(name) else {
            return false;
        };
        filter
            .extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&extension))
    }
}

/// The extension of `name`, or `None` for a name with no extension. A leading
/// dot (`.gitignore`) or a trailing dot (`name.`) has no extension.
fn extension_of(name: &OsStr) -> Option<String> {
    let name = name.to_string_lossy();
    let dot = name.rfind('.')?;
    if dot == 0 || dot + 1 == name.len() {
        return None;
    }
    Some(name[dot + 1..].to_string())
}

/// Directories first, then a case-insensitive name; ties break on the original
/// name, so the order is deterministic even for names differing only in case.
fn sort_entries(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| {
                a.name
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&b.name.to_string_lossy().to_lowercase())
            })
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}
