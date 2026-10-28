//! RAR archives, read through libarchive (BSD-2-Clause, by Tim Kientzle and its
//! contributors; its RAR and RAR 5 readers by Andres Mejia and Grzegorz
//! Antoniak), which reads them without RARLAB's own code, whose licence isn't
//! free. `archive.dll` is loaded when a RAR archive is first
//! read: beside the program, else where `VPI_LIBARCHIVE` says, else the
//! repository's build of it (`tools\libarchive\setup.ps1`). Only its RAR
//! readers are switched on: its other readers would take a zip packed inside a
//! RAR archive for the archive itself. Its functions declared from its own
//! headers (`archive.h`, `archive_entry.h`, 3.8.10). Ours.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::archive::Entry;

type archive = c_void;
type archive_entry = c_void;

const ARCHIVE_EOF: c_int = 1;
const ARCHIVE_OK: c_int = 0;
const ARCHIVE_WARN: c_int = -20;
/// `AE_IFMT` and `AE_IFREG`: an entry's kind, and a file's.
const AE_IFMT: u16 = 0o170000;
const AE_IFREG: u16 = 0o100000;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
}
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

/// The libarchive functions used, as its headers declare them on 64-bit Windows (`la_ssize_t` and
/// `time_t` 64 bits, `wchar_t` 16, a mode an `unsigned short`).
struct Lib {
    read_new: unsafe extern "C" fn() -> *mut archive,
    support_rar: unsafe extern "C" fn(*mut archive) -> c_int,
    support_rar5: unsafe extern "C" fn(*mut archive) -> c_int,
    open_filenames_w: unsafe extern "C" fn(*mut archive, *const *const u16, usize) -> c_int,
    next_header: unsafe extern "C" fn(*mut archive, *mut *mut archive_entry) -> c_int,
    read_data: unsafe extern "C" fn(*mut archive, *mut c_void, usize) -> isize,
    error_string: unsafe extern "C" fn(*mut archive) -> *const c_char,
    read_free: unsafe extern "C" fn(*mut archive) -> c_int,
    entry_pathname_w: unsafe extern "C" fn(*mut archive_entry) -> *const u16,
    entry_size: unsafe extern "C" fn(*mut archive_entry) -> i64,
    entry_filetype: unsafe extern "C" fn(*mut archive_entry) -> u16,
    entry_mtime: unsafe extern "C" fn(*mut archive_entry) -> i64,
    entry_is_encrypted: unsafe extern "C" fn(*mut archive_entry) -> c_int,
}

// SAFETY: plain function pointers into a library loaded for the program's life
unsafe impl Send for Lib {}
unsafe impl Sync for Lib {}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Where `archive.dll` may be, in the order looked.
fn places() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let exe = std::env::current_exe().ok();
    if let Some(dir) = exe.as_deref().and_then(Path::parent) {
        out.push(dir.join("archive.dll"));
    }
    if let Some(p) = std::env::var_os("VPI_LIBARCHIVE") {
        out.push(PathBuf::from(p));
    }
    // the repository's build, the program being in target\<profile> (development builds: a release
    // build, compiled with VPI_RELEASE_BUILD set, looks beside the program alone)
    if option_env!("VPI_RELEASE_BUILD").is_none()
        && let Some(repo) = exe.as_deref().and_then(|e| e.ancestors().nth(3))
    {
        out.push(repo.join(r"vpx-build\libarchive\vpi-build\bin\Release\archive.dll"));
    }
    out
}

fn load() -> Result<Lib, String> {
    let path = places().into_iter().find(|p| p.exists()).ok_or("libarchive (archive.dll) isn't installed, so RAR archives can't be read: tools\\libarchive\\setup.ps1 builds it")?;
    let w = wide(&path);
    // SAFETY: a null-terminated path
    let module = unsafe { LoadLibraryExW(w.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
    if module.is_null() {
        return Err(format!("could not load {}: {}", path.display(), std::io::Error::last_os_error()));
    }
    let symbol = |name: &[u8]| -> Result<*const c_void, String> {
        // SAFETY: a loaded module and a null-terminated name
        let p = unsafe { GetProcAddress(module, name.as_ptr()) };
        if p.is_null() { Err(format!("{} lacks {}", path.display(), String::from_utf8_lossy(&name[..name.len() - 1]))) } else { Ok(p) }
    };
    // SAFETY: each function's type as libarchive's headers declare it
    unsafe {
        Ok(Lib {
            read_new: std::mem::transmute::<*const c_void, unsafe extern "C" fn() -> *mut archive>(symbol(b"archive_read_new\0")?),
            support_rar: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive) -> c_int>(symbol(b"archive_read_support_format_rar\0")?),
            support_rar5: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive) -> c_int>(symbol(b"archive_read_support_format_rar5\0")?),
            open_filenames_w: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive, *const *const u16, usize) -> c_int>(symbol(
                b"archive_read_open_filenames_w\0",
            )?),
            next_header: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive, *mut *mut archive_entry) -> c_int>(symbol(b"archive_read_next_header\0")?),
            read_data: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive, *mut c_void, usize) -> isize>(symbol(b"archive_read_data\0")?),
            error_string: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive) -> *const c_char>(symbol(b"archive_error_string\0")?),
            read_free: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive) -> c_int>(symbol(b"archive_read_free\0")?),
            entry_pathname_w: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive_entry) -> *const u16>(symbol(b"archive_entry_pathname_w\0")?),
            entry_size: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive_entry) -> i64>(symbol(b"archive_entry_size\0")?),
            entry_filetype: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive_entry) -> u16>(symbol(b"archive_entry_filetype\0")?),
            entry_mtime: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive_entry) -> i64>(symbol(b"archive_entry_mtime\0")?),
            entry_is_encrypted: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive_entry) -> c_int>(symbol(b"archive_entry_is_encrypted\0")?),
        })
    }
}

fn lib() -> Result<&'static Lib, String> {
    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    LIB.get_or_init(load).as_ref().map_err(Clone::clone)
}

/// An entry of the archive: its path, size, whether it's a file (not a folder or a link), when it
/// was changed (milliseconds since 1970) and whether it's locked.
struct Header {
    path: String,
    size: u64,
    file: bool,
    modified: i64,
    encrypted: bool,
}

/// libarchive's reader for one archive, its parts given in order; freed when dropped.
struct Reader {
    lib: &'static Lib,
    a: *mut archive,
    name: String,
}

impl Reader {
    fn open(parts: &[PathBuf]) -> Result<Reader, String> {
        let lib = lib()?;
        let name = parts.first().map(|p| p.display().to_string()).unwrap_or_default();
        // SAFETY: libarchive's own constructor; freed in Drop
        let a = unsafe { (lib.read_new)() };
        if a.is_null() {
            return Err(format!("{name}: libarchive could not start"));
        }
        let r = Reader { lib, a, name };
        let names: Vec<Vec<u16>> = parts.iter().map(|p| wide(p)).collect();
        let mut list: Vec<*const u16> = names.iter().map(|n| n.as_ptr()).collect();
        list.push(std::ptr::null());
        // SAFETY: a live reader; a null-terminated list of null-terminated paths, alive for the call
        // (libarchive copies them)
        let ok = unsafe { (lib.support_rar)(a) == ARCHIVE_OK && (lib.support_rar5)(a) == ARCHIVE_OK && (lib.open_filenames_w)(a, list.as_ptr(), 1 << 16) == ARCHIVE_OK };
        if !ok {
            return Err(r.error("not a RAR archive it can read"));
        }
        Ok(r)
    }

    fn error(&self, fallback: &str) -> String {
        // SAFETY: a live reader; its message, if any, a C string it owns
        let msg = unsafe {
            let p = (self.lib.error_string)(self.a);
            (!p.is_null()).then(|| std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
        };
        format!("{}: {}", self.name, msg.unwrap_or_else(|| fallback.to_string()))
    }

    /// The next entry.
    fn next(&mut self) -> Result<Option<Header>, String> {
        let mut e: *mut archive_entry = std::ptr::null_mut();
        // SAFETY: a live reader; the entry it hands back lives until the next call
        let rc = unsafe { (self.lib.next_header)(self.a, &mut e) };
        if rc == ARCHIVE_EOF {
            return Ok(None);
        }
        if rc != ARCHIVE_OK && rc != ARCHIVE_WARN {
            return Err(self.error("its list of files couldn't be read"));
        }
        // SAFETY: the entry just handed back; its name a null-terminated wide string it owns
        unsafe {
            let p = (self.lib.entry_pathname_w)(e);
            let path = if p.is_null() {
                String::new()
            } else {
                let len = (0..).take_while(|&i| *p.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
            };
            Ok(Some(Header {
                path: path.replace('\\', "/"),
                size: (self.lib.entry_size)(e).max(0) as u64,
                file: (self.lib.entry_filetype)(e) & AE_IFMT == AE_IFREG,
                modified: (self.lib.entry_mtime)(e).saturating_mul(1000),
                encrypted: (self.lib.entry_is_encrypted)(e) != 0,
            }))
        }
    }

    /// The current entry's contents into `out`, at most `limit` bytes.
    fn data(&mut self, out: &mut dyn Write, limit: u64) -> Result<(), String> {
        let mut buf = vec![0u8; 1 << 16];
        let mut left = limit;
        while left > 0 {
            // SAFETY: a live reader, positioned on an entry; `buf` writable for its length
            let n = unsafe { (self.lib.read_data)(self.a, buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 {
                return Err(self.error("a file in it couldn't be read"));
            }
            if n == 0 {
                break;
            }
            let n = (n as u64).min(left) as usize;
            out.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", self.name))?;
            left -= n as u64;
        }
        Ok(())
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: the reader made in `open`, freed once
        unsafe { (self.lib.read_free)(self.a) };
    }
}

/// A RAR archive's files (its parts given in order), each path with `/` between folders; an archive
/// with a password refused, which libarchive can't open.
pub fn list(parts: &[PathBuf]) -> Result<Vec<Entry>, String> {
    let mut r = Reader::open(parts)?;
    let password = |name: &str| format!("{name} has a password, which ours can't open: unpack it first with the program it was made for");
    let mut entries = Vec::new();
    loop {
        // an archive whose list of files is itself locked fails here
        let next = r.next().map_err(|e| if e.contains("ncrypt") { password(&r.name) } else { e })?;
        let Some(h) = next else { break };
        if h.encrypted {
            return Err(password(&r.name));
        }
        if h.file {
            entries.push(Entry { path: h.path, size: h.size, modified: Some(h.modified) });
        }
    }
    Ok(entries)
}

/// One file of a RAR archive into `out`, at most `limit` bytes.
/// Many of an archive's files out, read through once: each into the writer `open` gives for its
/// path (None: passed over, libarchive skipping its data).
pub fn extract(parts: &[PathBuf], open: &mut crate::archive::Opener) -> Result<(), String> {
    let mut r = Reader::open(parts)?;
    while let Some(h) = r.next()? {
        if h.file
            && let Some(mut out) = open(&h.path)?
        {
            r.data(&mut out, u64::MAX)?;
            out.flush().map_err(|e| format!("{}: {e}", h.path))?;
        }
    }
    Ok(())
}

pub fn copy_out(parts: &[PathBuf], entry: &str, out: &mut dyn Write, limit: u64) -> Result<(), String> {
    let mut r = Reader::open(parts)?;
    while let Some(h) = r.next()? {
        if h.file && h.path == entry {
            return r.data(out, limit);
        }
    }
    Err(format!("{}: no {entry} in it", r.name))
}

/// A RAR archive's parts, in order, from any one of them: `name.part1.rar`, `name.part2.rar`... (RAR
/// 3 on), or `name.rar`, `name.r00`, `name.r01`... (older); the file alone when it has none.
pub fn parts_of(path: &Path) -> Vec<PathBuf> {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return vec![path.to_path_buf()];
    };
    let dir = path.parent().unwrap_or(Path::new(""));
    // ASCII letters alone lowered, so places in it are places in the name
    let lower = name.to_ascii_lowercase();
    // name.partN.rar, N padded to the same width in every part
    if let Some(stem) = lower.strip_suffix(".rar")
        && let Some(at) = stem.rfind(".part")
        && stem[at + 5..].chars().all(|c| c.is_ascii_digit())
        && stem.len() > at + 5
    {
        let width = stem.len() - at - 5;
        let base = &name[..at];
        let mut out = Vec::new();
        for n in 1.. {
            let p = dir.join(format!("{base}.part{n:0width$}.rar"));
            if !p.exists() {
                break;
            }
            out.push(p);
        }
        if !out.is_empty() {
            return out;
        }
    }
    // name.rar with name.r00, name.r01...
    let base = match lower.rsplit_once('.') {
        Some((b, x)) if x == "rar" || (x.len() == 3 && x.starts_with('r') && x[1..].chars().all(|c| c.is_ascii_digit())) => &name[..b.len()],
        _ => return vec![path.to_path_buf()],
    };
    let first = dir.join(format!("{base}.rar"));
    let mut out = vec![if first.exists() { first } else { path.to_path_buf() }];
    for n in 0..100 {
        let p = dir.join(format!("{base}.r{n:02}"));
        if !p.exists() {
            break;
        }
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_found_from_any_of_them() {
        let dir = std::env::temp_dir().join(format!("vpi-library-parts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["Pack.part1.rar", "Pack.part2.rar", "Pack.part3.rar", "Old.rar", "Old.r00", "Old.r01", "Alone.rar"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let names = |p: Vec<PathBuf>| p.iter().map(|x| x.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(names(parts_of(&dir.join("Pack.part2.rar"))), ["Pack.part1.rar", "Pack.part2.rar", "Pack.part3.rar"]);
        assert_eq!(names(parts_of(&dir.join("Old.r01"))), ["Old.rar", "Old.r00", "Old.r01"]);
        assert_eq!(names(parts_of(&dir.join("Alone.rar"))), ["Alone.rar"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
