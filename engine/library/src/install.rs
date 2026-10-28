//! A drop's files copied into place, as the user ticked them in the drop window: each written under a passing name beside its place and renamed into it once whole, so
//! a failure halfway leaves nothing half-written; a file already there replaced (the old one to the
//! Recycle Bin, to be brought back), kept beside the new one, or left, as the user chooses
//! (Replace, the old one to the Recycle Bin, or Keep both), row by row
//! (asked each time, with Append as well) — a pack's folder there replaced
//! whole (its old files the new one hasn't to the Recycle Bin too), its new files appended to it,
//! or left; a ROM set dropped in another kind of archive made a zip, the only kind PinMAME reads.
//! Ours.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::archive::{Format, Source};
use crate::drop::Download;
use crate::plan::{There, there_at};

/// What's done with a file already where a new one goes.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Already {
    /// The new one in its place, the old one to the Recycle Bin.
    Replace,
    /// The new one beside it, its name numbered.
    KeepBoth,
    /// The old one left, the new one not copied: for a pack's folder there, none of its files.
    Skip,
    /// A pack's files added to its folder there, a file of the same name replaced (the old one to
    /// the Recycle Bin), the folder's others left.
    Append,
}

/// What was done.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub copied: usize,
    pub replaced: usize,
    pub kept_both: usize,
    /// There already, the same size: left as they were.
    pub same: usize,
    /// Not copied, as chosen.
    pub skipped: usize,
    /// A replaced pack's old files the new one hasn't, sent to the Recycle Bin.
    pub removed: usize,
    pub bytes: u64,
    /// The tables put in place.
    pub tables: Vec<PathBuf>,
    /// Every file put in place, with its kind (`drop::kind_key`'s names), for the table's page.
    #[serde(skip)]
    pub placed: Vec<(PathBuf, &'static str)>,
    /// What couldn't be done, file by file.
    pub problems: Vec<String>,
}

/// One file to copy: the drop's entry, the passing file it's written to, its place, whether it
/// replaces what's there, its size and kind.
struct Job {
    from: String,
    temp: PathBuf,
    to: PathBuf,
    replace: bool,
    size: u64,
    kind: crate::analysis::Kind,
}

/// A free name beside `path`: `name (2).ext`, `name (3).ext`, …; in one of PinUP Popper's media
/// folders, `name01.ext`, `name02.ext` …, as Popper numbers a table's media.
fn numbered(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    if path.parent().is_some_and(is_media_folder) {
        return (1..).map(|n| path.with_file_name(format!("{stem}{n:02}{ext}"))).find(|p| !p.exists()).expect("a free name");
    }
    (2..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("a free name")
}

/// Whether a folder is one of PinUP Popper's media folders (`POPMedia\<system>\<screen>`).
fn is_media_folder(folder: &Path) -> bool {
    folder.parent().and_then(Path::parent).and_then(Path::file_name).is_some_and(|n| n.eq_ignore_ascii_case("POPMedia"))
}

fn passing(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".vpi-part");
    dest.with_file_name(name)
}

/// Copies a download's ticked rows (`include`, one per row) into place, rows nothing placed into
/// the folder the user chose for them (`put`, by row: their paths below the row's folder kept).
/// `recycle` sends a replaced file away (the Recycle Bin: `to_recycle_bin`); `progress` hears the
/// bytes done of the total as each file starts.
pub fn install(d: &Download, include: &[bool], put: &HashMap<usize, PathBuf>, already: Already, recycle: &dyn Fn(&Path) -> Result<(), String>, progress: &mut dyn FnMut(u64, u64)) -> Result<Added, String> {
    install_rows(d, include, put, already, &HashMap::new(), recycle, progress)
}

/// As `install`, with what's done with a file already there chosen row by row (`rows`), `already`
/// for a row with no choice.
pub fn install_rows(
    d: &Download,
    include: &[bool],
    put: &HashMap<usize, PathBuf>,
    already: Already,
    rows: &HashMap<usize, Already>,
    recycle: &dyn Fn(&Path) -> Result<(), String>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Added, String> {
    let listing = d.listing.as_ref().ok_or("This download couldn't be read, so there's nothing to add from it.")?;
    let mut added = Added::default();
    let mut jobs: Vec<Job> = Vec::new();
    // packs' folders replaced whole: each folder, and the files the new pack puts in it
    let mut whole: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
    // the media put in each of PinUP Popper's folders, by kind of file, to number them
    let mut media_named: HashMap<(PathBuf, String), usize> = HashMap::new();
    for (r, (row, on)) in d.rows.iter().zip(include.iter().chain(std::iter::repeat(&false))).enumerate() {
        let already = rows.get(&r).copied().unwrap_or(already);
        // a pack's folder there already: its choice covers the pack, new files and all
        let folder = row.to.as_ref().filter(|t| t.is_dir() && put.get(&r).is_none()).cloned();
        if *on
            && let Some(f) = &folder
            && already == Already::Replace
        {
            whole.push((f.clone(), Vec::new()));
        }
        for &i in &row.steps {
            let step = &d.steps[i];
            let dest = match put.get(&r) {
                // one of PinUP Popper's media folders: named for its table, as Popper numbers them
                Some(folder) if is_media_folder(folder) && d.stem.is_some() => {
                    // as Popper names them: for its own name for the game, where it has one
                    let stem = crate::popper::media_name_in(folder, &format!("{}.vpx", d.stem.as_deref().unwrap_or_default()), d.stem.as_deref().unwrap_or_default());
                    let ext = step.from.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
                    let n = media_named.entry((folder.clone(), ext.to_lowercase())).or_insert(0);
                    let name = if *n == 0 { format!("{stem}.{ext}") } else { format!("{stem}{:02}.{ext}", *n) };
                    *n += 1;
                    Some(folder.join(name))
                }
                // one of PinballX's or PinballY's: named for the table as its game list names it,
                // the one it shows first, the rest numbered beside it
                Some(folder) if crate::frontends::is_media_folder(folder) && d.media_name.is_some() => {
                    let name = d.media_name.as_deref().unwrap_or_default();
                    let ext = step.from.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
                    let n = media_named.entry((folder.clone(), String::new())).or_insert(0);
                    let file = if *n == 0 { format!("{name}.{ext}") } else { format!("{name} {}.{ext}", *n) };
                    *n += 1;
                    Some(folder.join(file))
                }
                Some(folder) => {
                    let below = match &row.under {
                        Some(top) => step.from.strip_prefix(top.as_str()).map(|p| p.trim_start_matches('/')).unwrap_or(&step.from),
                        None => step.from.rsplit('/').next().unwrap_or(&step.from),
                    };
                    Some(folder.join(below))
                }
                None => step.to.clone(),
            };
            let Some(dest) = dest else { continue };
            if !*on {
                // left out: there already, or as the user chose
                if there_at(&dest, &step.from, step.size) == There::SameSize {
                    added.same += 1;
                } else {
                    added.skipped += 1;
                }
                continue;
            }
            if let Some((_, kept)) = whole.last_mut().filter(|(f, _)| folder.as_ref() == Some(f)) {
                kept.push(dest.clone());
            }
            let (to, replace) = match there_at(&dest, &step.from, step.size) {
                // a pack left as it is takes none of the new one's files
                There::Nothing if folder.is_some() && already == Already::Skip => {
                    added.skipped += 1;
                    continue;
                }
                There::Nothing => (dest, false),
                There::SameSize => {
                    added.same += 1;
                    continue;
                }
                There::Different | There::Named => match already {
                    Already::Replace | Already::Append => (dest, true),
                    Already::KeepBoth => {
                        added.kept_both += 1;
                        (numbered(&dest), false)
                    }
                    Already::Skip => {
                        added.skipped += 1;
                        continue;
                    }
                },
            };
            jobs.push(Job { from: step.from.clone(), temp: passing(&to), to, replace, size: step.size, kind: step.kind });
        }
    }
    if jobs.is_empty() {
        replace_whole(&whole, recycle, &mut added);
        return Ok(added);
    }
    let total: u64 = jobs.iter().map(|j| j.size).sum();
    // a ROM set dropped whole (the plan's one step names the archive, not a file in it): the archive
    // itself, or its files made a zip
    let whole_set = match &listing.source {
        Source::Archive(a) if d.steps.len() == 1 && d.steps[0].kind == crate::analysis::Kind::Rom && !listing.entries.iter().any(|e| e.path == d.steps[0].from) => Some(a),
        _ => None,
    };
    let result = if let Some(a) = whole_set {
        progress(0, jobs[0].size);
        whole_rom_set(listing, a.format, a.path(), &jobs[0].temp)
    } else {
        let wanted: HashMap<&str, (&Path, u64)> = jobs.iter().map(|j| (j.from.as_str(), (j.temp.as_path(), j.size))).collect();
        let mut done = 0u64;
        listing.extract(&mut |entry| {
            let Some((temp, size)) = wanted.get(entry) else { return Ok(None) };
            progress(done, total);
            done += size;
            if let Some(dir) = temp.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let f = std::fs::File::create(temp).map_err(|e| format!("{}: {e}", temp.display()))?;
            Ok(Some(Box::new(std::io::BufWriter::new(f))))
        })
    };
    if let Err(e) = result {
        for j in &jobs {
            let _ = std::fs::remove_file(&j.temp);
        }
        return Err(e);
    }
    progress(total, total);
    for Job { from, temp, to, replace, size, kind } in jobs {
        if !temp.exists() {
            added.problems.push(format!("{from}: not found in the download"));
            continue;
        }
        if kind == crate::analysis::Kind::Patch {
            patched(d, &from, &temp, &to, replace, recycle, &mut added);
            continue;
        }
        if replace {
            if let Err(e) = recycle(&to) {
                let _ = std::fs::remove_file(&temp);
                added.problems.push(format!("{}: the file there couldn't be moved to the Recycle Bin ({e}), so it was left", to.display()));
                continue;
            }
            added.replaced += 1;
        }
        match std::fs::rename(&temp, &to) {
            Ok(()) => {
                added.copied += 1;
                added.bytes += size;
                if to.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx")) {
                    added.tables.push(to.clone());
                }
                added.placed.push((to, crate::drop::kind_key(kind)));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp);
                added.problems.push(format!("{}: {e}", to.display()));
            }
        }
    }
    replace_whole(&whole, recycle, &mut added);
    Ok(added)
}

/// Packs' folders replaced whole, once the new packs' files are in: each folder's files the new pack
/// hasn't sent to the Recycle Bin, so the folder holds the new pack alone.
fn replace_whole(whole: &[(PathBuf, Vec<PathBuf>)], recycle: &dyn Fn(&Path) -> Result<(), String>, added: &mut Added) {
    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                files(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    // a place as one spelling: the drop's paths keep its own slashes
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    for (folder, kept) in whole {
        let keep: std::collections::HashSet<String> = kept.iter().map(|p| norm(p)).collect();
        let mut there = Vec::new();
        files(folder, &mut there);
        for old in there.into_iter().filter(|p| !keep.contains(&norm(p))) {
            match recycle(&old) {
                Ok(()) => added.removed += 1,
                Err(e) => added.problems.push(format!("{}: couldn't be moved to the Recycle Bin ({e})", old.display())),
            }
        }
    }
}

/// A VPU patch (`temp`, written out of the drop) applied: the modded table made from its
/// original — the table the download is for, which stays as it is — and read as a table before it
/// goes in at `to` (one there replaced when `replace`, to the Recycle Bin); the original's
/// backglass, settings and POV copied for it, named for it, where it has none.
fn patched(d: &Download, from: &str, temp: &Path, to: &Path, replace: bool, recycle: &dyn Fn(&Path) -> Result<(), String>, added: &mut Added) {
    let original = d.stem.as_deref().map(|stem| to.with_file_name(format!("{stem}.vpx")));
    let made = (|| -> Result<PathBuf, String> {
        let original = original.as_deref().ok_or("which table it patches isn't known")?;
        let patch = std::fs::read(temp).map_err(|e| format!("{}: {e}", temp.display()))?;
        let mut name = to.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(".vpi-made");
        let made = to.with_file_name(name);
        crate::patch::make(original, &patch, &made)?;
        Ok(made)
    })();
    let _ = std::fs::remove_file(temp);
    let made = match made {
        Ok(m) => m,
        Err(e) => {
            added.problems.push(format!("{from}: {e}"));
            return;
        }
    };
    if replace {
        if let Err(e) = recycle(to) {
            let _ = std::fs::remove_file(&made);
            added.problems.push(format!("{}: the file there couldn't be moved to the Recycle Bin ({e}), so it was left", to.display()));
            return;
        }
        added.replaced += 1;
    }
    if let Err(e) = std::fs::rename(&made, to) {
        let _ = std::fs::remove_file(&made);
        added.problems.push(format!("{}: {e}", to.display()));
        return;
    }
    added.copied += 1;
    added.bytes += std::fs::metadata(to).map_or(0, |m| m.len());
    added.tables.push(to.to_path_buf());
    added.placed.push((to.to_path_buf(), "table"));
    let Some(original) = original else { return };
    for (ext, kind) in [("directb2s", "backglass"), ("ini", "tableSettings"), ("pov", "pov"), ("vpp", "physics")] {
        let (from, copy) = (original.with_extension(ext), to.with_extension(ext));
        if from.is_file() && !copy.exists() {
            match std::fs::copy(&from, &copy) {
                Ok(n) => {
                    added.copied += 1;
                    added.bytes += n;
                    added.placed.push((copy, kind));
                }
                Err(e) => added.problems.push(format!("{}: {e}", copy.display())),
            }
        }
    }
}

/// A ROM set dropped whole into `out`: a zip as it is; another kind of archive's files written into
/// a new zip, the only kind PinMAME reads.
fn whole_rom_set(listing: &crate::archive::Listing, format: Format, archive: &Path, out: &Path) -> Result<(), String> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if format == Format::Zip {
        return std::fs::copy(archive, out).map(|_| ()).map_err(|e| format!("{}: {e}", out.display()));
    }
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let zip = std::rc::Rc::new(std::cell::RefCell::new(zip::ZipWriter::new(std::io::BufWriter::new(file))));
    struct Into(std::rc::Rc<std::cell::RefCell<zip::ZipWriter<std::io::BufWriter<std::fs::File>>>>);
    impl Write for Into {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    listing.extract(&mut |entry| {
        // the set's files at the zip's top, as PinMAME looks for them
        let name = entry.rsplit('/').next().unwrap_or(entry);
        zip.borrow_mut().start_file(name, options).map_err(|e| format!("{name}: {e}"))?;
        Ok(Some(Box::new(Into(zip.clone()))))
    })?;
    let zip = std::rc::Rc::try_unwrap(zip).map_err(|_| "the zip still in use".to_string())?.into_inner();
    zip.finish().map_err(|e| format!("{}: {e}", out.display()))?.flush().map_err(|e| format!("{}: {e}", out.display()))
}

/// A PinUP pack's folder renamed where it lies to `name`, the name its table's script hands PinUP,
/// so the player finds it (a table page's Fix, Needs attention's Rename all).
/// Where it is now.
pub fn rename_pack(from: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) || name.trim_matches('.').is_empty() {
        return Err(format!("\"{name}\" can't be a folder's name."));
    }
    let to = from.with_file_name(name);
    // the same folder in another case: Windows renames it in two steps
    let same = to.to_string_lossy().eq_ignore_ascii_case(&from.to_string_lossy());
    if to.exists() && !same {
        return Err(format!("A folder called {name} is there already: {}", to.display()));
    }
    if same {
        let between = from.with_file_name(format!("{name}.renaming"));
        std::fs::rename(from, &between).and_then(|_| std::fs::rename(&between, &to)).map_err(|e| format!("{}: {e}", from.display()))?;
    } else {
        std::fs::rename(from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    }
    Ok(to)
}

/// A file or folder to the Recycle Bin, where Explorer can bring it back (Windows' `SHFileOperationW`,
/// its undo kept, nothing asked — but for one too big for the Recycle Bin, which Windows would
/// otherwise delete for good: it asks first, and a no leaves it).
#[cfg(windows)]
pub fn to_recycle_bin(path: &Path) -> Result<(), String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    struct ShFileOp {
        hwnd: *mut c_void,
        func: u32,
        from: *const u16,
        to: *const u16,
        flags: u16,
        aborted: i32,
        mappings: *mut c_void,
        title: *const u16,
    }
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHFileOperationW(op: *mut ShFileOp) -> i32;
    }
    const FO_DELETE: u32 = 3;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;
    const FOF_WANTNUKEWARNING: u16 = 0x4000;
    let full = std::path::absolute(path).map_err(|e| e.to_string())?;
    // a list of names, each ended by a nought, the list by another
    let mut from: Vec<u16> = full.as_os_str().encode_wide().collect();
    from.extend([0, 0]);
    let mut op = ShFileOp {
        hwnd: std::ptr::null_mut(),
        func: FO_DELETE,
        from: from.as_ptr(),
        to: std::ptr::null(),
        flags: FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO | FOF_NOERRORUI | FOF_WANTNUKEWARNING,
        aborted: 0,
        mappings: std::ptr::null_mut(),
        title: std::ptr::null(),
    };
    // SAFETY: a filled-in operation whose list lives until it returns
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 || op.aborted != 0 { Err(format!("Windows said {r:#x}")) } else { Ok(()) }
}

#[cfg(not(windows))]
pub fn to_recycle_bin(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drop::{Known, inspect};
    use crate::plan::Layout;
    use crate::vps::Database;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-install-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn layout(top: &Path) -> Layout {
        Layout { tables: top.join("Tables"), scripts: top.join("Scripts"), vpinmame: top.join("VPinMAME"), pup_videos: top.join("PUPVideos"), data: top.join("data"), front_ends: Vec::new(), media_front: None, per_game: false }
    }

    #[test]
    fn a_drop_goes_into_place_and_a_second_asks_what_to_do() {
        let top = scratch("place");
        let drop = top.join("drop");
        std::fs::create_dir_all(drop.join("pack/Backglass")).unwrap();
        std::fs::write(drop.join("Taxi.directb2s"), b"backglass").unwrap();
        std::fs::write(drop.join("pack/screens.pup"), b"screens").unwrap();
        std::fs::write(drop.join("pack/Backglass/a.mp4"), b"video").unwrap();
        let known = [Known { stem: "Taxi (Williams 1988)".into(), game_name: Some("taxi_l4".into()), pup_names: vec!["taxi".into()], vps_game: None, helpers: Vec::new(), music: Vec::new(), file: None }];
        let l = layout(&top);
        let db = Database { games: Vec::new() };
        let gone = |p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string());
        // the backglass isn't named for the library's table: dropped on it, its files are for it
        let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::On(&known[0]), &[])[0];
        assert_eq!(d.for_table.as_deref(), Some("Taxi (Williams 1988)"));
        let include: Vec<bool> = d.rows.iter().map(|r| r.include).collect();
        let none = HashMap::new();
        let a = install(d, &include, &none, Already::Skip, &gone, &mut |_, _| {}).unwrap();
        assert_eq!((a.copied, a.problems.len()), (3, 0));
        assert_eq!(std::fs::read(l.tables.join("Taxi (Williams 1988).directb2s")).unwrap(), b"backglass");
        // the pack named as its table's script asks PinUP for it
        assert_eq!(std::fs::read(l.pup_videos.join("taxi/Backglass/a.mp4")).unwrap(), b"video");
        // again: the same files are left; a changed one kept beside the old when asked
        std::fs::write(drop.join("Taxi.directb2s"), b"a newer backglass").unwrap();
        let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::On(&known[0]), &[])[0];
        let row = d.rows.iter().find(|r| r.kind == "backglass").unwrap();
        assert_eq!(row.state, "replace");
        let include: Vec<bool> = d.rows.iter().map(|r| r.include).collect();
        let a = install(d, &include, &none, Already::KeepBoth, &gone, &mut |_, _| {}).unwrap();
        // the pack's files, there already, aren't ticked: counted as there already
        assert_eq!((a.copied, a.kept_both, a.same, a.skipped), (1, 1, 2, 0));
        assert_eq!(std::fs::read(l.tables.join("Taxi (Williams 1988) (2).directb2s")).unwrap(), b"a newer backglass");
        // and replaced when asked, the old one sent away
        let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::On(&known[0]), &[])[0];
        let include: Vec<bool> = d.rows.iter().map(|r| r.include).collect();
        let a = install(d, &include, &none, Already::Replace, &gone, &mut |_, _| {}).unwrap();
        assert_eq!(a.replaced, 1);
        assert_eq!(std::fs::read(l.tables.join("Taxi (Williams 1988).directb2s")).unwrap(), b"a newer backglass");
        // nothing passing left behind
        let leftovers = std::fs::read_dir(&l.tables).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".vpi-part")).count();
        assert_eq!(leftovers, 0);
        let _ = std::fs::remove_dir_all(&top);
    }

    #[test]
    fn a_pack_there_replaced_whole_appended_to_or_left() {
        let top = scratch("pack");
        let drop = top.join("drop");
        std::fs::create_dir_all(drop.join("pack/Backglass")).unwrap();
        std::fs::write(drop.join("pack/screens.pup"), b"new screens").unwrap();
        std::fs::write(drop.join("pack/Backglass/b.mp4"), b"new video").unwrap();
        let known = [Known { stem: "Taxi".into(), game_name: Some("taxi_l4".into()), pup_names: vec!["taxi".into()], vps_game: None, helpers: Vec::new(), music: Vec::new(), file: None }];
        let l = layout(&top);
        let db = Database { games: Vec::new() };
        let gone = |p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string());
        let pack = l.pup_videos.join("taxi");
        let old = || {
            let _ = std::fs::remove_dir_all(&pack);
            std::fs::create_dir_all(pack.join("Backglass")).unwrap();
            std::fs::write(pack.join("screens.pup"), b"old").unwrap();
            std::fs::write(pack.join("Backglass/a.mp4"), b"old video").unwrap();
        };
        let run = |choice: Already| {
            old();
            let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::On(&known[0]), &[])[0];
            let r = d.rows.iter().position(|r| r.kind == "pupPack").unwrap();
            install_rows(d, &vec![true; d.rows.len()], &HashMap::new(), Already::Skip, &HashMap::from([(r, choice)]), &gone, &mut |_, _| {}).unwrap()
        };
        // left: nothing of the new pack comes in
        let left = run(Already::Skip);
        let left_new = pack.join("Backglass/b.mp4").exists();
        // appended: its new file in, the same-named one replaced, the old pack's others kept
        let appended = run(Already::Append);
        let appended_files = (std::fs::read(pack.join("screens.pup")).unwrap(), pack.join("Backglass/a.mp4").exists(), pack.join("Backglass/b.mp4").exists());
        // replaced whole: the old pack's file the new one hasn't goes too
        let replaced = run(Already::Replace);
        let replaced_files = (pack.join("Backglass/a.mp4").exists(), std::fs::read(pack.join("Backglass/b.mp4")).unwrap());
        let _ = std::fs::remove_dir_all(&top);
        assert!(!left_new && left.copied == 0, "{left:?}");
        assert_eq!((appended.copied, appended.replaced), (2, 1));
        assert_eq!(appended_files, (b"new screens".to_vec(), true, true));
        assert_eq!((replaced.replaced, replaced.removed), (1, 1));
        assert_eq!(replaced_files, (false, b"new video".to_vec()));
    }

    #[test]
    fn media_into_popper_folders_named_for_the_table() {
        let top = scratch("media");
        let drop = top.join("drop");
        std::fs::create_dir_all(drop.join("Wheel")).unwrap();
        std::fs::create_dir_all(drop.join("Playfield")).unwrap();
        std::fs::write(drop.join("Wheel/logo.png"), b"wheel").unwrap();
        std::fs::write(drop.join("Wheel/logo2.png"), b"wheel two").unwrap();
        std::fs::write(drop.join("Playfield/attract.mp4"), b"video").unwrap();
        std::fs::write(drop.join("Taxi.directb2s"), b"backglass").unwrap();
        std::fs::write(drop.join("side art.png"), b"art").unwrap();
        let l = layout(&top);
        let media = top.join("POPMedia/Visual Pinball X");
        std::fs::create_dir_all(&media).unwrap();
        let known = [Known { stem: "Taxi".into(), game_name: Some("taxi_l4".into()), pup_names: Vec::new(), vps_game: None, helpers: Vec::new(), music: Vec::new(), file: None }];
        let db = Database { games: Vec::new() };
        let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::On(&known[0]), &[])[0];
        // the art nothing could tell the screen of: the user's to put in one of Popper's folders
        let r = d.rows.iter().position(|r| r.state == "unsorted").unwrap();
        let topper = crate::drop::places(d, &l).into_iter().find(|p| p.0 == "media:Topper").unwrap().2;
        let mut include: Vec<bool> = d.rows.iter().map(|r| r.include).collect();
        include[r] = true;
        let gone = |p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string());
        let a = install(d, &include, &HashMap::from([(r, topper)]), Already::Skip, &gone, &mut |_, _| {}).unwrap();
        let read = |p: &str| std::fs::read(media.join(p)).ok();
        let got = (read("Wheel/Taxi.png"), read("Wheel/Taxi01.png"), read("PlayField/Taxi.mp4"), read("Topper/Taxi.png"));
        let _ = std::fs::remove_dir_all(&top);
        assert!(a.problems.is_empty(), "{:?}", a.problems);
        // two wheels: the first named for the table, the next numbered, as Popper numbers them
        let wheels: std::collections::BTreeSet<Vec<u8>> = [got.0.clone(), got.1.clone()].into_iter().flatten().collect();
        assert_eq!(wheels, [b"wheel".to_vec(), b"wheel two".to_vec()].into());
        assert_eq!((got.2.as_deref(), got.3.as_deref()), (Some(&b"video"[..]), Some(&b"art"[..])));
    }

    #[test]
    fn media_into_pinballxs_folders_named_as_its_game_list_names_the_table() {
        let top = scratch("pinballx");
        let drop = top.join("drop");
        std::fs::create_dir_all(drop.join("Wheel")).unwrap();
        std::fs::create_dir_all(drop.join("Playfield")).unwrap();
        std::fs::write(drop.join("Wheel/logo.png"), b"wheel").unwrap();
        std::fs::write(drop.join("Wheel/logo2.png"), b"wheel two").unwrap();
        std::fs::write(drop.join("Playfield/attract.mp4"), b"video").unwrap();
        std::fs::write(drop.join("Taxi.directb2s"), b"backglass").unwrap();
        std::fs::write(drop.join("side art.png"), b"art").unwrap();
        let mut l = layout(&top);
        // PinballX playing the library's tables, its game list describing the table; no PinUP Popper
        let px = top.join("PinballX");
        std::fs::create_dir_all(px.join("Config")).unwrap();
        let ini = format!("[VisualPinball]\r\nEnabled=True\r\nTablePath={}\r\n", l.tables.display());
        let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain(ini.encode_utf16().flat_map(u16::to_le_bytes)).collect();
        std::fs::write(px.join("Config/PinballX.ini"), utf16).unwrap();
        std::fs::create_dir_all(px.join("Databases/Visual Pinball")).unwrap();
        std::fs::write(px.join("Databases/Visual Pinball/Visual Pinball.xml"), b"<menu><game name=\"Taxi\"><description>Taxi (Williams 1988)</description></game></menu>").unwrap();
        std::fs::create_dir_all(&l.tables).unwrap();
        l.front_ends = vec![px.clone()];
        let known = [Known { stem: "Taxi".into(), game_name: Some("taxi_l4".into()), pup_names: Vec::new(), vps_game: None, helpers: Vec::new(), music: Vec::new(), file: None }];
        let db = Database { games: Vec::new() };
        let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::On(&known[0]), &[])[0];
        // the art nothing could tell the screen of: the user's to put in PinballX's topper pictures
        let r = d.rows.iter().position(|r| r.state == "unsorted").unwrap();
        let places = crate::drop::places(d, &l);
        let topper = places.iter().find(|p| p.0 == "media:Topper").unwrap();
        assert_eq!(topper.1, "PinballX: topper pictures");
        let mut include: Vec<bool> = d.rows.iter().map(|r| r.include).collect();
        include[r] = true;
        let gone = |p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string());
        let a = install(d, &include, &HashMap::from([(r, topper.2.clone())]), Already::Skip, &gone, &mut |_, _| {}).unwrap();
        let media = px.join("Media/Visual Pinball");
        let read = |p: &str| std::fs::read(media.join(p)).ok();
        let name = "Taxi (Williams 1988)";
        let got = (read(&format!("Wheel Images/{name}.png")), read(&format!("Wheel Images/{name} 1.png")), read(&format!("Table Videos/{name}.mp4")), read(&format!("Topper Images/{name}.png")));
        let _ = std::fs::remove_dir_all(&top);
        assert!(a.problems.is_empty(), "{:?}", a.problems);
        // two wheels: the first the one PinballX shows, the next numbered beside it
        let wheels: std::collections::BTreeSet<Vec<u8>> = [got.0.clone(), got.1.clone()].into_iter().flatten().collect();
        assert_eq!(wheels, [b"wheel".to_vec(), b"wheel two".to_vec()].into());
        assert_eq!((got.2.as_deref(), got.3.as_deref()), (Some(&b"video"[..]), Some(&b"art"[..])));
    }

    #[test]
    fn files_nothing_placed_go_where_the_user_puts_them() {
        let top = scratch("put");
        let drop = top.join("drop");
        std::fs::create_dir_all(drop.join("PUP Alt Audio/Backglass")).unwrap();
        std::fs::write(drop.join("PUP Alt Audio/Backglass/hello.mp4"), b"mp4").unwrap();
        std::fs::write(drop.join("Taxi.directb2s"), b"backglass").unwrap();
        let known = [Known { stem: "Taxi".into(), game_name: Some("taxi_l4".into()), pup_names: vec!["taxi".into()], vps_game: None, helpers: Vec::new(), music: Vec::new(), file: None }];
        let l = layout(&top);
        let db = Database { games: Vec::new() };
        let d = &inspect(std::slice::from_ref(&drop), &db, &known, &l, crate::drop::Tie::Found, &[])[0];
        // one row for the folder nothing placed, unticked until the user puts it somewhere
        let r = d.rows.iter().position(|r| r.kind == "folder").unwrap();
        assert_eq!((d.rows[r].name.as_str(), d.rows[r].state.as_str(), d.rows[r].include), ("PUP Alt Audio", "unsorted", false));
        let pack = crate::drop::places(d, &l).into_iter().find(|p| p.0 == "pupPack").unwrap().2;
        assert_eq!(pack, l.pup_videos.join("taxi"));
        let mut include: Vec<bool> = d.rows.iter().map(|r| r.include).collect();
        include[r] = true;
        let put = HashMap::from([(r, pack.clone())]);
        let gone = |p: &Path| std::fs::remove_file(p).map_err(|e| e.to_string());
        let a = install(d, &include, &put, Already::Skip, &gone, &mut |_, _| {}).unwrap();
        assert_eq!(a.copied, 2);
        // its folders below the one it was in kept
        assert_eq!(std::fs::read(pack.join("Backglass/hello.mp4")).unwrap(), b"mp4");
        let _ = std::fs::remove_dir_all(&top);
    }
}
