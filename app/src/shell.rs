//! Web links opened in the user's browser, and files and folders shown in Explorer, through
//! Windows' shell.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(hwnd: *mut c_void, operation: *const u16, file: *const u16, parameters: *const u16, directory: *const u16, show: i32) -> *mut c_void;
}

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

const SW_SHOWNORMAL: i32 = 1;

fn open(target: &std::ffi::OsStr) -> Result<(), String> {
    let (op, file) = (wide("open".as_ref()), wide(target));
    // above 32 is success
    let done = unsafe { ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) } as usize;
    if done > 32 { Ok(()) } else { Err(format!("Windows couldn't open it (error {done})")) }
}

/// A web link, in the user's own browser; nothing but web links.
pub fn open_link(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.chars().any(char::is_whitespace) {
        return Err("Only web links open from here.".into());
    }
    open(url.as_ref())
}

/// A page on this PC (an `.html` file), in the user's own browser.
pub fn open_page(path: &Path) -> Result<(), String> {
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("html")) || !path.is_file() {
        return Err("Only a page on this PC opens from here.".into());
    }
    open(path.as_os_str())
}

/// A folder opened, or a file shown selected in its folder.
pub fn show(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        return open(path.as_os_str());
    }
    if !path.exists() {
        return Err(format!("It isn't there any more: {}", path.display()));
    }
    std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Explorer didn't start: {e}"))
}
