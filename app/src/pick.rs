//! A folder or a file chosen in Windows' own picker, over the app's window (first setup's folders,
//! when VPX isn't found, or another is wanted; a DOF configuration to bring in): the common item
//! dialog, in its folder mode or with the file types offered, on a thread of its own for COM.

use std::path::{Path, PathBuf};

/// The folder the user picks, or none when they cancel. `owner`: the app window's handle, which
/// the picker sits over; `start`: the folder it opens in, when that's there.
pub fn folder(owner: isize, title: &str, start: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let title = title.to_string();
    let start = start.filter(|p| p.is_dir()).map(Path::to_path_buf);
    std::thread::spawn(move || pick(owner, &title, start.as_deref(), None)).join().map_err(|_| "The folder picker stopped.".to_string())?
}

/// The file the user picks, or none when they cancel: `kinds` the file types offered, each its
/// words and its patterns (`*.zip;*.ini`).
pub fn file(owner: isize, title: &str, kinds: &[(&str, &str)]) -> Result<Option<PathBuf>, String> {
    let title = title.to_string();
    let kinds: Vec<(String, String)> = kinds.iter().map(|(words, patterns)| (words.to_string(), patterns.to_string())).collect();
    std::thread::spawn(move || pick(owner, &title, None, Some(&kinds))).join().map_err(|_| "The file picker stopped.".to_string())?
}

#[cfg(windows)]
fn pick(owner: isize, title: &str, start: Option<&Path>, kinds: Option<&[(String, String)]>) -> Result<Option<PathBuf>, String> {
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH};
    use windows::core::{HSTRING, PCWSTR};
    let what = if kinds.is_some() { "file" } else { "folder" };
    // the file types' words and patterns, kept while the dialog shows
    let words: Vec<(HSTRING, HSTRING)> = kinds.unwrap_or_default().iter().map(|(w, p)| (HSTRING::from(w.as_str()), HSTRING::from(p.as_str()))).collect();
    // SAFETY: COM started on this thread of our own and ended on it; the dialog's interfaces used
    // while they live, the file types' text outliving them, the path Windows gives freed once read
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok().map_err(|e| format!("The {what} picker didn't open: {e}"))?;
        let chosen = (|| -> windows::core::Result<Option<PathBuf>> {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            let options = dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
            if kinds.is_some() {
                let types: Vec<COMDLG_FILTERSPEC> = words.iter().map(|(w, p)| COMDLG_FILTERSPEC { pszName: PCWSTR(w.as_ptr()), pszSpec: PCWSTR(p.as_ptr()) }).collect();
                dialog.SetFileTypes(&types)?;
                dialog.SetOptions(options | FOS_FILEMUSTEXIST)?;
            } else {
                dialog.SetOptions(options | FOS_PICKFOLDERS)?;
            }
            dialog.SetTitle(&HSTRING::from(title))?;
            if let Some(start) = start
                && let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(start.as_os_str()), None::<&IBindCtx>)
            {
                dialog.SetFolder(&item)?;
            }
            match dialog.Show(Some(HWND(owner as *mut _))) {
                Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => return Ok(None),
                shown => shown?,
            }
            let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let text = path.to_string();
            CoTaskMemFree(Some(path.0 as *const _));
            Ok(text.ok().map(PathBuf::from))
        })();
        CoUninitialize();
        chosen.map_err(|e| format!("The {what} picker didn't open: {e}"))
    }
}

#[cfg(not(windows))]
fn pick(_owner: isize, _title: &str, _start: Option<&Path>, _kinds: Option<&[(String, String)]>) -> Result<Option<PathBuf>, String> {
    Err("Files and folders are picked on Windows.".into())
}
