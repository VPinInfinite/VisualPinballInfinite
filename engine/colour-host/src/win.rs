// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The Windows calls the host needs (kernel32), declared from Microsoft's
//! documentation: loading a library, waiting on the link and the player,
//! and a path in the code page libserum and libvni read paths in.

#![allow(non_snake_case)]

use std::ffi::{CString, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

pub type Handle = *mut c_void;

pub const SYNCHRONIZE: u32 = 0x0010_0000;
pub const WAIT_OBJECT_0: u32 = 0;
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;
const CP_ACP: u32 = 0;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: Handle, flags: u32) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *const c_void;
    pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    pub fn WaitForMultipleObjects(count: u32, handles: *const Handle, wait_all: i32, milliseconds: u32) -> u32;
    fn WideCharToMultiByte(code_page: u32, flags: u32, wide: *const u16, wide_len: i32, out: *mut u8, out_len: i32, default: *const u8, used_default: *mut i32) -> i32;
}

/// A loaded library, kept for the life of the program.
pub struct Library(Handle);

impl Library {
    /// Loads `path`, the libraries beside it found first (its own
    /// dependencies, as VPX's bundle keeps them).
    pub fn load(path: &Path) -> Result<Library, String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: a null-terminated path
        let h = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if h.is_null() { Err(format!("could not load {}: {}", path.display(), std::io::Error::last_os_error())) } else { Ok(Library(h)) }
    }

    /// The function `name` (null-terminated), if the library has it.
    pub fn symbol(&self, name: &[u8]) -> Option<*const c_void> {
        // SAFETY: a loaded module and a null-terminated name
        let p = unsafe { GetProcAddress(self.0, name.as_ptr()) };
        (!p.is_null()).then_some(p)
    }
}

/// `path` in the Windows code page, as VPX hands libserum and libvni their
/// paths (`std::filesystem::path::string()`); None if it can't be said in it.
pub fn ansi_path(path: &Path) -> Option<CString> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.is_empty() {
        return CString::new("").ok();
    }
    let mut used_default = 0;
    // SAFETY: the size asked first, then the text written into a buffer that size
    let n = unsafe { WideCharToMultiByte(CP_ACP, 0, wide.as_ptr(), wide.len() as i32, std::ptr::null_mut(), 0, std::ptr::null(), &mut used_default) };
    if n <= 0 || used_default != 0 {
        return None;
    }
    let mut out = vec![0u8; n as usize];
    // SAFETY: as above, into `out`
    unsafe { WideCharToMultiByte(CP_ACP, 0, wide.as_ptr(), wide.len() as i32, out.as_mut_ptr(), n, std::ptr::null(), &mut used_default) };
    CString::new(out).ok()
}
