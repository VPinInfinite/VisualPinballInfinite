//! What a dropped file or folder holds, listed without unpacking it: a folder's
//! files, an archive's — a zip's from its central directory, a 7-Zip archive's
//! from its header (`sevenz-rust2`, Apache-2.0), a RAR archive's, in however
//! many parts, through libarchive (`rar.rs`) — or the one file dropped; and a
//! file of it read out when needed: a read-me, a table to read its info from.
//! An archive locked with a password is opened with the first password at hand
//! that opens it (`passwords`): those typed, and those the app keeps. Ours.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A password, never written out where the drop is (`{:?}` shows none).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<password>")
    }
}

impl Secret {
    pub fn new(text: &str) -> Secret {
        Secret(text.to_string())
    }

    pub fn text(&self) -> &str {
        &self.0
    }
}

/// The passwords tried on a locked archive, the latest first: those typed while the program runs,
/// and those it was given as it started (the app's own, kept).
static PASSWORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A password tried first from now on.
pub fn remember_password(password: &str) {
    if password.is_empty() {
        return;
    }
    let mut list = PASSWORDS.lock().unwrap_or_else(|e| e.into_inner());
    list.retain(|p| p != password);
    list.insert(0, password.to_string());
}

/// A password tried no more.
pub fn forget_password(password: &str) {
    PASSWORDS.lock().unwrap_or_else(|e| e.into_inner()).retain(|p| p != password);
}

/// The passwords to try, as kept: replacing those at hand.
pub fn set_passwords(list: Vec<String>) {
    *PASSWORDS.lock().unwrap_or_else(|e| e.into_inner()) = list.into_iter().filter(|p| !p.is_empty()).collect();
}

/// The passwords to try, the latest first.
pub fn passwords() -> Vec<String> {
    PASSWORDS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// How an archive no password at hand opens is said: the screens show a box for its password by it.
pub const LOCKED: &str = " is locked with a password";

/// Whether `error` says an archive is locked, a password to be typed for it.
pub fn is_locked(error: &str) -> bool {
    error.ends_with(LOCKED)
}

pub fn locked_message(path: &Path) -> String {
    format!("{}{LOCKED}", path.display())
}

/// One file in a drop, its path relative to the drop's top with `/` between folders.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    /// When it was last changed, in milliseconds since 1970, as the archive or folder keeps it.
    pub modified: Option<i64>,
}

/// A zip entry's name, its folders split by `/`: as the zip encodes it, else read as UTF-8 at best.
pub fn zip_name<R: Read + ?Sized>(f: &zip::read::ZipFile<'_, R>) -> String {
    f.name().map(|n| n.into_owned()).unwrap_or_else(|_| String::from_utf8_lossy(f.name_raw()).into_owned()).replace('\\', "/")
}

/// Whether an archive's name for one of its files keeps the file inside the folder it's unpacked
/// into: no `..`, no root, drive or share, no stream (`name:stream`), no control characters. An
/// archive can name its files anything; one named to climb out of its folder is left out of the
/// listing, so nothing ever places it.
pub fn stays_inside(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['/', '\\'])
        && name.split(['/', '\\']).all(|part| part != ".." && !part.contains(':') && !part.chars().any(char::is_control))
}

/// An archive's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Zip,
    SevenZip,
    Rar,
}

impl Format {
    pub fn describe(self) -> &'static str {
        match self {
            Format::Zip => "zip archive",
            Format::SevenZip => "7-Zip archive",
            Format::Rar => "RAR archive",
        }
    }
}

/// An archive dropped: its kind and its file, or files for a RAR archive in parts, in order; the
/// passwords that open it where it's locked.
#[derive(Clone, Debug, PartialEq)]
pub struct Archive {
    pub format: Format,
    pub parts: Vec<PathBuf>,
    pub passwords: Vec<Secret>,
}

impl Archive {
    /// Its file, the first of its parts.
    pub fn path(&self) -> &Path {
        &self.parts[0]
    }

    fn password_texts(&self) -> Vec<String> {
        self.passwords.iter().map(|s| s.text().to_string()).collect()
    }

    /// The first password, as the zip and 7-Zip readers take one.
    fn password(&self) -> Option<&str> {
        self.passwords.first().map(Secret::text)
    }

    /// Its name without the kind of archive, or the part (`name.part1.rar` is `name`).
    pub fn name(&self) -> String {
        let stem = self.path().file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        match stem.to_ascii_lowercase().rfind(".part") {
            Some(at) if self.format == Format::Rar && stem.len() > at + 5 && stem[at + 5..].chars().all(|c| c.is_ascii_digit()) => stem[..at].to_string(),
            _ => stem,
        }
    }
}

/// What a drop is.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Folder(PathBuf),
    Archive(Archive),
    /// One file dropped by itself.
    File(PathBuf),
}

/// A drop's files.
#[derive(Clone, Debug)]
pub struct Listing {
    pub source: Source,
    pub entries: Vec<Entry>,
}

/// The kind of archive a file is, by its name: a RAR archive's later parts too (`.r00`).
pub fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "zip" => Some(Format::Zip),
        "7z" => Some(Format::SevenZip),
        "rar" => Some(Format::Rar),
        x if x.len() == 3 && x.starts_with('r') && x[1..].chars().all(|c| c.is_ascii_digit()) => Some(Format::Rar),
        _ => None,
    }
}

/// Lists a dropped file or folder.
pub fn list(path: &Path) -> Result<Listing, String> {
    if path.is_dir() {
        let mut entries = Vec::new();
        walk(path, path, &mut entries)?;
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        return Ok(Listing { source: Source::Folder(path.to_path_buf()), entries });
    }
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let (archive, mut entries) = match format_of(path) {
        Some(Format::Zip) => {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| fail(&e))?).map_err(|e| fail(&e))?;
            let mut entries = Vec::new();
            // its smallest locked file, read to try a password on
            let mut probe: Option<(usize, u64)> = None;
            for i in 0..zip.len() {
                let f = zip.by_index_raw(i).map_err(|e| fail(&e))?;
                if f.is_dir() {
                    continue;
                }
                if f.encrypted() && probe.is_none_or(|(_, size)| f.size() < size) {
                    probe = Some((i, f.size()));
                }
                let modified = f.last_modified().map(|t| civil_ms(t.year().into(), t.month().into(), t.day().into(), t.hour().into(), t.minute().into(), t.second().into()));
                entries.push(Entry { path: zip_name(&f), size: f.size(), modified });
            }
            let passwords = match probe {
                None => Vec::new(),
                Some((i, _)) => {
                    // a wrong password fails its check or its file's checksum
                    let opens = |zip: &mut zip::ZipArchive<std::fs::File>, p: &str| zip.by_index_decrypt(i, p.as_bytes()).is_ok_and(|mut f| std::io::copy(&mut f, &mut std::io::sink()).is_ok());
                    let found = passwords().into_iter().find(|p| opens(&mut zip, p)).ok_or_else(|| locked_message(path))?;
                    vec![Secret::new(&found)]
                }
            };
            (Archive { format: Format::Zip, parts: vec![path.to_path_buf()], passwords }, entries)
        }
        Some(Format::SevenZip) => {
            let candidates = passwords();
            // its list of files locked: opened with the first password that reads it
            let (a, mut used) = match sevenz_rust2::Archive::open(path) {
                Ok(a) => (a, None),
                Err(sevenz_rust2::Error::PasswordRequired) => candidates
                    .iter()
                    .find_map(|p| sevenz_rust2::Archive::open_with_password(path, &sevenz_rust2::Password::from(p.as_str())).ok().map(|a| (a, Some(p.clone()))))
                    .ok_or_else(|| locked_message(path))?,
                Err(e) => return Err(seven_zip_error(path, e)),
            };
            // its files locked, its list not: the first password that reads its first locked block
            let is_aes = |c: &sevenz_rust2::Coder| c.encoder_method_id() == sevenz_rust2::EncoderMethod::ID_AES256_SHA256;
            if used.is_none()
                && let Some(block) = (0..a.blocks.len()).find(|&b| a.blocks[b].coders.iter().any(is_aes))
            {
                used = Some(candidates.into_iter().find(|p| seven_zip_block_opens(path, &a, block, p)).ok_or_else(|| locked_message(path))?);
            }
            let entries = a
                .files
                .iter()
                .filter(|f| !f.is_directory())
                .map(|f| Entry { path: f.name().replace('\\', "/"), size: f.size(), modified: unix_ms(std::time::SystemTime::from(f.last_modified_date())) })
                .collect();
            (Archive { format: Format::SevenZip, parts: vec![path.to_path_buf()], passwords: used.iter().map(|p| Secret::new(p)).collect() }, entries)
        }
        Some(Format::Rar) => {
            let parts = crate::rar::parts_of(path);
            let (entries, used) = crate::rar::list(&parts, &passwords())?;
            (Archive { format: Format::Rar, parts, passwords: used.iter().map(|p| Secret::new(p)).collect() }, entries)
        }
        None => {
            let meta = std::fs::metadata(path).map_err(|e| fail(&e))?;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let modified = meta.modified().ok().and_then(unix_ms);
            return Ok(Listing { source: Source::File(path.to_path_buf()), entries: vec![Entry { path: name, size: meta.len(), modified }] });
        }
    };
    // a file named to climb out of wherever it's unpacked: left out, never placed
    entries.retain(|e| stays_inside(&e.path));
    Ok(Listing { source: Source::Archive(archive), entries })
}

fn walk(top: &Path, dir: &Path, out: &mut Vec<Entry>) -> Result<(), String> {
    for item in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let item = item.map_err(|e| e.to_string())?;
        let path = item.path();
        if path.is_dir() {
            walk(top, &path, out)?;
        } else {
            let rel = path.strip_prefix(top).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let meta = item.metadata().ok();
            out.push(Entry { path: rel, size: meta.as_ref().map_or(0, |m| m.len()), modified: meta.and_then(|m| m.modified().ok()).and_then(unix_ms) });
        }
    }
    Ok(())
}

/// A 7-Zip archive's error said plainly: one locked with a password none at hand opens.
fn seven_zip_error(path: &Path, e: sevenz_rust2::Error) -> String {
    match e {
        sevenz_rust2::Error::PasswordRequired | sevenz_rust2::Error::MaybeBadPassword(_) => locked_message(path),
        e => format!("{}: {e}", path.display()),
    }
}

/// A 7-Zip archive's password as its reader takes it: none where it isn't locked.
fn seven_zip_password(password: Option<&str>) -> sevenz_rust2::Password {
    password.map_or_else(sevenz_rust2::Password::empty, sevenz_rust2::Password::from)
}

fn seven_zip_open(path: &Path, password: Option<&str>) -> Result<sevenz_rust2::Archive, String> {
    sevenz_rust2::Archive::open_with_password(path, &seven_zip_password(password)).map_err(|e| seven_zip_error(path, e))
}

/// Whether `password` reads a 7-Zip archive's `block` through: a wrong one fails its decompression
/// or its files' checksums.
fn seven_zip_block_opens(path: &Path, archive: &sevenz_rust2::Archive, block: usize, password: &str) -> bool {
    let Ok(mut source) = std::fs::File::open(path) else { return false };
    let password = sevenz_rust2::Password::from(password);
    let decoder = sevenz_rust2::BlockDecoder::new(1, block, archive, &password, &mut source);
    decoder
        .for_each_entries(&mut |_, reader| {
            std::io::copy(reader, &mut std::io::sink())?;
            Ok(true)
        })
        .is_ok()
}

/// One file of a 7-Zip archive into `out`, at most `limit` bytes: its block alone decoded, the files
/// before it in that block read through (a solid archive's files follow one another in one stream).
fn seven_zip_copy(path: &Path, password: Option<&str>, entry: &str, out: &mut dyn Write, limit: u64) -> Result<(), String> {
    let archive = seven_zip_open(path, password)?;
    let index = archive.files.iter().position(|f| f.name().replace('\\', "/") == entry).ok_or_else(|| format!("{}: no {entry} in it", path.display()))?;
    let Some(block) = archive.stream_map.file_block_index[index] else {
        return Ok(()); // an empty file
    };
    let mut source = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let password = seven_zip_password(password);
    let decoder = sevenz_rust2::BlockDecoder::new(1, block, &archive, &password, &mut source);
    let wanted = &archive.files[index];
    decoder
        .for_each_entries(&mut |f, reader| {
            if std::ptr::eq(f, wanted) {
                std::io::copy(&mut reader.take(limit), out)?;
                Ok(false)
            } else {
                std::io::copy(reader, &mut std::io::sink())?;
                Ok(true)
            }
        })
        .map_err(|e| seven_zip_error(path, e))?;
    Ok(())
}

/// What `Listing::extract` asks of its caller for each file it meets: a writer for it, or None to
/// pass it over.
pub type Opener<'a> = dyn FnMut(&str) -> Result<Option<Box<dyn Write>>, String> + 'a;

/// A 7-Zip archive's files out, block by block, each block decoded once.
fn seven_zip_extract(path: &Path, password: Option<&str>, open: &mut Opener) -> Result<(), String> {
    let archive = seven_zip_open(path, password)?;
    // its empty files, which no block holds
    for (i, f) in archive.files.iter().enumerate() {
        if !f.is_directory()
            && archive.stream_map.file_block_index[i].is_none()
            && let Some(mut out) = open(&f.name().replace('\\', "/"))?
        {
            out.flush().map_err(|e| format!("{}: {e}", f.name()))?;
        }
    }
    let mut source = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let password = seven_zip_password(password);
    for block in 0..archive.blocks.len() {
        let decoder = sevenz_rust2::BlockDecoder::new(1, block, &archive, &password, &mut source);
        let mut failed: Option<String> = None;
        decoder
            .for_each_entries(&mut |f, reader| {
                if f.is_directory() {
                    return Ok(true);
                }
                let name = f.name().replace('\\', "/");
                match open(&name) {
                    Ok(Some(mut out)) => {
                        std::io::copy(reader, &mut out)?;
                        out.flush()?;
                    }
                    Ok(None) => {
                        std::io::copy(reader, &mut std::io::sink())?;
                    }
                    Err(e) => {
                        failed = Some(e);
                        return Ok(false);
                    }
                }
                Ok(true)
            })
            .map_err(|e| seven_zip_error(path, e))?;
        if let Some(e) = failed {
            return Err(e);
        }
    }
    Ok(())
}

impl Listing {
    /// One of the drop's files into `out`, at most `limit` bytes.
    pub fn copy_out(&self, entry: &str, out: &mut dyn Write, limit: u64) -> Result<(), String> {
        let open = |p: &Path| std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()));
        let copy = |r: &mut dyn Read, out: &mut dyn Write| std::io::copy(&mut r.take(limit), out).map(|_| ()).map_err(|e| format!("{entry}: {e}"));
        match &self.source {
            Source::Folder(top) => copy(&mut open(&top.join(entry))?, out),
            Source::File(path) => copy(&mut open(path)?, out),
            Source::Archive(a) => match a.format {
                Format::Zip => {
                    let mut zip = zip::ZipArchive::new(open(a.path())?).map_err(|e| format!("{}: {e}", a.path().display()))?;
                    let found = match a.password() {
                        Some(p) => zip.by_name_decrypt(entry, p.as_bytes()),
                        None => zip.by_name(entry),
                    };
                    let mut f = found.map_err(|e| format!("{} ({entry}): {e}", a.path().display()))?;
                    copy(&mut f, out)
                }
                Format::SevenZip => seven_zip_copy(a.path(), a.password(), entry, out, limit),
                Format::Rar => crate::rar::copy_out(&a.parts, &a.password_texts(), entry, out, limit),
            },
        }
    }

    /// Many of the drop's files out at once, each into the writer `open` gives for its path (None:
    /// passed over), the archive read through once — a solid 7-Zip block's or a RAR archive's files
    /// follow one another in one stream, so taking them one by one would read it again for each.
    pub fn extract(&self, open: &mut Opener) -> Result<(), String> {
        let open_file = |p: &Path| std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()));
        match &self.source {
            Source::Folder(top) => {
                for e in &self.entries {
                    if let Some(mut out) = open(&e.path)? {
                        std::io::copy(&mut open_file(&top.join(&e.path))?, &mut out).map_err(|x| format!("{}: {x}", e.path))?;
                        out.flush().map_err(|x| format!("{}: {x}", e.path))?;
                    }
                }
                Ok(())
            }
            Source::File(path) => {
                for e in &self.entries {
                    if let Some(mut out) = open(&e.path)? {
                        std::io::copy(&mut open_file(path)?, &mut out).map_err(|x| format!("{}: {x}", e.path))?;
                        out.flush().map_err(|x| format!("{}: {x}", e.path))?;
                    }
                }
                Ok(())
            }
            Source::Archive(a) => match a.format {
                Format::Zip => {
                    let mut zip = zip::ZipArchive::new(open_file(a.path())?).map_err(|e| format!("{}: {e}", a.path().display()))?;
                    for i in 0..zip.len() {
                        let found = match a.password() {
                            Some(p) => zip.by_index_decrypt(i, p.as_bytes()),
                            None => zip.by_index(i),
                        };
                        let mut f = found.map_err(|e| format!("{}: {e}", a.path().display()))?;
                        if f.is_dir() {
                            continue;
                        }
                        let name = zip_name(&f);
                        if let Some(mut out) = open(&name)? {
                            std::io::copy(&mut f, &mut out).map_err(|e| format!("{name}: {e}"))?;
                            out.flush().map_err(|e| format!("{name}: {e}"))?;
                        }
                    }
                    Ok(())
                }
                Format::SevenZip => seven_zip_extract(a.path(), a.password(), open),
                Format::Rar => crate::rar::extract(&a.parts, &a.password_texts(), open),
            },
        }
    }

    /// A small text file's contents (a read-me), at most `limit` bytes.
    pub fn read_text(&self, entry: &str, limit: u64) -> Option<String> {
        let mut bytes = Vec::new();
        self.copy_out(entry, &mut bytes, limit).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// A time in milliseconds since 1970.
fn unix_ms(t: std::time::SystemTime) -> Option<i64> {
    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as i64)
}

/// A date and time, taken as UTC, in milliseconds since 1970 (days from the civil calendar, as
/// Howard Hinnant gives them). A zip's times carry no zone: near enough for a day's comparison.
pub fn civil_ms(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    ((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_days() {
        assert_eq!(civil_ms(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(civil_ms(2000, 3, 1, 0, 0, 0), 951_868_800_000);
        assert_eq!(civil_ms(2026, 10, 22, 12, 30, 15), 1_792_672_215_000);
    }

    #[test]
    fn archives_by_name() {
        assert_eq!(format_of(Path::new("a.ZIP")), Some(Format::Zip));
        assert_eq!(format_of(Path::new("a.7z")), Some(Format::SevenZip));
        assert_eq!(format_of(Path::new("a.part2.rar")), Some(Format::Rar));
        assert_eq!(format_of(Path::new("a.r07")), Some(Format::Rar));
        assert_eq!(format_of(Path::new("a.vpx")), None);
    }

    /// Every locked archive kept for the tests (`testdata/locked`, its README says what each holds),
    /// opened with its password alone, one test as the passwords at hand are the program's.
    #[test]
    fn locked_archives_open_with_their_password() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata").join("locked");
        let text = |l: &Listing, name: &str| {
            let mut bytes = Vec::new();
            match l.copy_out(name, &mut bytes, 1024) {
                Ok(()) => Some(String::from_utf8_lossy(&bytes).into_owned()),
                Err(e) => {
                    eprintln!("{name}: {e}");
                    None
                }
            }
        };
        let all = |l: &Listing| l.entries.iter().map(|e| e.path.clone()).collect::<Vec<_>>();
        let whole = ["zipcrypto.zip", "aes.zip", "data.7z", "headers.7z"];
        let rars_all_locked = ["rar4_encrypted_filenames.rar", "rar4_solid_encrypted.rar", "rar4_solid_encrypted_filenames.rar", "rar5_encrypted_filenames.rar", "rar5_solid_encrypted.rar", "rar5_solid_encrypted_filenames.rar"];
        // no password, or a wrong one: locked, said so
        for pw in [vec![], vec!["wrong".to_string()]] {
            set_passwords(pw);
            for f in whole.iter().chain(&rars_all_locked).chain(&["rar4_encrypted.rar", "rar5_encrypted.rar"]) {
                let e = list(&dir.join(f)).map(|_| ()).unwrap_err();
                assert!(is_locked(&e), "{f}: {e}");
            }
        }
        // its password: listed, every file read out as it was, by one file or all at once
        set_passwords(vec!["wrong".into(), "password".into()]);
        for f in whole {
            let l = list(&dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
            assert_eq!(all(&l), ["a.txt", "b.txt"], "{f}");
            assert_eq!(text(&l, "b.txt").as_deref(), Some("This is from b.txt"), "{f}");
            let mut got = Vec::new();
            l.extract(&mut |name| {
                got.push(name.to_string());
                Ok(Some(Box::new(std::io::sink()) as Box<dyn Write>))
            })
            .unwrap_or_else(|e| panic!("{f}: {e}"));
            assert_eq!(got, ["a.txt", "b.txt"], "{f}");
            // the password kept with it, never shown
            assert!(format!("{l:?}").contains("<password>") && !format!("{l:?}").contains("\"password\""), "{f}");
        }
        for f in rars_all_locked {
            // libarchive reads no solid RAR 4 archive, locked or not ("RAR solid archive support
            // unavailable", at its second file)
            if f.starts_with("rar4_solid") {
                let e = list(&dir.join(f)).map(|_| ()).unwrap_err();
                assert!(e.contains("solid"), "{f}: {e}");
                continue;
            }
            let l = list(&dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
            assert_eq!(all(&l), ["a.txt", "b.txt", "c.txt", "d.txt"], "{f}");
            for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
                assert_eq!(text(&l, name), Some(format!("This is from {name}")), "{f} {name}");
            }
        }
        // b.txt locked with `password`, d.txt with `password2`: RAR 5 checks each file's password, so
        // both open where both are at hand; RAR 4 keeps nothing to check by, the first that opens one
        // opening them all
        let l = list(&dir.join("rar5_encrypted.rar")).map(|_| ()).unwrap_err();
        assert!(is_locked(&l), "{l}");
        set_passwords(vec!["password".into(), "password2".into()]);
        let l = list(&dir.join("rar5_encrypted.rar")).unwrap();
        for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
            assert_eq!(text(&l, name), Some(format!("This is from {name}")), "rar5 {name}");
        }
        let l = list(&dir.join("rar4_encrypted.rar")).unwrap();
        assert_eq!(text(&l, "b.txt").as_deref(), Some("This is from b.txt"));
        assert_eq!(text(&l, "c.txt").as_deref(), Some("This is from c.txt"));
        assert_eq!(text(&l, "d.txt"), None);
        set_passwords(Vec::new());
    }

    #[test]
    fn names_that_climb_out_of_their_folder_are_left_out() {
        for fine in ["table.vpx", "pack/Backglass/a.mp4", "./pack/a.png", "Jaws (1975).zip", "a..b.zip"] {
            assert!(stays_inside(fine), "{fine}");
        }
        for out in ["", "../evil.dll", "pack/../../evil.dll", "pack\\..\\..\\evil.dll", "/Windows/evil.dll", "\\\\server\\share\\x", "C:/Windows/evil.dll", "c:evil.dll", "read-me.txt:hidden", "a\u{0}b"] {
            assert!(!stays_inside(out), "{out:?}");
        }
        // a zip holding one: listed without it
        let dir = std::env::temp_dir().join(format!("vpi-archive-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            for name in ["pack/screens.pup", "pack/../../evil.dll"] {
                z.start_file(name, options).unwrap();
                z.write_all(b"x").unwrap();
            }
            z.finish().unwrap();
        }
        let listed: Vec<String> = list(&path).unwrap().entries.into_iter().map(|e| e.path).collect();
        assert_eq!(listed, ["pack/screens.pup"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
