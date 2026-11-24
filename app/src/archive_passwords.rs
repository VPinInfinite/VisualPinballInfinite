//! The passwords that opened locked downloads, kept to try on the next one locked: sealed with
//! Windows' data protection for this account alone (`CryptProtectData`), in the library's folder
//! (`Library\archive-passwords.dat`); handed to the archive reading as the app starts
//! (`library::archive::set_passwords`) and kept again once a typed one opens a download. Ours.

use std::path::PathBuf;

fn file() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Library").join("archive-passwords.dat"))
}

/// The kept passwords handed to the archive reading, as the app starts.
pub fn load() {
    let Some(f) = file() else { return };
    let Ok(bytes) = std::fs::read(&f) else { return };
    let Some(open) = seal::open(&bytes) else {
        eprintln!("the kept archive passwords didn't open (another account's, or changed): {}", f.display());
        return;
    };
    let list: Vec<String> = serde_json::from_slice(&open).unwrap_or_default();
    ::library::archive::set_passwords(list);
}

/// The passwords at hand kept, sealed: written beside the file kept, then put in its place.
pub fn save() -> Result<(), String> {
    let f = file().ok_or("The app's data folder wasn't found.")?;
    let json = serde_json::to_vec(&::library::archive::passwords()).map_err(|e| e.to_string())?;
    let sealed = seal::close(&json)?;
    if let Some(dir) = f.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let next = f.with_extension("dat.new");
    std::fs::write(&next, sealed).map_err(|e| format!("{}: {e}", next.display()))?;
    std::fs::rename(&next, &f).map_err(|e| format!("{}: {e}", f.display()))
}

/// Sealing with Windows' data protection, for the user's account alone, with words of our own beside
/// it: the file copied from another account, or changed, doesn't open.
mod seal {
    const WORDS: &[u8] = b"Visual Pinball Infinite: passwords of locked downloads";

    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(input: *const Blob, name: *const u16, entropy: *const Blob, reserved: *mut core::ffi::c_void, prompt: *mut core::ffi::c_void, flags: u32, out: *mut Blob) -> i32;
        fn CryptUnprotectData(input: *const Blob, name: *mut *mut u16, entropy: *const Blob, reserved: *mut core::ffi::c_void, prompt: *mut core::ffi::c_void, flags: u32, out: *mut Blob) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    const UI_FORBIDDEN: u32 = 0x1;

    fn run(bytes: &[u8], close: bool) -> Option<Vec<u8>> {
        let input = Blob { len: bytes.len() as u32, data: bytes.as_ptr() as *mut u8 };
        let entropy = Blob { len: WORDS.len() as u32, data: WORDS.as_ptr() as *mut u8 };
        let mut out = Blob { len: 0, data: std::ptr::null_mut() };
        // SAFETY: the blobs point at live bytes of the lengths given; Windows' answer is copied out
        // and its memory freed
        unsafe {
            let ok = if close {
                CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null_mut(), std::ptr::null_mut(), UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null_mut(), std::ptr::null_mut(), UI_FORBIDDEN, &mut out)
            };
            if ok == 0 || out.data.is_null() {
                return None;
            }
            let got = std::slice::from_raw_parts(out.data, out.len as usize).to_vec();
            LocalFree(out.data.cast());
            Some(got)
        }
    }

    pub fn close(bytes: &[u8]) -> Result<Vec<u8>, String> {
        run(bytes, true).ok_or_else(|| "Windows' data protection didn't seal the passwords".to_string())
    }

    pub fn open(bytes: &[u8]) -> Option<Vec<u8>> {
        run(bytes, false)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn sealed_for_this_account_and_opened_again() {
            let sealed = super::close(b"[\"password\"]").unwrap();
            assert!(!sealed.windows(8).any(|w| w == b"password"));
            assert_eq!(super::open(&sealed).as_deref(), Some(&b"[\"password\"]"[..]));
            let mut changed = sealed.clone();
            let last = changed.len() - 1;
            changed[last] ^= 1;
            assert_eq!(super::open(&changed), None);
        }
    }
}
