//! RAR archives, read through libarchive (BSD-2-Clause, by Tim Kientzle and its
//! contributors; its RAR and RAR 5 readers by Andres Mejia and Grzegorz
//! Antoniak), which reads them without RARLAB's own code, whose licence isn't
//! free. `archive.dll` is loaded when a RAR archive is first
//! read: beside the program, else where `VPI_LIBARCHIVE` says, else the
//! repository's build of it (`tools\libarchive\setup.ps1`). Only its RAR
//! readers are switched on: its other readers would take a zip packed inside a
//! RAR archive for the archive itself. Its functions declared from its own
//! headers (`archive.h`, `archive_entry.h`, 3.8.10; the same in 3.8.11). An
//! archive locked with a password, which libarchive can't open, handed to it
//! unlocked as it reads (`rar_unlock`), through its read, skip and seek
//! callbacks. Ours.

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
const ARCHIVE_FATAL: i64 = -30;

/// libarchive's callbacks for an archive it's handed (`archive_read_callback`, `archive_skip_callback`,
/// `archive_seek_callback`, `archive_open_callback` and `archive_close_callback`).
type ReadCallback = unsafe extern "C" fn(*mut archive, *mut c_void, *mut *const c_void) -> isize;
type SkipCallback = unsafe extern "C" fn(*mut archive, *mut c_void, i64) -> i64;
type SeekCallback = unsafe extern "C" fn(*mut archive, *mut c_void, i64, c_int) -> i64;
type OpenCallback = unsafe extern "C" fn(*mut archive, *mut c_void) -> c_int;
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
    read_open2: unsafe extern "C" fn(*mut archive, *mut c_void, Option<OpenCallback>, Option<ReadCallback>, Option<SkipCallback>, Option<OpenCallback>) -> c_int,
    set_seek_callback: unsafe extern "C" fn(*mut archive, Option<SeekCallback>) -> c_int,
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
    // the repository's build, the program being in target\<profile> or a test in its `deps`
    // (development builds: a release build, compiled with VPI_RELEASE_BUILD set, looks beside the
    // program alone)
    if option_env!("VPI_RELEASE_BUILD").is_none() {
        for up in [3, 4] {
            if let Some(repo) = exe.as_deref().and_then(|e| e.ancestors().nth(up)) {
                out.push(repo.join(r"vpx-build\libarchive\vpi-build\bin\Release\archive.dll"));
            }
        }
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
            read_open2: std::mem::transmute::<
                *const c_void,
                unsafe extern "C" fn(*mut archive, *mut c_void, Option<OpenCallback>, Option<ReadCallback>, Option<SkipCallback>, Option<OpenCallback>) -> c_int,
            >(symbol(b"archive_read_open2\0")?),
            set_seek_callback: std::mem::transmute::<*const c_void, unsafe extern "C" fn(*mut archive, Option<SeekCallback>) -> c_int>(symbol(b"archive_read_set_seek_callback\0")?),
        })
    }
}

/// An unlocked archive as libarchive reads it, through the callbacks below: the bytes of its last
/// read kept until the next.
struct Stream {
    unlocked: crate::rar_unlock::Unlocked,
    buf: Vec<u8>,
}

unsafe extern "C" fn stream_read(_a: *mut archive, data: *mut c_void, out: *mut *const c_void) -> isize {
    // SAFETY: `data` is the reader's Stream, alive until the reader is freed
    let s = unsafe { &mut *data.cast::<Stream>() };
    let Stream { unlocked, buf } = s;
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unlocked.read_into(buf))) {
        Ok(Ok(n)) => {
            // SAFETY: libarchive's place for the buffer, which stays alive until the next read
            unsafe { *out = buf.as_ptr().cast() };
            n as isize
        }
        _ => -1,
    }
}

unsafe extern "C" fn stream_skip(_a: *mut archive, data: *mut c_void, request: i64) -> i64 {
    // SAFETY: as in `stream_read`
    let s = unsafe { &mut *data.cast::<Stream>() };
    let from = s.unlocked.position();
    s.unlocked.seek_to(from.saturating_add(request.max(0) as u64));
    (s.unlocked.position() - from) as i64
}

unsafe extern "C" fn stream_seek(_a: *mut archive, data: *mut c_void, offset: i64, whence: c_int) -> i64 {
    // SAFETY: as in `stream_read`
    let s = unsafe { &mut *data.cast::<Stream>() };
    let base = match whence {
        0 => 0,
        1 => s.unlocked.position() as i64,
        2 => s.unlocked.len() as i64,
        _ => return ARCHIVE_FATAL,
    };
    match base.checked_add(offset) {
        Some(to) if to >= 0 => {
            s.unlocked.seek_to(to as u64);
            s.unlocked.position() as i64
        }
        _ => ARCHIVE_FATAL,
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

/// libarchive's reader for one archive, its parts given in order, or an archive unlocked; freed when
/// dropped, its stream after it.
struct Reader {
    lib: &'static Lib,
    a: *mut archive,
    name: String,
    stream: Option<Box<Stream>>,
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
        let r = Reader { lib, a, name, stream: None };
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

    /// An archive unlocked (`rar_unlock`), read through it.
    fn open_unlocked(unlocked: crate::rar_unlock::Unlocked, name: &str) -> Result<Reader, String> {
        let lib = lib()?;
        // SAFETY: libarchive's own constructor; freed in Drop
        let a = unsafe { (lib.read_new)() };
        if a.is_null() {
            return Err(format!("{name}: libarchive could not start"));
        }
        let mut stream = Box::new(Stream { unlocked, buf: vec![0u8; 1 << 20] });
        let data = (&mut *stream as *mut Stream).cast::<c_void>();
        let r = Reader { lib, a, name: name.to_string(), stream: Some(stream) };
        // SAFETY: a live reader; `data` the boxed Stream, which the reader keeps until after it's freed
        let ok = unsafe {
            (lib.support_rar)(a) == ARCHIVE_OK
                && (lib.support_rar5)(a) == ARCHIVE_OK
                && (lib.set_seek_callback)(a, Some(stream_seek)) == ARCHIVE_OK
                && (lib.read_open2)(a, data, None, Some(stream_read), Some(stream_skip), None) == ARCHIVE_OK
        };
        if !ok {
            return Err(r.error("not a RAR archive it can read, unlocked"));
        }
        Ok(r)
    }

    /// The checksums to check on the files' data as it comes out, which libarchive can't.
    fn checks(&self) -> std::sync::Arc<Vec<crate::rar_unlock::Check>> {
        self.stream.as_ref().map(|s| s.unlocked.checks.clone()).unwrap_or_default()
    }

    /// The current entry's contents into `out` (at most `limit` bytes), checked against `check` where
    /// the archive keeps its checksum as a MAC.
    fn checked_data(&mut self, out: &mut dyn Write, limit: u64, check: Option<&crate::rar_unlock::Check>, path: &str) -> Result<(), String> {
        let Some(check) = check else { return self.data(out, limit) };
        let mut hashed = Hashed { out, crc: crc32fast::Hasher::new() };
        self.data(&mut hashed, limit)?;
        if limit == u64::MAX && !check.matches(hashed.crc.finalize()) {
            return Err(format!("{}: {path} didn't come out whole (its checksum differs): the password may not be its own", self.name));
        }
        Ok(())
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

/// A writer handing on what it's given, its CRC32 worked out as it goes.
struct Hashed<'a> {
    out: &'a mut dyn Write,
    crc: crc32fast::Hasher,
}

impl Write for Hashed<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.out.write(buf)?;
        self.crc.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

/// How the plain reading of an archive ended, where it didn't list it.
enum Plainly {
    Locked,
    Failed(String),
}

/// A RAR archive's files read as it is: locked ones told apart.
fn list_plain(parts: &[PathBuf]) -> Result<Vec<Entry>, Plainly> {
    let mut r = Reader::open(parts).map_err(Plainly::Failed)?;
    let mut entries = Vec::new();
    loop {
        // an archive whose list of files is itself locked fails here
        let next = r.next().map_err(|e| if e.contains("ncrypt") { Plainly::Locked } else { Plainly::Failed(e) })?;
        let Some(h) = next else { break };
        if h.encrypted {
            return Err(Plainly::Locked);
        }
        if h.file {
            entries.push(Entry { path: h.path, size: h.size, modified: Some(h.modified) });
        }
    }
    Ok(entries)
}

/// `parts[0]` unlocked with `passwords`, said in words where it can't be.
fn unlocked(parts: &[PathBuf], passwords: &[String]) -> Result<crate::rar_unlock::Unlocked, String> {
    let name = parts[0].display().to_string();
    match crate::rar_unlock::unlock(&parts[0], passwords) {
        Ok(u) => Ok(u),
        Err(crate::rar_unlock::Fail::Locked) => Err(crate::archive::locked_message(&parts[0])),
        Err(crate::rar_unlock::Fail::Cannot(e)) => Err(format!("{name}: {e}")),
    }
}

/// The reader for an archive: as it is, or unlocked with `passwords` where it's locked.
fn reader(parts: &[PathBuf], passwords: &[String]) -> Result<Reader, String> {
    if passwords.is_empty() {
        return Reader::open(parts);
    }
    let name = parts[0].display().to_string();
    Reader::open_unlocked(unlocked(parts, passwords)?, &name)
}

/// Whether the `n`th file of an archive unlocked comes out whole: a password RAR 2.9 to 4 keep
/// nothing to check against checked by reading the first file it opens.
fn reads(unlocked: &crate::rar_unlock::Unlocked, n: usize, name: &str) -> bool {
    let Ok(again) = unlocked.again() else { return false };
    let Ok(mut r) = Reader::open_unlocked(again, name) else { return false };
    let checks = r.checks();
    let mut index = 0;
    while let Ok(Some(h)) = r.next() {
        if index == n {
            let check = checks.iter().find(|c| c.file == index);
            return r.checked_data(&mut std::io::sink(), u64::MAX, check, &h.path).is_ok();
        }
        index += 1;
    }
    false
}

/// A RAR archive's files (its parts given in order), each path with `/` between folders, and the
/// passwords it's opened with where it's locked: every one of `passwords` at once (RAR 5 checks each
/// file's against the value it keeps), then each by itself. One locked in parts isn't opened.
pub fn list(parts: &[PathBuf], passwords: &[String]) -> Result<(Vec<Entry>, Vec<String>), String> {
    match list_plain(parts) {
        Ok(entries) => return Ok((entries, Vec::new())),
        Err(Plainly::Failed(e)) => return Err(e),
        Err(Plainly::Locked) => {}
    }
    let name = parts[0].display().to_string();
    if parts.len() > 1 {
        return Err(format!("{name} is locked with a password and split into parts, which ours can't open yet: unpack it first with the program it was made for"));
    }
    let attempts = std::iter::once(passwords.to_vec()).chain(passwords.iter().map(|p| vec![p.clone()])).filter(|a| !a.is_empty());
    for attempt in attempts {
        let unlocked = match crate::rar_unlock::unlock(&parts[0], &attempt) {
            Ok(u) => u,
            Err(crate::rar_unlock::Fail::Locked) => continue,
            Err(crate::rar_unlock::Fail::Cannot(e)) => return Err(format!("{name}: {e}")),
        };
        if let Some(n) = unlocked.unchecked
            && !reads(&unlocked, n, &name)
        {
            continue;
        }
        let mut r = Reader::open_unlocked(unlocked, &name)?;
        let mut entries = Vec::new();
        while let Some(h) = r.next()? {
            if h.file {
                entries.push(Entry { path: h.path, size: h.size, modified: Some(h.modified) });
            }
        }
        return Ok((entries, attempt));
    }
    Err(crate::archive::locked_message(&parts[0]))
}

/// Many of an archive's files out, read through once: each into the writer `open` gives for its
/// path (None: passed over, libarchive skipping its data); a locked archive unlocked with `passwords`.
pub fn extract(parts: &[PathBuf], passwords: &[String], open: &mut crate::archive::Opener) -> Result<(), String> {
    let mut r = reader(parts, passwords)?;
    let checks = r.checks();
    let mut index = 0;
    while let Some(h) = r.next()? {
        if h.file
            && let Some(mut out) = open(&h.path)?
        {
            r.checked_data(&mut out, u64::MAX, checks.iter().find(|c| c.file == index), &h.path)?;
            out.flush().map_err(|e| format!("{}: {e}", h.path))?;
        }
        index += 1;
    }
    Ok(())
}

/// One file of a RAR archive into `out`, at most `limit` bytes; a locked archive unlocked with
/// `passwords`.
pub fn copy_out(parts: &[PathBuf], passwords: &[String], entry: &str, out: &mut dyn Write, limit: u64) -> Result<(), String> {
    let mut r = reader(parts, passwords)?;
    let checks = r.checks();
    let mut index = 0;
    while let Some(h) = r.next()? {
        if h.file && h.path == entry {
            return r.checked_data(out, limit, checks.iter().find(|c| c.file == index), entry);
        }
        index += 1;
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
