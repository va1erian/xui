//! The native file picker: the Common Item Dialog (`IFileOpenDialog` and
//! `IFileSaveDialog`), shown modally over a window.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FileOpenDialog, FileSaveDialog,
    IFileDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};
use xui_core::backend::{FileDialogMode, FileDialogOutcome, FileDialogRequest};

use super::window::Hwnd;

/// What `Show` returns when the user cancels (`HRESULT_FROM_WIN32(ERROR_CANCELLED)`,
/// winerror.h).
const CANCELLED: i32 = 0x800704C7_u32 as i32;

/// Shows the picker described by `request` over `owner` and waits for the
/// answer. A dialog that cannot be created declines, so the portable picker
/// can take over.
pub(crate) fn show(owner: Hwnd, request: &FileDialogRequest) -> FileDialogOutcome {
    // SAFETY: COM is initialised once per thread; a second call (or a thread
    // already in another apartment) fails harmlessly and COM stays usable.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    match pick(owner, request) {
        Ok(Some(path)) => FileDialogOutcome::Chosen(path),
        Ok(None) => FileDialogOutcome::Cancelled,
        Err(_) => FileDialogOutcome::Declined,
    }
}

fn pick(owner: Hwnd, request: &FileDialogRequest) -> windows::core::Result<Option<PathBuf>> {
    // SAFETY: standard Common Item Dialog calls on interfaces created here;
    // every string passed outlives the call that reads it (`specs` and the
    // wide strings it points at live until after `Show`), and the shown
    // dialog is modal so nothing is used after it returns except the result.
    unsafe {
        let dialog: IFileDialog = match request.mode {
            FileDialogMode::Open => CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?,
            FileDialogMode::Save => CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?,
        };
        let mut options = dialog.GetOptions()? | FOS_FORCEFILESYSTEM;
        match request.mode {
            FileDialogMode::Open if request.require_existing => options |= FOS_FILEMUSTEXIST,
            FileDialogMode::Save => options |= FOS_OVERWRITEPROMPT,
            FileDialogMode::Open => {}
        }
        dialog.SetOptions(options)?;
        dialog.SetTitle(&HSTRING::from(request.title.as_str()))?;
        if let Some(name) = &request.suggested_name {
            dialog.SetFileName(&HSTRING::from(name.as_str()))?;
        }
        if let Some(dir) = request.initial_dir.as_deref().and_then(folder_item) {
            dialog.SetFolder(&dir)?;
        }

        let names: Vec<(HSTRING, HSTRING)> = request
            .filters
            .iter()
            .map(|filter| {
                let spec = filter_spec(&filter.extensions);
                (HSTRING::from(filter.label.as_str()), HSTRING::from(spec))
            })
            .collect();
        let specs: Vec<COMDLG_FILTERSPEC> = names
            .iter()
            .map(|(name, spec)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            })
            .collect();
        if !specs.is_empty() {
            dialog.SetFileTypes(&specs)?;
            // Only a save appends an extension; an open must not rewrite what the
            // user typed.
            if request.mode == FileDialogMode::Save
                && let Some(ext) = request
                    .filters
                    .first()
                    .and_then(|f| f.extensions.first())
                    .map(|ext| bare_extension(ext))
                    .filter(|ext| !ext.is_empty())
            {
                dialog.SetDefaultExtension(&HSTRING::from(ext))?;
            }
        }

        match dialog.Show(Some(HWND(owner.raw().0))) {
            Ok(()) => {}
            Err(error) if error.code().0 == CANCELLED => return Ok(None),
            Err(error) => return Err(error),
        }
        let item = dialog.GetResult()?;
        let name: PWSTR = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(name.to_string()?);
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(path))
    }
}

/// The shell item for a folder, or `None` when it does not exist.
fn folder_item(dir: &Path) -> Option<IShellItem> {
    // SAFETY: the wide string outlives the call, which copies it.
    unsafe { SHCreateItemFromParsingName(&HSTRING::from(dir.as_os_str()), None) }.ok()
}

/// `ext` without a leading `*.` or `.`: `"msl"`, `".msl"` and `"*.msl"` all
/// name the same extension.
fn bare_extension(ext: &str) -> &str {
    ext.trim_start_matches("*.").trim_start_matches('.')
}

/// The pattern list of one filter, `*.a;*.b`. A filter with no extensions
/// accepts any file, as [`xui_core::backend::FileFilter`] documents.
fn filter_spec(extensions: &[String]) -> String {
    let patterns: Vec<String> = extensions
        .iter()
        .map(|ext| bare_extension(ext))
        .filter(|ext| !ext.is_empty())
        .map(|ext| format!("*.{ext}"))
        .collect();
    if patterns.is_empty() {
        "*.*".to_owned()
    } else {
        patterns.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(extensions: &[&str]) -> String {
        filter_spec(
            &extensions
                .iter()
                .map(|e| (*e).to_owned())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn a_filter_lists_each_extension_as_a_pattern() {
        assert_eq!(spec(&["msl", "txt"]), "*.msl;*.txt");
    }

    #[test]
    fn a_dot_or_a_glob_prefix_names_the_same_extension() {
        assert_eq!(spec(&[".msl", "*.txt"]), "*.msl;*.txt");
        assert_eq!(bare_extension("*.msl"), "msl");
    }

    #[test]
    fn a_filter_with_no_extensions_accepts_any_file() {
        assert_eq!(spec(&[]), "*.*");
        assert_eq!(spec(&["", "."]), "*.*");
    }
}
