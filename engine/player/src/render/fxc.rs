//! FXC, the HLSL shader compiler built into every copy of Windows
//! (`d3dcompiler_47.dll`). SDL3 GPU needs compiled shaders, and FXC's DXBC
//! output is one of the formats its Direct3D 12 backend accepts.

use std::ffi::{CStr, c_char, c_void};
use std::ptr::{null, null_mut};

/// The parts of the `ID3DBlob` COM interface used here.
#[repr(C)]
struct BlobVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut Blob) -> u32,
    get_buffer_pointer: unsafe extern "system" fn(*mut Blob) -> *mut c_void,
    get_buffer_size: unsafe extern "system" fn(*mut Blob) -> usize,
}

#[repr(C)]
struct Blob {
    vtbl: *const BlobVtbl,
}

#[link(name = "d3dcompiler")]
unsafe extern "system" {
    fn D3DCompile(
        source: *const c_void,
        source_size: usize,
        source_name: *const c_char,
        defines: *const c_void,
        include: *const c_void,
        entry_point: *const c_char,
        target: *const c_char,
        flags1: u32,
        flags2: u32,
        code: *mut *mut Blob,
        errors: *mut *mut Blob,
    ) -> i32;
}

const OPTIMIZATION_LEVEL3: u32 = 1 << 15;

/// Compiles one entry point of `source` for `target`, e.g. `vs_5_1`.
pub fn compile(source: &str, entry_point: &CStr, target: &CStr) -> Result<Vec<u8>, String> {
    let mut code = null_mut();
    let mut errors = null_mut();
    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            c"player.hlsl".as_ptr(),
            null(),
            null(),
            entry_point.as_ptr(),
            target.as_ptr(),
            OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            &mut errors,
        )
    };
    let messages = unsafe { take(errors) };
    let code = unsafe { take(code) };
    if result < 0 || code.is_empty() {
        return Err(format!(
            "FXC could not compile {}: {}",
            entry_point.to_string_lossy(),
            String::from_utf8_lossy(&messages).trim()
        ));
    }
    Ok(code)
}

/// Copies a blob's contents out and releases it.
unsafe fn take(blob: *mut Blob) -> Vec<u8> {
    if blob.is_null() {
        return Vec::new();
    }
    unsafe {
        let vtbl = &*(*blob).vtbl;
        let data = (vtbl.get_buffer_pointer)(blob) as *const u8;
        let bytes = std::slice::from_raw_parts(data, (vtbl.get_buffer_size)(blob)).to_vec();
        (vtbl.release)(blob);
        bytes
    }
}
