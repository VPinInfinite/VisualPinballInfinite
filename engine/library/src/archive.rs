//! What a dropped file or folder holds, listed without unpacking it: a folder's
//! files, an archive's — a zip's from its central directory, a 7-Zip archive's
//! from its header (`sevenz-rust2`, Apache-2.0), a RAR archive's, in however
//! many parts, through libarchive (`rar.rs`) — or the one file dropped; and a
//! file of it read out when needed: a read-me, a table to read its info from.
//! Ours.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// One file in a drop, its path relative to the drop's top with `/` between folders.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    /// When it was last changed, in milliseconds since 1970, as the archive or folder keeps it.
    pub modified: Option<i64>,
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

/// An archive dropped: its kind and its file, or files for a RAR archive in parts, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Archive {
    pub format: Format,
    pub parts: Vec<PathBuf>,
}

impl Archive {
    /// Its file, the first of its parts.
    pub fn path(&self) -> &Path {
        &self.parts[0]
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
    let (archive, entries) = match format_of(path) {
        Some(Format::Zip) => {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| fail(&e))?).map_err(|e| fail(&e))?;
            let mut entries = Vec::new();
            for i in 0..zip.len() {
                let f = zip.by_index_raw(i).map_err(|e| fail(&e))?;
                if f.is_dir() {
                    continue;
                }
                let modified = f.last_modified().map(|t| civil_ms(t.year().into(), t.month().into(), t.day().into(), t.hour().into(), t.minute().into(), t.second().into()));
                entries.push(Entry { path: f.name().replace('\\', "/"), size: f.size(), modified });
            }
            (Archive { format: Format::Zip, parts: vec![path.to_path_buf()] }, entries)
        }
        Some(Format::SevenZip) => {
            let a = sevenz_rust2::Archive::open(path).map_err(|e| seven_zip_error(path, e))?;
            // its contents locked with a password, the list of its files not
            if a.blocks.iter().flat_map(|b| &b.coders).any(|c| c.encoder_method_id() == sevenz_rust2::EncoderMethod::ID_AES256_SHA256) {
                return Err(seven_zip_error(path, sevenz_rust2::Error::PasswordRequired));
            }
            let entries = a
                .files
                .iter()
                .filter(|f| !f.is_directory())
                .map(|f| Entry { path: f.name().replace('\\', "/"), size: f.size(), modified: unix_ms(std::time::SystemTime::from(f.last_modified_date())) })
                .collect();
            (Archive { format: Format::SevenZip, parts: vec![path.to_path_buf()] }, entries)
        }
        Some(Format::Rar) => {
            let parts = crate::rar::parts_of(path);
            let entries = crate::rar::list(&parts)?;
            (Archive { format: Format::Rar, parts }, entries)
        }
        None => {
            let meta = std::fs::metadata(path).map_err(|e| fail(&e))?;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let modified = meta.modified().ok().and_then(unix_ms);
            return Ok(Listing { source: Source::File(path.to_path_buf()), entries: vec![Entry { path: name, size: meta.len(), modified }] });
        }
    };
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

/// A 7-Zip archive's error said plainly: one with a password ours can't open.
fn seven_zip_error(path: &Path, e: sevenz_rust2::Error) -> String {
    match e {
        // with no AES in ours, a locked archive's method is unknown to it
        sevenz_rust2::Error::PasswordRequired => format!("{} has a password, which ours can't open: unpack it first with the program it was made for", path.display()),
        sevenz_rust2::Error::UnsupportedCompressionMethod(m) if m.to_uppercase().contains("AES") => {
            format!("{} has a password, which ours can't open: unpack it first with the program it was made for", path.display())
        }
        e => format!("{}: {e}", path.display()),
    }
}

/// One file of a 7-Zip archive into `out`, at most `limit` bytes: its block alone decoded, the files
/// before it in that block read through (a solid archive's files follow one another in one stream).
fn seven_zip_copy(path: &Path, entry: &str, out: &mut dyn Write, limit: u64) -> Result<(), String> {
    let archive = sevenz_rust2::Archive::open(path).map_err(|e| seven_zip_error(path, e))?;
    let index = archive.files.iter().position(|f| f.name().replace('\\', "/") == entry).ok_or_else(|| format!("{}: no {entry} in it", path.display()))?;
    let Some(block) = archive.stream_map.file_block_index[index] else {
        return Ok(()); // an empty file
    };
    let mut source = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let password = sevenz_rust2::Password::empty();
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
fn seven_zip_extract(path: &Path, open: &mut Opener) -> Result<(), String> {
    let archive = sevenz_rust2::Archive::open(path).map_err(|e| seven_zip_error(path, e))?;
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
    let password = sevenz_rust2::Password::empty();
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
                    let mut f = zip.by_name(entry).map_err(|e| format!("{} ({entry}): {e}", a.path().display()))?;
                    copy(&mut f, out)
                }
                Format::SevenZip => seven_zip_copy(a.path(), entry, out, limit),
                Format::Rar => crate::rar::copy_out(&a.parts, entry, out, limit),
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
                        let mut f = zip.by_index(i).map_err(|e| format!("{}: {e}", a.path().display()))?;
                        if f.is_dir() {
                            continue;
                        }
                        let name = f.name().replace('\\', "/");
                        if let Some(mut out) = open(&name)? {
                            std::io::copy(&mut f, &mut out).map_err(|e| format!("{name}: {e}"))?;
                            out.flush().map_err(|e| format!("{name}: {e}"))?;
                        }
                    }
                    Ok(())
                }
                Format::SevenZip => seven_zip_extract(a.path(), open),
                Format::Rar => crate::rar::extract(&a.parts, open),
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
}
