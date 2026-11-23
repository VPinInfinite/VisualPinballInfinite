//! libwinevbs, Wine's VBScript engine as a library, loaded as its own
//! DLL (LGPL-2.1-or-later, so it stays a separate, replaceable file).
//!
//! libwinevbs carries its own copy of COM and OLE Automation, so every string
//! and VARIANT that passes through it is made, copied and freed with its
//! functions, never Windows' own.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::com::*;

type CoCreateInstanceFn = unsafe extern "system" fn(*const GUID, *mut c_void, u32, *const GUID, *mut *mut c_void) -> HRESULT;
type SysAllocStringLenFn = unsafe extern "system" fn(*const u16, u32) -> *mut u16;
type SysFreeStringFn = unsafe extern "system" fn(*mut u16);
type SysStringLenFn = unsafe extern "system" fn(*mut u16) -> u32;
type VariantInitFn = unsafe extern "system" fn(*mut VARIANT);
type VariantClearFn = unsafe extern "system" fn(*mut VARIANT) -> HRESULT;
type VariantCopyFn = unsafe extern "system" fn(*mut VARIANT, *const VARIANT) -> HRESULT;
type VariantChangeTypeFn = unsafe extern "system" fn(*mut VARIANT, *const VARIANT, u16, u16) -> HRESULT;
type SafeArrayCreateVectorFn = unsafe extern "system" fn(u16, i32, u32) -> *mut c_void;
type SafeArrayPutElementFn = unsafe extern "system" fn(*mut c_void, *const i32, *mut c_void) -> HRESULT;
type SafeArrayGetElementFn = unsafe extern "system" fn(*mut c_void, *const i32, *mut c_void) -> HRESULT;
type SafeArrayBoundFn = unsafe extern "system" fn(*mut c_void, u32, *mut i32) -> HRESULT;
type SafeArrayGetDimFn = unsafe extern "system" fn(*mut c_void) -> u32;
type SafeArrayCreateFn = unsafe extern "system" fn(u16, u32, *const SAFEARRAYBOUND) -> *mut c_void;
type SafeArrayAccessDataFn = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
type SafeArrayUnaccessDataFn = unsafe extern "system" fn(*mut c_void) -> HRESULT;

/// What libwinevbs asks of its host (`winevbs_callbacks` in `libwinevbs.h`).
#[repr(C)]
pub struct Callbacks {
    pub log: unsafe extern "C" fn(level: i32, format: *const c_char, args: *mut c_void),
    pub create_object: unsafe extern "C" fn(progid: *const u16, factory: *mut c_void, object: *mut *mut c_void) -> HRESULT,
}

type InitFn = unsafe extern "C" fn(*const Callbacks);

/// Asked about every path the file object touches (`libwinevbs_file_fence_t`).
pub type FileFenceFn = unsafe extern "C" fn(path: *const u16, write: i32) -> i32;
/// `libwinevbs_set_file_fence`, in our build only (`tools/libwinevbs`).
pub type SetFileFenceFn = unsafe extern "C" fn(Option<FileFenceFn>, *const u16);

/// The library's functions.
pub struct WineVbs {
    pub co_create_instance: CoCreateInstanceFn,
    pub sys_alloc_string_len: SysAllocStringLenFn,
    pub sys_free_string: SysFreeStringFn,
    pub sys_string_len: SysStringLenFn,
    pub variant_init: VariantInitFn,
    pub variant_clear: VariantClearFn,
    pub variant_copy: VariantCopyFn,
    pub variant_copy_ind: VariantCopyFn,
    pub variant_change_type: VariantChangeTypeFn,
    pub safe_array_create_vector: SafeArrayCreateVectorFn,
    pub safe_array_put_element: SafeArrayPutElementFn,
    pub safe_array_get_element: SafeArrayGetElementFn,
    pub safe_array_get_lbound: SafeArrayBoundFn,
    pub safe_array_get_ubound: SafeArrayBoundFn,
    pub safe_array_get_dim: SafeArrayGetDimFn,
    pub safe_array_create: SafeArrayCreateFn,
    pub safe_array_access_data: SafeArrayAccessDataFn,
    pub safe_array_unaccess_data: SafeArrayUnaccessDataFn,
    /// The file object's fence; `None` in a libwinevbs that isn't ours.
    pub set_file_fence: Option<SetFileFenceFn>,
}

// SAFETY: plain function pointers into a DLL that stays loaded
unsafe impl Send for WineVbs {}
unsafe impl Sync for WineVbs {}

static LIBRARY: OnceLock<Result<WineVbs, String>> = OnceLock::new();

#[cfg(windows)]
mod os {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        pub fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    /// Finds the DLL's own dependencies next to it.
    pub const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
}

/// Where to look for `winevbs64.dll`: next to the program (how it ships),
/// then, in the repository (development builds), our own build with our
/// patches (`tools/libwinevbs`, built into `vpx-build/libwinevbs/build`), then
/// the copy from VPX's automated build (`vpx-build/winevbs`).
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.join("winevbs64.dll"));
        // target/release/vpi-player.exe -> repository root
        let roots: Vec<&Path> = if crate::DEVELOPMENT { dir.ancestors().take(4).collect() } else { Vec::new() };
        for up in &roots {
            out.push(up.join("vpx-build").join("libwinevbs").join("build").join("winevbs64.dll"));
        }
        for up in &roots {
            out.push(up.join("vpx-build").join("winevbs").join("winevbs64.dll"));
        }
    }
    out
}

unsafe extern "C" {
    /// The C runtime's formatter behind every `printf` (UCRT).
    fn __stdio_common_vsprintf(options: u64, buffer: *mut c_char, count: usize, format: *const c_char, locale: *mut c_void, args: *mut c_void) -> i32;
}

unsafe extern "C" fn log(level: i32, format: *const c_char, args: *mut c_void) {
    // SAFETY: libwinevbs passes a printf format and its arguments
    let text = unsafe {
        let mut buf = [0 as c_char; 1024];
        let n = __stdio_common_vsprintf(2, buf.as_mut_ptr(), buf.len(), format, std::ptr::null_mut(), args);
        if n < 0 && format.is_null() {
            return;
        }
        std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    };
    super::log::engine_message(level, text.trim_end());
}

unsafe extern "C" fn create_object(progid: *const u16, factory: *mut c_void, object: *mut *mut c_void) -> HRESULT {
    // SAFETY: libwinevbs passes the script's ProgID, its own factory for
    // objects it has built in (or null), and where the object goes
    unsafe {
        let name = from_wide(progid);
        super::fence::create_object(&name, factory, object)
    }
}

static CALLBACKS: Callbacks = Callbacks { log, create_object };

#[allow(clippy::missing_transmute_annotations)] // each pointer becomes its field's function type
fn load_from(path: &Path) -> Result<WineVbs, String> {
    #[cfg(windows)]
    // SAFETY: loading a DLL and looking up its exports by name
    unsafe {
        let module = os::LoadLibraryExW(wide(&path.to_string_lossy()).as_ptr(), std::ptr::null_mut(), os::LOAD_WITH_ALTERED_SEARCH_PATH);
        if module.is_null() {
            return Err(format!("could not load {}", path.display()));
        }
        let get = |name: &str| -> Result<*mut c_void, String> {
            let c = format!("{name}\0");
            let p = os::GetProcAddress(module, c.as_ptr());
            if p.is_null() { Err(format!("{} has no {name}", path.display())) } else { Ok(p) }
        };
        macro_rules! f {
            ($name:literal) => {
                std::mem::transmute::<*mut c_void, _>(get($name)?)
            };
        }
        let lib = WineVbs {
            co_create_instance: f!("CoCreateInstance"),
            sys_alloc_string_len: f!("SysAllocStringLen"),
            sys_free_string: f!("SysFreeString"),
            sys_string_len: f!("SysStringLen"),
            variant_init: f!("VariantInit"),
            variant_clear: f!("VariantClear"),
            variant_copy: f!("VariantCopy"),
            variant_copy_ind: f!("VariantCopyInd"),
            variant_change_type: f!("VariantChangeType"),
            safe_array_create_vector: f!("SafeArrayCreateVector"),
            safe_array_put_element: f!("SafeArrayPutElement"),
            safe_array_get_element: f!("SafeArrayGetElement"),
            safe_array_get_lbound: f!("SafeArrayGetLBound"),
            safe_array_get_ubound: f!("SafeArrayGetUBound"),
            safe_array_get_dim: f!("SafeArrayGetDim"),
            safe_array_create: f!("SafeArrayCreate"),
            safe_array_access_data: f!("SafeArrayAccessData"),
            safe_array_unaccess_data: f!("SafeArrayUnaccessData"),
            set_file_fence: get("libwinevbs_set_file_fence").ok().map(|p| std::mem::transmute::<*mut c_void, SetFileFenceFn>(p)),
        };
        if let Ok(init) = get("libwinevbs_init") {
            let init: InitFn = std::mem::transmute::<*mut c_void, InitFn>(init);
            init(&CALLBACKS);
        }
        Ok(lib)
    }
    #[cfg(not(windows))]
    Err(format!("loading {} is only done on Windows so far", path.display()))
}

/// The library, loaded once.
pub fn get() -> Result<&'static WineVbs, String> {
    LIBRARY
        .get_or_init(|| {
            let tried = candidates();
            for path in &tried {
                if path.exists() {
                    return load_from(path);
                }
            }
            Err(format!("winevbs64.dll not found (looked in {})", tried.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")))
        })
        .as_ref()
        .map_err(Clone::clone)
}

impl WineVbs {
    /// A BSTR holding `s`, made by the library.
    pub fn bstr(&self, s: &str) -> *mut u16 {
        let w: Vec<u16> = s.encode_utf16().collect();
        // SAFETY: the library copies `w`
        unsafe { (self.sys_alloc_string_len)(w.as_ptr(), w.len() as u32) }
    }

    /// A BSTR's text.
    ///
    /// # Safety
    /// `b` must be null or a BSTR made by this library.
    pub unsafe fn bstr_text(&self, b: *mut u16) -> String {
        if b.is_null() {
            return String::new();
        }
        unsafe {
            let n = (self.sys_string_len)(b) as usize;
            String::from_utf16_lossy(std::slice::from_raw_parts(b, n))
        }
    }
}
