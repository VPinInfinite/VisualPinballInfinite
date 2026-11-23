//! Every ROM set the library has, and those its tables ask for that aren't there, at once — the ROM
//! manager's list: each zip in VPinMAME's `roms` folder and in a table's own `pinmame\roms` (VPX
//! 10.8.1's layout), told by its name — or, where PinMAME knows no set of that name, by its files'
//! checksums — and checked as PinMAME loads it (a clone's files its parent holds taken from the
//! parent's zip beside it), with the tables that play it, directly or as the set a clone of it is
//! based on. Nothing is changed. Ours.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::plan::Layout;
use crate::roms::{self, Found, Of, Set};

/// One set, there or asked for.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The set's name, in lower case: its zip's, else the one a table asks for.
    pub set: String,
    /// PinMAME's own name for it.
    pub description: Option<String>,
    /// The set it's a clone of.
    pub parent: Option<String>,
    /// Its zip; None where a table asks for it and it isn't there.
    pub zip: Option<PathBuf>,
    pub size: u64,
    /// What it is: `good`, `files missing`, `wrong files`, `needs its parent`, `misnamed`,
    /// `unknown`, `unreadable`, `not installed`.
    pub status: &'static str,
    /// How it looks — good, warn or bad, as a table page's checks look — and in words.
    pub look: &'static str,
    pub text: String,
    /// For a zip named for no set PinMAME knows: the set its files are.
    pub holds: Option<String>,
    /// The tables that play it.
    pub used_by: Vec<PathBuf>,
    /// The tables that play a clone of it from the same folder, which takes files from it.
    pub parent_of: Vec<PathBuf>,
    /// For a set no table plays: the tables that play another version of its game, with the set
    /// each plays.
    pub other_version_of: Vec<(PathBuf, String)>,
}

impl Entry {
    /// No table plays it, nor a clone of it.
    pub fn unused(&self) -> bool {
        self.zip.is_some() && self.used_by.is_empty() && self.parent_of.is_empty()
    }
}

/// The folder PinMAME plays a table's ROMs from: its own `pinmame\roms` where it has one, as VPX
/// 10.8.1's plugin looks first, else VPinMAME's.
pub fn rom_folder(layout: &Layout, table: &Path) -> PathBuf {
    table.parent().map(|d| d.join("pinmame").join("roms")).filter(|d| d.is_dir()).unwrap_or_else(|| layout.vpinmame.join("roms"))
}

fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// The set a zip's files are, by their checksums: of the sets PinMAME knows whose own files — those
/// it needs, not its parent's — are all there, the one with most of its files there.
pub fn identify(found: &[Found], all: &[Set]) -> Option<String> {
    let has = |crc: &str, size: u64| u32::from_str_radix(crc, 16).ok().filter(|c| *c != 0).is_some_and(|crc| found.iter().any(|f| f.crc == crc && f.size == size));
    let mut best: Option<(usize, usize, &str)> = None;
    for s in all {
        let own: Vec<_> = s.files.iter().filter(|e| !e.optional && !e.no_dump && !e.in_parent && u32::from_str_radix(&e.crc, 16).is_ok_and(|c| c != 0)).collect();
        if own.is_empty() || !own.iter().all(|e| has(&e.crc, e.size)) {
            continue;
        }
        let matched = s.files.iter().filter(|e| has(&e.crc, e.size)).count();
        if best.is_none_or(|(o, m, _)| (own.len(), matched) > (o, m)) {
            best = Some((own.len(), matched, &s.set));
        }
    }
    best.map(|(_, _, s)| s.to_lowercase())
}

/// Every set the library has, and those `tables` ask for (each table with its set, after
/// VPinMAME's aliases) that aren't there; `all`, every set PinMAME knows with its files
/// (`roms::all_rom_files`). Sets there first, by name, then those asked for.
pub fn inventory(layout: &Layout, tables: &[(PathBuf, String)], all: &[Set]) -> Vec<Entry> {
    let by_name: HashMap<String, &Set> = all.iter().map(|s| (s.set.to_lowercase(), s)).collect();
    let parent_of = |set: &str| by_name.get(set).and_then(|s| s.parent()).map(str::to_lowercase);
    // VPinMAME's folder, and each table's own
    let mut folders: Vec<PathBuf> = vec![layout.vpinmame.join("roms")];
    for (t, _) in tables {
        let f = rom_folder(layout, t);
        if !folders.iter().any(|x| key(x) == key(&f)) {
            folders.push(f);
        }
    }
    // the tables, by the folder and set PinMAME plays them from
    let mut wants: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();
    for (t, set) in tables {
        wants.entry((key(&rom_folder(layout, t)), set.to_lowercase())).or_default().push(t.clone());
    }
    let mut out: Vec<Entry> = Vec::new();
    let mut there: HashSet<(String, String)> = HashSet::new();
    for folder in &folders {
        let mut zips: Vec<PathBuf> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")))
            .collect();
        zips.sort_by_key(|p| key(p));
        let names: HashSet<String> = zips.iter().filter_map(|z| z.file_stem().map(|s| s.to_string_lossy().to_lowercase())).collect();
        let fkey = key(folder);
        let files_of = |name: &str| std::fs::File::open(folder.join(format!("{name}.zip"))).map_err(|e| e.to_string()).and_then(roms::zip_files);
        for zip in zips {
            let name = zip.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            there.insert((fkey.clone(), name.clone()));
            let size = std::fs::metadata(&zip).map_or(0, |m| m.len());
            let used_by = wants.get(&(fkey.clone(), name.clone())).cloned().unwrap_or_default();
            let mut parent_of_tables: Vec<PathBuf> = wants.iter().filter(|((f, set), _)| *f == fkey && parent_of(set).as_deref() == Some(name.as_str())).flat_map(|(_, ts)| ts.clone()).collect();
            parent_of_tables.sort();
            let entry = |status, look, text: String, set: Option<&Set>, holds| Entry {
                set: name.clone(),
                description: set.and_then(|s| s.description.clone()),
                parent: set.and_then(|s| s.parent()).map(str::to_lowercase),
                zip: Some(zip.clone()),
                size,
                status,
                look,
                text,
                holds,
                used_by: used_by.clone(),
                parent_of: parent_of_tables.clone(),
                other_version_of: Vec::new(),
            };
            let files = match files_of(&name) {
                Ok(f) => f,
                Err(e) => {
                    out.push(entry("unreadable", "bad", format!("Its zip couldn't be read: {e}"), by_name.get(&name).copied(), None));
                    continue;
                }
            };
            match by_name.get(&name).copied() {
                Some(s) => {
                    let mut found = files;
                    let parent_there = s.parent().is_some_and(|p| names.contains(&p.to_lowercase()));
                    if let Some(p) = s.parent().filter(|_| parent_there)
                        && let Ok(more) = files_of(&p.to_lowercase())
                    {
                        found.extend(more);
                    }
                    let c = roms::check(s, &found);
                    let (look, text) = roms::verdict(&c, parent_there, Of::Table);
                    let status = if !c.missing.is_empty() {
                        "files missing"
                    } else if !c.wrong.is_empty() {
                        "wrong files"
                    } else if !c.from_parent.is_empty() && !parent_there {
                        "needs its parent"
                    } else {
                        "good"
                    };
                    out.push(entry(status, look, text, Some(s), None));
                }
                None => match identify(&files, all) {
                    Some(h) => {
                        let what = by_name.get(&h).and_then(|s| s.description.clone()).map_or(String::new(), |d| format!(" ({d})"));
                        let text = format!("PinMAME knows no set called {name}, but its files are {h}{what}: PinMAME looks for them as {h}.zip.");
                        out.push(entry("misnamed", "warn", text, None, Some(h)));
                    }
                    None => out.push(entry("unknown", "warn", format!("PinMAME knows no set called {name}, and its files aren't any set's it knows."), None, None)),
                },
            }
        }
    }
    // asked for and not there
    let mut asked: Vec<((String, String), Vec<PathBuf>)> = wants.into_iter().filter(|(k, _)| !there.contains(k)).collect();
    asked.sort_by(|a, b| a.0.1.cmp(&b.0.1));
    for ((_, set), mut ts) in asked {
        ts.sort();
        let s = by_name.get(&set).copied();
        let text = match s {
            Some(_) => format!("A table plays {set}, which isn't installed: get it from the table's VPS page."),
            None => format!("A table asks for {set}, which PinMAME doesn't know: its script may name it wrongly (Change the ROM on its page)."),
        };
        out.push(Entry {
            set: set.clone(),
            description: s.and_then(|s| s.description.clone()),
            parent: s.and_then(|s| s.parent()).map(str::to_lowercase),
            zip: None,
            size: 0,
            status: "not installed",
            look: "bad",
            text,
            holds: None,
            used_by: ts,
            parent_of: Vec::new(),
            other_version_of: Vec::new(),
        });
    }
    // a set no table plays: the tables that play another version of its game (its parent's
    // family: the parent and its clones)
    let root = |set: &str| parent_of(set).unwrap_or_else(|| set.to_string());
    for e in out.iter_mut().filter(|e| e.unused()) {
        let mine = root(e.holds.as_deref().unwrap_or(&e.set));
        let mut others: Vec<(PathBuf, String)> = tables.iter().filter(|(_, set)| root(&set.to_lowercase()) == mine).map(|(t, set)| (t.clone(), set.to_lowercase())).collect();
        others.sort();
        e.other_version_of = others;
    }
    out
}

/// A fix the ROMs page can offer for a set, each asked first.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fix {
    /// A set under a name PinMAME doesn't look for, renamed to the one it does; no zip of that name
    /// is there.
    Rename { to: PathBuf },
    /// A copy of a set that's there under its own name, holding the same files: the copy removed.
    RemoveCopy { of: PathBuf },
    /// A zip that holds no set, or a set no table plays: removed, on the user's word.
    Remove,
}

/// One file of a set as the ROMs page lists it: PinMAME's expected file beside what the zip holds,
/// or a file in the zip the set doesn't name.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub name: String,
    pub size: u64,
    /// Its checksum, as PinMAME lists it (eight hex digits), else the zip's.
    pub crc: String,
    /// `good`, `from its parent` (taken from the parent's zip beside it), `in its parent` (its
    /// parent's, whose zip isn't there), `missing`, `wrong` (there by name, another file),
    /// `optional` (not there, PinMAME plays without it), `not needed` (in the zip, not the set's).
    pub status: &'static str,
}

/// Each file of the set a zip holds — its own set, else the one its files are where PinMAME knows
/// no set of its name — checked as PinMAME loads it, its parent's zip beside it counted; and the
/// files in it the set doesn't name. A zip of no set PinMAME knows lists its files as not needed.
pub fn files_of(zip: &Path, all: &[Set]) -> Result<Vec<FileRow>, String> {
    let found = std::fs::File::open(zip).map_err(|e| format!("{}: {e}", zip.display())).and_then(roms::zip_files)?;
    let name = zip.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let set = all.iter().find(|s| s.set.eq_ignore_ascii_case(&name)).or_else(|| identify(&found, all).and_then(|h| all.iter().find(|s| s.set.eq_ignore_ascii_case(&h))));
    let hex = |crc: u32| format!("{crc:08x}");
    let Some(set) = set else {
        return Ok(found.iter().map(|f| FileRow { name: f.name.clone(), size: f.size, crc: hex(f.crc), status: "not needed" }).collect());
    };
    let parent: Vec<roms::Found> = set
        .parent()
        .map(|p| zip.with_file_name(format!("{}.zip", p.to_lowercase())))
        .filter(|p| p.is_file())
        .and_then(|p| std::fs::File::open(p).ok())
        .and_then(|f| roms::zip_files(f).ok())
        .unwrap_or_default();
    let matches = |f: &roms::Found, e: &roms::Expected| u32::from_str_radix(&e.crc, 16).ok().filter(|c| *c != 0).map_or(f.name.eq_ignore_ascii_case(&e.name), |c| f.crc == c) && f.size == e.size;
    let mut used = vec![false; found.len()];
    let mut out = Vec::new();
    for e in set.files.iter().filter(|e| !e.no_dump) {
        let here = found.iter().position(|f| matches(f, e));
        let status = match here {
            Some(i) => {
                used[i] = true;
                "good"
            }
            None if parent.iter().any(|f| matches(f, e)) => "from its parent",
            None => match found.iter().position(|f| f.name.eq_ignore_ascii_case(&e.name)) {
                Some(i) => {
                    used[i] = true;
                    "wrong"
                }
                None if e.optional => "optional",
                // the parent's, whose zip isn't beside it
                None if e.in_parent && set.parent().is_some() && parent.is_empty() => "in its parent",
                None => "missing",
            },
        };
        out.push(FileRow { name: e.name.clone(), size: e.size, crc: e.crc.to_lowercase(), status });
    }
    out.extend(found.iter().zip(&used).filter(|(_, u)| !**u).map(|(f, _)| FileRow { name: f.name.clone(), size: f.size, crc: hex(f.crc), status: "not needed" }));
    Ok(out)
}

/// The files of a zip, as a set of checksums and sizes, for telling two zips the same.
fn contents(zip: &Path) -> Option<BTreeSet<(u32, u64)>> {
    let files = roms::zip_files(std::fs::File::open(zip).ok()?).ok()?;
    Some(files.into_iter().map(|f| (f.crc, f.size)).collect())
}

/// The fixes that fit a set as the list found it.
pub fn fixes(e: &Entry) -> Vec<Fix> {
    let Some(zip) = &e.zip else { return Vec::new() };
    let mut out = Vec::new();
    if let (Some(set), "misnamed") = (&e.holds, e.status) {
        let to = zip.with_file_name(format!("{set}.zip"));
        if !to.exists() {
            out.push(Fix::Rename { to });
        } else if contents(&to).is_some_and(|theirs| contents(zip).is_some_and(|ours| ours == theirs)) {
            out.push(Fix::RemoveCopy { of: to });
        }
    }
    if e.unused() && !out.iter().any(|f| matches!(f, Fix::RemoveCopy { .. })) {
        out.push(Fix::Remove);
    }
    out
}

/// A fix carried out: a rename where nothing's there under the new name, else a zip to `recycle`
/// (the Recycle Bin).
pub fn fix(e: &Entry, fix: &Fix, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    let zip = e.zip.as_deref().ok_or("That set isn't installed.")?;
    if !fixes(e).contains(fix) {
        return Err(format!("{} has changed since it was checked: check it again first.", zip.display()));
    }
    match fix {
        Fix::Rename { to } => std::fs::rename(zip, to).map_err(|e| format!("{}: {e}", zip.display())),
        Fix::RemoveCopy { .. } | Fix::Remove => recycle(zip),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roms::Expected;

    #[test]
    fn fixes_offered_where_they_fit_and_carried_out() {
        let root = std::env::temp_dir().join(format!("vpi-rom-fixes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let roms = root.join("roms");
        let entry = |zip: &Path, status: &'static str, holds: Option<&str>, used: bool| Entry {
            set: zip.file_stem().unwrap().to_string_lossy().to_lowercase(),
            description: None,
            parent: None,
            zip: Some(zip.to_path_buf()),
            size: 0,
            status,
            look: "warn",
            text: String::new(),
            holds: holds.map(str::to_string),
            used_by: if used { vec![root.join("t.vpx")] } else { Vec::new() },
            parent_of: Vec::new(),
            other_version_of: Vec::new(),
        };
        // a misnamed set with nothing under its own name, no table playing it: renamed, or removed
        let lone = roms.join("Game 1.1.zip");
        zip(&lone, &[("g11.bin", b"cln1")]);
        let e = entry(&lone, "misnamed", Some("game_11"), false);
        assert_eq!(fixes(&e), [Fix::Rename { to: roms.join("game_11.zip") }, Fix::Remove]);
        fix(&e, &fixes(&e)[0], &|_| Ok(())).unwrap();
        assert!(roms.join("game_11.zip").is_file() && !lone.exists());
        // a second download of a set that's there, the same files: the copy removed
        let copy = roms.join("game_11 (1).zip");
        zip(&copy, &[("g11.bin", b"cln1")]);
        let e = entry(&copy, "misnamed", Some("game_11"), false);
        assert_eq!(fixes(&e), [Fix::RemoveCopy { of: roms.join("game_11.zip") }]);
        let gone = std::cell::RefCell::new(Vec::new());
        fix(&e, &fixes(&e)[0], &|p: &Path| {
            gone.borrow_mut().push(p.to_path_buf());
            std::fs::remove_file(p).map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(gone.borrow().as_slice(), [copy.clone()]);
        // one that's different: nothing but removing it, no table playing it
        let other = roms.join("game_11 (2).zip");
        zip(&other, &[("g11.bin", b"diff")]);
        assert_eq!(fixes(&entry(&other, "misnamed", Some("game_11"), false)), [Fix::Remove]);
        // a set a table plays: nothing to offer
        assert!(fixes(&entry(&roms.join("game_11.zip"), "good", None, true)).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    fn file(name: &str, crc: &str, in_parent: bool) -> Expected {
        Expected { name: name.into(), size: 4, crc: crc.into(), in_parent, ..Default::default() }
    }

    fn zip(path: &Path, files: &[(&str, &[u8])]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, bytes) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
            std::io::Write::write_all(&mut w, bytes).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn every_set_there_and_asked_for_told_apart() {
        let root = std::env::temp_dir().join(format!("vpi-rom-inventory-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::under(&root);
        let roms = layout.vpinmame.join("roms");
        let crc = |b: &[u8]| format!("{:08x}", crc32fast::hash(b));
        // a parent and its clone, made up: the clone's sound files its parent's
        let (main, clone_main, sound) = (b"main", b"cln1", b"snd1");
        let all = vec![
            Set { set: "game_10".into(), known: true, clone_of: None, description: Some("Game (1.0)".into()), files: vec![file("g10.bin", &crc(main), false), file("snd.bin", &crc(sound), false)] },
            Set { set: "game_11".into(), known: true, clone_of: Some("game_10".into()), description: Some("Game (1.1)".into()), files: vec![file("g11.bin", &crc(clone_main), false), file("snd.bin", &crc(sound), true)] },
            Set { set: "other".into(), known: true, clone_of: None, description: Some("Other".into()), files: vec![file("o.bin", &crc(b"othr"), false)] },
        ];
        // the parent, a clone beside it, a copy of the clone under a name no set has, and a zip of
        // nobody's files
        zip(&roms.join("game_10.zip"), &[("g10.bin", main), ("snd.bin", sound)]);
        zip(&roms.join("game_11.zip"), &[("g11.bin", clone_main)]);
        zip(&roms.join("Game 1.1 (fixed).zip"), &[("g11.bin", clone_main), ("snd.bin", sound)]);
        zip(&roms.join("junk.zip"), &[("x.bin", b"xxxx")]);
        // a table in its own folder with its own copy of the clone, missing its parent there
        let own = layout.tables.join("Game").join("game.vpx");
        zip(&own.parent().unwrap().join("pinmame").join("roms").join("game_11.zip"), &[("g11.bin", clone_main)]);
        let tables = vec![(layout.tables.join("a.vpx"), "game_11".to_string()), (own.clone(), "game_11".into()), (layout.tables.join("b.vpx"), "other".into())];
        let list = inventory(&layout, &tables, &all);
        // each file of a set, as the page's Files list shows them
        let statuses = |zip: &Path| files_of(zip, &all).unwrap().into_iter().map(|f| (f.name, f.status)).collect::<Vec<_>>();
        let s = |n: &str, st: &'static str| (n.to_string(), st);
        let files = (
            statuses(&roms.join("game_11.zip")),
            statuses(&own.parent().unwrap().join("pinmame").join("roms").join("game_11.zip")),
            statuses(&roms.join("Game 1.1 (fixed).zip")),
            statuses(&roms.join("junk.zip")),
        );
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(files.0, [s("g11.bin", "good"), s("snd.bin", "from its parent")]);
        assert_eq!(files.1, [s("g11.bin", "good"), s("snd.bin", "in its parent")]);
        assert_eq!(files.2, [s("g11.bin", "good"), s("snd.bin", "good")]);
        assert_eq!(files.3, [s("x.bin", "not needed")]);
        let row = |set: &str, own_folder: bool| list.iter().find(|e| e.set == set && e.zip.as_ref().is_some_and(|z| z.to_string_lossy().contains("pinmame")) == own_folder).unwrap();
        assert_eq!((row("game_10", false).status, row("game_10", false).unused()), ("good", false));
        assert_eq!(row("game_10", false).parent_of, [layout.tables.join("a.vpx")]);
        assert_eq!((row("game_11", false).status, row("game_11", false).used_by.len()), ("good", 1));
        assert_eq!(row("game_11", true).status, "needs its parent");
        assert_eq!(row("game 1.1 (fixed)", false).holds.as_deref(), Some("game_11"));
        assert_eq!(row("game 1.1 (fixed)", false).status, "misnamed");
        assert_eq!(row("junk", false).status, "unknown");
        assert!(row("junk", false).unused());
        // the misnamed copy plays for no table, its game's tables named
        assert_eq!(row("game 1.1 (fixed)", false).other_version_of.len(), 2);
        let missing = list.iter().find(|e| e.set == "other").unwrap();
        assert_eq!((missing.status, missing.zip.is_none()), ("not installed", true));
    }
}
