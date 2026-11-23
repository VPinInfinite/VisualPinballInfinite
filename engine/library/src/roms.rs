//! A ROM set checked file by file before it's installed: the files PinMAME expects for the set —
//! asked of our PinMAME build (`vpi-pinmame --rom-files`, its `PinmameGetGameRoms`, patch 3) —
//! against the files the drop holds, each found by its CRC whatever its name, as PinMAME finds a
//! ROM in a zip, else by its name. A clone's files its parent set holds may come from the parent's
//! zip, as PinMAME looks there next. Ours.

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One file a set needs, as PinMAME's audit expects it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Expected {
    pub name: String,
    pub size: u64,
    /// Its CRC-32, in hex.
    pub crc: String,
    pub optional: bool,
    pub no_dump: bool,
    pub bad_dump: bool,
    /// Its parent set holds it too.
    pub in_parent: bool,
}

/// A set as PinMAME knows it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Set {
    pub set: String,
    pub known: bool,
    pub clone_of: Option<String>,
    pub description: Option<String>,
    pub files: Vec<Expected>,
}

impl Set {
    pub fn parent(&self) -> Option<&str> {
        self.clone_of.as_deref().filter(|p| !p.is_empty())
    }
}

/// One file the drop holds: its name (without a folder), size and CRC-32.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    pub size: u64,
    pub crc: u32,
}

/// What a set's check found.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub set: String,
    pub known: bool,
    pub description: Option<String>,
    /// The files it needs: all but the optional and those never dumped.
    pub needed: usize,
    /// Those found, matching.
    pub good: usize,
    /// Needed and not there.
    pub missing: Vec<String>,
    /// There by name, but not the file PinMAME expects (another version, or damaged).
    pub wrong: Vec<String>,
    /// Needed, not there, and held by the parent set: PinMAME takes them from its zip.
    pub from_parent: Vec<String>,
    pub parent: Option<String>,
}

impl Check {
    /// Everything the set needs is there, the parent's files aside.
    pub fn complete(&self) -> bool {
        self.known && self.missing.is_empty() && self.wrong.is_empty()
    }
}

/// `found` against what `set` expects.
pub fn check(set: &Set, found: &[Found]) -> Check {
    let mut c = Check { set: set.set.clone(), known: set.known, description: set.description.clone(), parent: set.parent().map(str::to_string), ..Default::default() };
    for e in &set.files {
        if e.no_dump {
            continue;
        }
        let crc = u32::from_str_radix(&e.crc, 16).ok().filter(|c| *c != 0);
        let by_crc = crc.is_some_and(|crc| found.iter().any(|f| f.crc == crc && f.size == e.size));
        let by_name = found.iter().find(|f| f.name.eq_ignore_ascii_case(&e.name));
        if !e.optional {
            c.needed += 1;
        }
        if by_crc || (crc.is_none() && by_name.is_some_and(|f| f.size == e.size)) {
            c.good += usize::from(!e.optional);
        } else if by_name.is_some() {
            c.wrong.push(e.name.clone());
        } else if e.optional {
        } else if e.in_parent && c.parent.is_some() {
            c.from_parent.push(e.name.clone());
        } else {
            c.missing.push(e.name.clone());
        }
    }
    c
}

/// The files of a zip, from its central directory — names, sizes and CRCs, nothing unpacked.
pub fn zip_files(reader: impl Read + Seek) -> Result<Vec<Found>, String> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().rsplit(['/', '\\']).next().unwrap_or(f.name()).to_string();
        out.push(Found { name, size: f.size(), crc: f.crc32() });
    }
    Ok(out)
}

/// The files of a zip held in memory.
pub fn zip_files_in(bytes: Vec<u8>) -> Result<Vec<Found>, String> {
    zip_files(Cursor::new(bytes))
}

/// A file's CRC-32, from its bytes.
pub fn found(name: &str, bytes: &[u8]) -> Found {
    Found { name: name.rsplit(['/', '\\']).next().unwrap_or(name).to_string(), size: bytes.len() as u64, crc: crc32fast::hash(bytes) }
}

/// Where `vpi-pinmame` is: beside this program, else the repository's build (developing).
fn host() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join("vpi-pinmame.exe");
    if beside.exists() {
        return Some(beside);
    }
    dir.ancestors().take(4).map(|up| up.join(r"target\release\vpi-pinmame.exe")).find(|p| p.exists())
}

static SETS: Mutex<Option<HashMap<String, Set>>> = Mutex::new(None);

/// The set as PinMAME knows it, asked of `vpi-pinmame` once per set.
pub fn set(name: &str) -> Result<Set, String> {
    let key = name.to_lowercase();
    if let Some(s) = SETS.lock().unwrap().as_ref().and_then(|m| m.get(&key)) {
        return Ok(s.clone());
    }
    let host = host().ok_or("vpi-pinmame isn't beside the program, so ROMs can't be checked")?;
    let mut cmd = std::process::Command::new(&host);
    cmd.args(["--rom-files", name]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window, and below the user's own programs (the library checks every table's
        // set in the background)
        cmd.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    let out = cmd.output().map_err(|e| format!("{}: {e}", host.display()))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let s: Set = serde_json::from_slice(&out.stdout).map_err(|e| format!("vpi-pinmame's list of {name}: {e}"))?;
    SETS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, s.clone());
    Ok(s)
}

static ALL_FILES: Mutex<Option<Vec<Set>>> = Mutex::new(None);

/// Every set PinMAME knows with its files, asked of `vpi-pinmame` once (`--all-rom-files`): for
/// telling which set a zip holds by its files.
pub fn all_rom_files() -> Result<Vec<Set>, String> {
    if let Some(all) = ALL_FILES.lock().unwrap().as_ref() {
        return Ok(all.clone());
    }
    let host = host().ok_or("vpi-pinmame isn't beside the program, so ROMs can't be checked")?;
    let mut cmd = std::process::Command::new(&host);
    cmd.arg("--all-rom-files");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window, and below the user's own programs
        cmd.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    let out = cmd.output().map_err(|e| format!("{}: {e}", host.display()))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let all: Vec<Set> = serde_json::from_slice(&out.stdout).map_err(|e| format!("vpi-pinmame's list of every set's files: {e}"))?;
    *ALL_FILES.lock().unwrap() = Some(all.clone());
    Ok(all)
}

/// One of the games PinMAME knows, as `vpi-pinmame --sets` lists them.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSet {
    pub set: String,
    pub clone_of: Option<String>,
    pub description: Option<String>,
}

static ALL_SETS: Mutex<Option<Vec<KnownSet>>> = Mutex::new(None);

/// Every game PinMAME knows, asked of `vpi-pinmame` once.
pub fn all_sets() -> Result<Vec<KnownSet>, String> {
    if let Some(all) = ALL_SETS.lock().unwrap().as_ref() {
        return Ok(all.clone());
    }
    let host = host().ok_or("vpi-pinmame isn't beside the program, so PinMAME's games can't be listed")?;
    let mut cmd = std::process::Command::new(&host);
    cmd.arg("--sets");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window, and below the user's own programs
        cmd.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    let out = cmd.output().map_err(|e| format!("{}: {e}", host.display()))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let all: Vec<KnownSet> = serde_json::from_slice(&out.stdout).map_err(|e| format!("vpi-pinmame's list of games: {e}"))?;
    *ALL_SETS.lock().unwrap() = Some(all.clone());
    Ok(all)
}

/// The sets of `set`'s game PinMAME knows: its parent and the parent's clones, or its own clones.
pub fn same_game(set: &str) -> Vec<KnownSet> {
    let Ok(all) = all_sets() else { return Vec::new() };
    let low = set.to_lowercase();
    let root = all.iter().find(|k| k.set.eq_ignore_ascii_case(&low)).and_then(|k| k.clone_of.clone()).filter(|p| !p.is_empty()).unwrap_or(low).to_lowercase();
    all.into_iter().filter(|k| k.set.eq_ignore_ascii_case(&root) || k.clone_of.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&root))).collect()
}

/// Whether a set's zip is in the ROM folder.
pub fn installed(roms: &Path, set: &str) -> bool {
    roms.join(format!("{set}.zip")).is_file()
}

/// An installed set checked file by file against what PinMAME expects, for a table's health
/// check: its zip in `roms`, with its parent set's beside it for the files a clone takes from
/// there, as PinMAME looks for them; the verdict in words, and how it looks.
pub fn check_installed(roms: &Path, set_name: &str) -> Result<(&'static str, String), String> {
    installed_check(roms, set_name).map(|c| (c.look, c.text))
}

/// What an installed set's check found.
pub struct Installed {
    pub look: &'static str,
    pub text: String,
    /// The set a clone takes files from: its zip, there or not, counts in the verdict too.
    pub parent: Option<String>,
}

/// `check_installed`, with the parent set the verdict also depends on.
pub fn installed_check(roms: &Path, set_name: &str) -> Result<Installed, String> {
    let zip = |name: &str| -> Result<Vec<Found>, String> {
        let path = roms.join(format!("{name}.zip"));
        zip_files(std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?)
    };
    let s = set(set_name)?;
    let mut found = zip(set_name)?;
    let parent_there = s.parent().is_some_and(|p| installed(roms, p));
    if let Some(parent) = s.parent().filter(|_| parent_there) {
        found.extend(zip(parent)?);
    }
    let (look, text) = verdict(&check(&s, &found), parent_there, Of::Table);
    Ok(Installed { look, text, parent: s.parent().map(str::to_lowercase) })
}

/// Whose ROM a check's words speak of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Of {
    /// A table's, installed.
    Table,
    /// A set in a drop, not added yet.
    Drop,
}

/// What a check says, in words, and how it looks: good, warn or bad. `parent_there`: the parent
/// set's zip is installed or in the drop.
pub fn verdict(c: &Check, parent_there: bool, of: Of) -> (&'static str, String) {
    let set = &c.set;
    if !c.known {
        return ("warn", match of {
            Of::Table => format!("PinMAME doesn't know a ROM called {set}. The table's script may name it wrongly: try Change the ROM on the table's page."),
            Of::Drop => format!("PinMAME doesn't know a ROM called {set}, so the player wouldn't find it under that name."),
        });
    }
    if !c.missing.is_empty() {
        // a missing file stops the game; most often the zip holds another version of the game
        let wrong = match c.wrong.len() {
            0 => String::new(),
            1 => ", and 1 more isn't the one PinMAME expects".to_string(),
            n => format!(", and {n} more aren't the ones PinMAME expects"),
        };
        let (m, n) = (c.missing.len(), c.needed);
        return ("bad", match of {
            Of::Table => format!("Its ROM ({set}) is missing {m} of its {n} files{wrong}, so the game won't start. The zip you have is probably a different version: get {set} from the table's VPS page."),
            Of::Drop => format!("This zip is missing {m} of the {n} files {set} needs{wrong}, so the game won't start. It's probably a different version: get {set} from the table's VPS page."),
        });
    }
    if !c.wrong.is_empty() {
        // the PinMAME library plays what it's given, so a file that isn't the one expected still runs
        let (w, are) = match c.wrong.len() {
            1 => ("1 file".to_string(), "isn't the one"),
            n => (format!("{n} files"), "aren't the ones"),
        };
        return ("bad", match of {
            Of::Table => format!("{w} in its ROM ({set}) {are} PinMAME expects. It may still play, but might not look or sound right. A fresh copy of the ROM fixes it."),
            Of::Drop => format!("{w} in this zip {are} PinMAME expects for {set}. It may still play, but might not look or sound right."),
        });
    }
    match (&c.parent, c.from_parent.len()) {
        (Some(parent), n) if n > 0 && parent_there => ("good", format!("Its {} files match {set}, {n} of them from {parent}.zip, the version it's based on", c.needed)),
        // PinMAME takes those files from the parent's zip: without it the game won't start
        (Some(parent), n) if n > 0 => ("bad", match of {
            Of::Table => format!("Its ROM ({set}) also needs {parent}.zip, the version it's based on, which isn't installed. The game won't start without it."),
            Of::Drop => format!("{set} also needs {parent}.zip, the version it's based on, which isn't installed or in this drop. The game won't start without it."),
        }),
        _ if c.needed == 1 => ("good", format!("Its one file matches {set}")),
        _ => ("good", format!("All {} files match {set}", c.needed)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn afm() -> Set {
        let e = |name: &str, crc: &str, in_parent| Expected { name: name.into(), size: 1 << 20, crc: crc.into(), in_parent, ..Default::default() };
        Set {
            set: "afm_113b".into(),
            known: true,
            clone_of: Some("afm_113".into()),
            description: None,
            files: vec![e("afm_113b.bin", "34fd2d7d", false), e("afm_s2.l1", "6e39d96e", true), e("afm_s3.l1", "1cbce9b1", true), e("afm_s4.l1", "5ff7fbb7", true)],
        }
    }

    #[test]
    fn files_found_by_their_crc_whatever_their_names() {
        let f = |name: &str, crc: u32| Found { name: name.into(), size: 1 << 20, crc };
        // all four, one under another name
        let all = [f("AFM_113B.BIN", 0x34fd2d7d), f("s2.rom", 0x6e39d96e), f("afm_s3.l1", 0x1cbce9b1), f("afm_s4.l1", 0x5ff7fbb7)];
        let c = check(&afm(), &all);
        assert!(c.complete() && c.from_parent.is_empty());
        assert_eq!((c.good, c.needed), (4, 4));
        assert_eq!(verdict(&c, false, Of::Table), ("good", "All 4 files match afm_113b".into()));
        // its own file alone: the sound ROMs from its parent
        let c = check(&afm(), &all[..1]);
        assert!(c.complete());
        assert_eq!(c.from_parent.len(), 3);
        assert_eq!(verdict(&c, false, Of::Table), ("bad", "Its ROM (afm_113b) also needs afm_113.zip, the version it's based on, which isn't installed. The game won't start without it.".into()));
        assert_eq!(verdict(&c, true, Of::Table).0, "good");
        // its own file another version: wrong
        let c = check(&afm(), &[f("afm_113b.bin", 0x12345678)]);
        assert_eq!(c.wrong, ["afm_113b.bin".to_string()]);
        assert_eq!(verdict(&c, true, Of::Table), ("bad", "1 file in its ROM (afm_113b) isn't the one PinMAME expects. It may still play, but might not look or sound right. A fresh copy of the ROM fixes it.".into()));
    }

    #[test]
    fn what_a_check_says_in_words() {
        let f = |name: &str, crc: u32| Found { name: name.into(), size: 1 << 20, crc };
        // a set with no parent: whatever isn't there is missing
        let ss = Set {
            set: "ss_15".into(),
            known: true,
            files: (1..=4).map(|i| Expected { name: format!("ss_{i}.bin"), size: 1 << 20, crc: format!("{i:08x}"), ..Default::default() }).collect(),
            ..Default::default()
        };
        // another version's files: none of the four there
        let c = check(&ss, &[f("ss_g14.bin", 11), f("ss_s2.bin", 12), f("ss_s3.bin", 13), f("ss_s4.bin", 14)]);
        assert_eq!(verdict(&c, false, Of::Table), ("bad", "Its ROM (ss_15) is missing 4 of its 4 files, so the game won't start. The zip you have is probably a different version: get ss_15 from the table's VPS page.".into()));
        assert_eq!(verdict(&c, false, Of::Drop).1, "This zip is missing 4 of the 4 files ss_15 needs, so the game won't start. It's probably a different version: get ss_15 from the table's VPS page.");
        // two wrong, two missing
        let c = check(&ss, &[f("ss_1.bin", 11), f("ss_2.bin", 12)]);
        assert_eq!(verdict(&c, false, Of::Table).1, "Its ROM (ss_15) is missing 2 of its 4 files, and 2 more aren't the ones PinMAME expects, so the game won't start. The zip you have is probably a different version: get ss_15 from the table's VPS page.");
        // two of another version, the rest right
        let c = check(&ss, &[f("ss_1.bin", 1), f("ss_2.bin", 2), f("ss_3.bin", 13), f("ss_4.bin", 14)]);
        assert_eq!(verdict(&c, false, Of::Table).1, "2 files in its ROM (ss_15) aren't the ones PinMAME expects. It may still play, but might not look or sound right. A fresh copy of the ROM fixes it.");
        let all = [f("afm_113b.bin", 0x34fd2d7d)];
        // a set PinMAME doesn't know
        let unknown = Set { set: "afm_999".into(), ..Default::default() };
        assert_eq!(verdict(&check(&unknown, &[]), true, Of::Table), ("warn", "PinMAME doesn't know a ROM called afm_999. The table's script may name it wrongly: try Change the ROM on the table's page.".into()));
        // its parent's files from the parent's zip
        assert_eq!(verdict(&check(&afm(), &all[..1]), true, Of::Table).1, "Its 4 files match afm_113b, 3 of them from afm_113.zip, the version it's based on");
    }

    #[test]
    fn a_zips_files_from_its_index() {
        let mut bytes = Vec::new();
        {
            let mut z = zip::ZipWriter::new(Cursor::new(&mut bytes));
            z.start_file("roms/a.bin", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
            std::io::Write::write_all(&mut z, b"hello").unwrap();
            z.finish().unwrap();
        }
        assert_eq!(zip_files_in(bytes).unwrap(), [found("a.bin", b"hello")]);
    }
}
