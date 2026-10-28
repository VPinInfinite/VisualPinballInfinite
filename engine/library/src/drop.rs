//! A drop as the app shows it: each download in it — a file, an archive or a
//! folder, or a folder of many downloads sorted into them — with the table it brings, matched to
//! VPS, or the library's table its files are for; each file's place, a pack's many files one row,
//! and what's there already. Nothing is copied here: `install.rs` copies, as the user chooses.
//! Ours.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::analysis::{Contents, Kind, analyse};
use crate::archive::{self, Listing, Source};
use crate::matcher::{self, Clues};
use crate::plan::{Layout, Step, There, common_folder, plan, there};
use crate::vps::Database;

/// A table in the library (or brought by another download of the same drop), as a drop's files are
/// tied to one.
#[derive(Clone, Debug, Default)]
pub struct Known {
    /// Its file's stem: what its backglass, views and settings are named for.
    pub stem: String,
    /// The game name its script gives (its ROM).
    pub game_name: Option<String>,
    /// The names its script may hand PinUP for its pack, the likeliest first.
    pub pup_names: Vec<String>,
    /// Its VPS game.
    pub vps_game: Option<String>,
    /// The script files its script loads.
    pub helpers: Vec<String>,
    /// The music its script plays, as it names it (`BLIZZARD/01 - I Don_t Wanna Stop.mp3`).
    pub music: Vec<String>,
    /// Its table file, where it's on disk (not one inside an archive).
    pub file: Option<PathBuf>,
}

/// Where a drop's downloads that bring no table of their own go (files dropped onto the window
/// after the first had gone with no table, only the first file added being tied to it, and the
/// table being added wasn't one to tie them to).
#[derive(Clone, Copy, Debug)]
pub enum Tie<'a> {
    /// Dropped on a table, its tile or its page: all of them for it.
    On(&'a Known),
    /// Dropped onto the drop window opened for a table: tied to a table being added by what they
    /// are, else to that table, as the first ones.
    Joining(&'a Known),
    /// Tied by what they are to a table being added or one in the library; else, with only one
    /// table being added and nothing saying otherwise, to that one.
    Found,
}

/// A table a download brings, matched to VPS.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableFound {
    /// Its file's name.
    pub file: String,
    pub stem: String,
    pub game_id: Option<String>,
    pub game: Option<String>,
    pub maker: Option<String>,
    pub year: Option<i32>,
    pub file_id: Option<String>,
    pub version: Option<String>,
    pub authors: Vec<String>,
    /// Found surely; else a guess the user confirms.
    pub sure: bool,
    /// How it was found, in words.
    pub how: String,
    /// The game name its script gives (its ROM), and whether its script drives PinMAME.
    pub game_name: Option<String>,
    pub pinmame: bool,
    /// The names its script may hand PinUP for its pack.
    pub pup_names: Vec<String>,
    /// The script files its script loads.
    pub helpers: Vec<String>,
    /// What its own file says of it — its table info's name, authors and version — for naming it
    /// where VPS's match is only a guess.
    pub own_name: Option<String>,
    pub own_authors: Option<String>,
    pub own_version: Option<String>,
    /// The music its script plays, as it names it.
    #[serde(skip)]
    pub music: Vec<String>,
    /// Its file, where it's on disk.
    #[serde(skip)]
    pub on_disk: Option<PathBuf>,
}

impl From<&TableFound> for Known {
    fn from(t: &TableFound) -> Known {
        Known {
            stem: t.stem.clone(),
            game_name: t.game_name.clone(),
            pup_names: t.pup_names.clone(),
            vps_game: t.game_id.clone(),
            helpers: t.helpers.clone(),
            music: t.music.clone(),
            file: t.on_disk.clone(),
        }
    }
}

/// One row of a download: a file, or a pack's many files.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// What it is, for the screens (`table`, `backglass`, `rom`, `pupPack`, …).
    pub kind: String,
    /// The same in words.
    pub label: String,
    /// The file's name, or the pack's.
    pub name: String,
    pub files: usize,
    pub size: u64,
    /// Where it goes: the file, or the pack's folder.
    pub to: Option<PathBuf>,
    /// The folder's kind, as the settings name them (`Tables`, `ROMs`, …).
    pub folder: Option<String>,
    pub note: Option<String>,
    /// `new`, `same` (there already, the same size), `some` (some of a pack's files there
    /// already), `replace` (a different file there), or `skip` (not copied: `note` says why).
    pub state: String,
    /// Ticked to begin with.
    pub include: bool,
    /// Its files, by their index in the download's steps.
    #[serde(skip)]
    pub steps: Vec<usize>,
    /// For files nothing placed, grouped by the folder they're in: that folder in the download, the
    /// files' paths kept below it wherever the user puts them.
    #[serde(skip)]
    pub under: Option<String>,
    /// What checking its files found (a ROM set's against what PinMAME expects).
    pub check: Option<RowCheck>,
}

/// What checking a row's files found: how it looks — `good`, `warn` or `bad` — and in words.
#[derive(Clone, Debug, Serialize)]
pub struct RowCheck {
    pub look: &'static str,
    pub text: String,
}

/// One download of a drop.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    /// The dropped item it came from.
    pub source: PathBuf,
    pub name: String,
    /// What it is, in words: "a zip archive, 5 files".
    pub what: String,
    /// The table it brings; the first, when it brings several.
    pub table: Option<TableFound>,
    /// The library's table its files are for, when it brings none of its own, by its file's stem.
    pub for_table: Option<String>,
    /// How that table was found, or why none was.
    pub for_note: Option<String>,
    /// The machine (ROM) it's for.
    pub rom: Option<String>,
    /// The name its table's PinUP pack goes under.
    pub pup: Option<String>,
    /// The table's file stem its own files are named for.
    #[serde(skip)]
    pub stem: Option<String>,
    /// The name its media go under in PinballX's or PinballY's folders, where a drop's media go in
    /// one of those (`frontends::media_target`).
    #[serde(skip)]
    pub media_name: Option<String>,
    /// What else to know about it.
    pub notes: Vec<String>,
    pub rows: Vec<Row>,
    /// Its read-me's first lines.
    pub readme: Option<String>,
    /// Why it couldn't be read.
    pub error: Option<String>,
    #[serde(skip)]
    pub listing: Option<Listing>,
    #[serde(skip)]
    pub steps: Vec<Step>,
}

/// The kinds of file that are a table's own, named for it.
const COMPANIONS: [Kind; 6] = [Kind::Backglass, Kind::Pov, Kind::Physics, Kind::TableSettings, Kind::Script, Kind::BackglassPlaces];

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn stem(path: &str) -> &str {
    let n = file_name(path);
    n.rsplit_once('.').map_or(n, |(s, _)| s)
}

/// A RAR archive's later part (`name.part2.rar`, `name.r00`), which its first part brings.
fn later_part(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if let Some(rest) = name.strip_suffix(".rar")
        && let Some(at) = rest.rfind(".part")
    {
        return rest[at + 5..].parse::<u32>().is_ok_and(|n| n > 1);
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    ext.len() == 3 && ext.starts_with('r') && ext[1..].chars().all(|c| c.is_ascii_digit())
}

fn tables_under(dir: &Path, depth: usize) -> usize {
    let Ok(items) = std::fs::read_dir(dir) else { return 0 };
    let mut n = 0;
    for item in items.flatten() {
        let p = item.path();
        if p.is_dir() {
            if depth < 6 {
                n += tables_under(&p, depth + 1);
            }
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx")) {
            n += 1;
        }
    }
    n
}

/// The downloads a dropped path holds, each named, with its listing: the path itself, unless it's
/// a folder of downloads — archives at its top (a ROM set's zip beside its one table aside), or
/// tables in more than one place — when each archive is one, each folder one (sorted out the same
/// way, a PinUP pack's own folder always one), and its loose files one per table with the files
/// named for it, the rest each one of its own.
pub fn downloads_in(path: &Path) -> Vec<(String, PathBuf, Result<Listing, String>)> {
    let named = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
    if !path.is_dir() {
        return vec![(named(path), path.to_path_buf(), archive::list(path))];
    }
    let mut children: Vec<PathBuf> = std::fs::read_dir(path).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    children.sort();
    // a PinUP pack's own folder is one download: the zips in it are its layouts
    if children.iter().any(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("screens.pup"))) {
        return vec![(named(path), path.to_path_buf(), archive::list(path))];
    }
    let archives: Vec<&PathBuf> = children.iter().filter(|p| p.is_file() && archive::format_of(p).is_some() && !later_part(p)).collect();
    let folders: Vec<&PathBuf> = children.iter().filter(|p| p.is_dir()).collect();
    let loose: Vec<&PathBuf> = children.iter().filter(|p| p.is_file() && archive::format_of(p).is_none()).collect();
    let loose_tables = loose.iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx"))).count();
    let tables_in_folders: Vec<usize> = folders.iter().map(|f| tables_under(f, 0)).collect();
    let places_with_tables = loose_tables.min(1) + tables_in_folders.iter().filter(|n| **n > 0).count();
    // a zip named as PinMAME names a ROM set, beside a table, is that table's ROM
    let rom_like = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) && crate::analysis::looks_like_rom_set(&named(p));
    let other_archives = archives.iter().filter(|p| !rom_like(p)).count();
    let tables_here = loose_tables + tables_in_folders.iter().sum::<usize>();
    let many = other_archives > 0 || places_with_tables > 1 || loose_tables > 1 || tables_in_folders.iter().any(|n| *n > 1) || (!archives.is_empty() && tables_here == 0);
    if !many {
        return vec![(named(path), path.to_path_buf(), archive::list(path))];
    }
    let mut out = Vec::new();
    for a in archives {
        out.push((named(a), a.clone(), archive::list(a)));
    }
    for f in folders {
        out.extend(downloads_in(f));
    }
    // loose files: each table with what's named for it; the rest each by itself
    let mut taken = vec![false; loose.len()];
    let stems: Vec<String> = loose.iter().map(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()).collect();
    for (i, p) in loose.iter().enumerate() {
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx")) {
            continue;
        }
        let mut entries = Vec::new();
        for (j, q) in loose.iter().enumerate() {
            if !taken[j] && (j == i || stems[j] == stems[i]) {
                taken[j] = true;
                let meta = std::fs::metadata(q).ok();
                let modified = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
                entries.push(archive::Entry { path: named(q), size: meta.map_or(0, |m| m.len()), modified });
            }
        }
        out.push((named(p), (*p).clone(), Ok(Listing { source: Source::Folder(path.to_path_buf()), entries })));
    }
    for (j, q) in loose.iter().enumerate() {
        if !taken[j] {
            out.push((named(q), (*q).clone(), archive::list(q)));
        }
    }
    out
}

/// The screens' name for a kind of file.
pub fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Table => "table",
        Kind::OldTable => "oldTable",
        Kind::FuturePinball => "futurePinball",
        Kind::Backglass => "backglass",
        Kind::BackglassPlaces => "backglassPlaces",
        Kind::Pov => "pov",
        Kind::Physics => "physics",
        Kind::TableSettings => "tableSettings",
        Kind::Script => "script",
        Kind::HelperScript => "helperScript",
        Kind::Patch => "patch",
        Kind::Rom => "rom",
        Kind::PupPack => "pupPack",
        Kind::AltSound => "altSound",
        Kind::Colouring(_) => "coloring",
        Kind::DmdFolder => "dmdFolder",
        Kind::Music => "music",
        Kind::Nvram => "nvram",
        Kind::PinmameSettings => "pinmameSettings",
        Kind::Media => "media",
        Kind::TablePicture => "tablePicture",
        Kind::Manual => "manual",
        Kind::Readme => "readme",
        Kind::InnerArchive => "innerArchive",
        Kind::Other => "other",
    }
}

/// The folder a kind of file goes into, as the settings' Folders page names them.
fn folder_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Rom => "ROMs",
        Kind::PupPack => "PinUP packs",
        Kind::AltSound => "AltSound packs",
        Kind::Colouring(_) => "Color files",
        Kind::Nvram | Kind::PinmameSettings => "Machines' memory",
        Kind::HelperScript => "Scripts",
        Kind::Music => "Music",
        Kind::Media => "PinUP Popper's media",
        _ => "Tables",
    }
}

fn capitalised(text: &str) -> String {
    let mut c = text.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// A download's rows: each file its own, a pack's files one.
fn rows_of(steps: &[Step]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let pack = |k: Kind| matches!(k, Kind::PupPack | Kind::AltSound | Kind::DmdFolder | Kind::Music);
    for (i, s) in steps.iter().enumerate() {
        // what nothing placed at the download's top, a picture or video say: one row each, for the
        // user to place (PinUP Popper's media folders among the places)
        if s.to.is_none() && matches!(s.kind, Kind::Other | Kind::Media) && !s.from.contains('/') {
            rows.push(Row {
                kind: "folder".into(),
                label: "Not sorted".into(),
                name: s.from.clone(),
                files: 1,
                size: s.size,
                to: None,
                folder: None,
                note: Some("where it goes is for you to choose".into()),
                state: String::new(),
                include: false,
                steps: vec![i],
                under: Some(String::new()),
                check: None,
            });
            continue;
        }
        // what nothing placed, in a folder of the download: one row a folder
        if s.to.is_none()
            && matches!(s.kind, Kind::Other | Kind::Media)
            && let Some((top, _)) = s.from.split_once('/')
        {
            if let Some(row) = rows.iter_mut().find(|r| r.under.as_deref() == Some(top)) {
                row.files += 1;
                row.size += s.size;
                row.steps.push(i);
            } else {
                rows.push(Row {
                    kind: "folder".into(),
                    label: "Not sorted".into(),
                    name: top.to_string(),
                    files: 1,
                    size: s.size,
                    to: None,
                    folder: None,
                    note: Some("where these go is for you to choose".into()),
                    state: String::new(),
                    include: false,
                    steps: vec![i],
                    under: Some(top.to_string()),
                    check: None,
                });
            }
            continue;
        }
        if pack(s.kind)
            && let Some(row) = rows.iter_mut().find(|r| r.kind == kind_key(s.kind))
        {
            row.files += 1;
            row.size += s.size;
            row.steps.push(i);
            continue;
        }
        rows.push(Row {
            kind: kind_key(s.kind).into(),
            label: capitalised(s.kind.describe()),
            name: file_name(&s.from).to_string(),
            files: 1,
            size: s.size,
            to: s.to.clone(),
            folder: s.to.as_ref().map(|_| folder_name(s.kind).to_string()),
            note: s.note.clone(),
            state: String::new(),
            include: false,
            steps: vec![i],
            under: None,
            check: None,
        });
    }
    for row in &mut rows {
        let of: Vec<&Step> = row.steps.iter().map(|&i| &steps[i]).collect();
        if row.under.is_none() && (row.files > 1 || pack(of[0].kind)) {
            row.to = common_folder(of.iter().filter_map(|s| s.to.as_deref()));
            row.name = row.to.as_ref().and_then(|t| t.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| row.name.clone());
        }
        let theres: Vec<There> = of.iter().filter(|s| s.to.is_some()).map(|s| there(s)).collect();
        if row.under.is_some() {
            row.state = "unsorted".into();
            continue;
        }
        row.state = if theres.is_empty() {
            "skip"
        } else if theres.iter().any(|t| matches!(t, There::Different | There::Named)) {
            "replace"
        } else if theres.iter().all(|t| *t == There::SameSize) {
            "same"
        } else if theres.contains(&There::SameSize) {
            "some"
        } else {
            "new"
        }
        .into();
        // a picture for the table changes how it looks: offered, not ticked
        row.include = matches!(row.state.as_str(), "new" | "some" | "replace") && of[0].kind != Kind::TablePicture;
    }
    rows
}

/// What a source is, in words.
fn described(listing: &Listing, files: usize) -> String {
    let n = |k: usize| if k == 1 { "1 file".to_string() } else { format!("{k} files") };
    match &listing.source {
        Source::Folder(_) if listing.entries.len() < 6 && listing.entries.iter().all(|e| !e.path.contains('/')) => n(files),
        Source::Folder(_) => format!("a folder, {}", n(files)),
        Source::Archive(a) if a.parts.len() > 1 => format!("a {} in {} parts, {}", a.format.describe(), a.parts.len(), n(files)),
        Source::Archive(a) => format!("a {}, {}", a.format.describe(), n(files)),
        Source::File(_) => "a file".into(),
    }
}

/// Why no table was found for a download's files.
enum Untied {
    /// What they are says they're another table's: why, in words.
    Elsewhere(String),
    /// Nothing says which.
    Unknown,
}

impl Untied {
    fn said(self) -> String {
        match self {
            Untied::Elsewhere(why) => why,
            Untied::Unknown => "which table it's for: to choose".into(),
        }
    }
}

/// The library's table a download's files are for, when it brings none: the one its backglass or
/// settings are named for, the one whose script gives the ROM it names, the one whose script asks
/// PinUP for its pack, the one of the game VPS finds by its name — with how it was found.
fn table_for<'a>(contents: &Contents, known: &'a [Known], db: &Database) -> Result<(&'a Known, String), Untied> {
    let companion = contents.files.iter().find(|(_, k)| COMPANIONS.contains(k)).map(|(e, _)| stem(&e.path).to_string());
    if let Some(s) = &companion
        && let Some(k) = known.iter().find(|k| k.stem.eq_ignore_ascii_case(s))
    {
        return Ok((k, "named for it".into()));
    }
    // a script a table's script loads (KISS's own `KISSCore.vbs`)
    for (e, k) in &contents.files {
        if *k == Kind::Script
            && let Some(k) = known.iter().find(|k| k.helpers.iter().any(|h| h.eq_ignore_ascii_case(file_name(&e.path))))
        {
            return Ok((k, format!("by a script its table loads, {}", file_name(&e.path))));
        }
    }
    let hints = contents.rom_hints();
    for h in &hints {
        let by_rom: Vec<&Known> = known.iter().filter(|k| k.game_name.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(h))).collect();
        if let [first, rest @ ..] = by_rom.as_slice() {
            let also = if rest.is_empty() { String::new() } else { format!(" (or {} more tables of it)", rest.len()) };
            return Ok((first, format!("by its ROM, {h}{also}")));
        }
        if let Some(k) = known.iter().find(|k| k.pup_names.iter().any(|p| p.eq_ignore_ascii_case(h))) {
            return Ok((k, format!("by the name its script asks PinUP for, {h}")));
        }
    }
    if let Some(s) = &companion
        && let Some((game, d)) = matcher::game_by_file_name(db, s)
        && d <= matcher::CONFIRMING
    {
        if let Some(k) = known.iter().find(|k| k.vps_game.as_deref() == Some(game.id.as_str())) {
            return Ok((k, format!("by its game on VPS, {}", game.name)));
        }
        return Err(Untied::Elsewhere(format!("for {} on VPS, which isn't in your library", game.name)));
    }
    Err(Untied::Unknown)
}

/// A backglass in a download whose name VPS knows for another machine than `game`'s, the table it
/// goes with (one ROM can serve two machines: AC/DC's Premium and its LUCI share theirs): a note
/// to check it before it's added.
fn backglass_of_another(contents: &Contents, game: Option<&str>, db: &Database) -> Option<String> {
    let game = db.game(game?)?;
    let backglass = contents.files.iter().find(|(_, k)| *k == Kind::Backglass).map(|(e, _)| stem(&e.path))?;
    let (other, d) = matcher::game_by_file_name(db, backglass)?;
    let named = |g: &crate::vps::Game| match g.year {
        Some(y) => format!("{} ({} {y})", g.name, g.manufacturer),
        None => g.name.clone(),
    };
    (d <= matcher::CONFIRMING && other.id != game.id).then(|| format!("Its backglass looks like it's for {}, but the table is {}: check it's the one you want", named(other), named(game)))
}

/// The places a file nothing placed can be put, by the screens' name for each, for a download:
/// the table's folder, its music folder, VPX's scripts, its PinUP pack's folder, its machine's
/// AltSound, colour and ROM folders — those the download's table and machine allow.
pub fn places(d: &Download, layout: &Layout) -> Vec<(&'static str, String, PathBuf)> {
    let mut out = vec![("tables", "Beside the table".to_string(), layout.tables.clone()), ("music", "The table's music folder".into(), layout.tables.join("music")), ("scripts", "VPX's scripts".into(), layout.scripts.clone())];
    if let Some(p) = d.pup.as_deref().or(d.rom.as_deref()) {
        out.push(("pupPack", "Its PinUP pack".into(), layout.pup_videos.join(p)));
    }
    if let Some(r) = &d.rom {
        out.push(("altSound", "Its machine's AltSound".into(), layout.vpinmame.join("altsound").join(r)));
        out.push(("colour", "Its machine's color files".into(), layout.vpinmame.join("altcolor").join(r)));
        out.push(("roms", "ROMs".into(), layout.vpinmame.join("roms")));
    }
    // the media folders of the front end a drop's media go in, for media nothing could tell the
    // screen of: its files named for the table as they go in
    if d.stem.is_some() {
        match crate::frontends::media_target(layout) {
            Some(crate::frontends::Target::Popper(root)) => out.extend(crate::plan::MEDIA_FOLDERS.iter().map(|&(k, label, folder)| (k, label.to_string(), root.join(folder)))),
            Some(crate::frontends::Target::Other(fe)) => {
                out.extend(crate::frontends::PLACES.iter().filter_map(|&(k, words, kind, video)| Some((k, format!("{}: {words}", fe.front.name()), fe.place(kind, video)?))));
            }
            None => {}
        }
    }
    out
}

/// A table file read and matched to VPS.
fn found(listing: &Listing, entry: &archive::Entry, db: &Database) -> (TableFound, Option<String>) {
    let read = crate::table::read_in(listing, entry);
    let mut note = None;
    let facts = match read {
        Ok(f) => f,
        Err(e) => {
            note = Some(format!("its table file couldn't be read ({e}): matched by its name alone"));
            crate::table::TableFacts { clues: Clues { file_name: stem(&entry.path).to_string(), ..Default::default() }, ..Default::default() }
        }
    };
    let m = matcher::match_table(db, &facts.clues);
    let t = TableFound {
        file: file_name(&entry.path).to_string(),
        stem: stem(&entry.path).to_string(),
        game_id: m.game.map(|g| g.id.clone()),
        game: m.game.map(|g| g.name.clone()),
        maker: m.game.map(|g| g.manufacturer.clone()).filter(|s| !s.is_empty()),
        year: m.game.and_then(|g| g.year),
        file_id: m.table_file.map(|f| f.id.clone()),
        version: m.table_file.and_then(|f| f.version.clone()),
        authors: m.table_file.map(|f| f.authors.clone()).unwrap_or_default(),
        sure: m.game.is_some() && m.found.sure(),
        how: if m.game.is_some() { m.found.describe().to_string() } else { "not found on VPS".to_string() },
        game_name: facts.game_name.clone(),
        pinmame: facts.pinmame,
        pup_names: facts.pup_names.clone(),
        helpers: facts.helpers.clone(),
        own_name: facts.clues.info_name.clone().filter(|n| !n.trim().is_empty() && !["table1", "visual pinball demo table"].contains(&n.trim().to_lowercase().as_str())),
        own_authors: facts.clues.info_authors.clone().filter(|a| !a.trim().is_empty()),
        own_version: facts.clues.info_version.clone().filter(|v| !v.trim().is_empty()),
        music: facts.music.clone(),
        on_disk: on_disk(listing, entry),
    };
    (t, note)
}

/// A drop's file on disk: dropped as it is, or in a dropped folder; None inside an archive.
fn on_disk(listing: &Listing, entry: &archive::Entry) -> Option<PathBuf> {
    let p = match &listing.source {
        Source::File(f) => f.clone(),
        Source::Folder(dir) => dir.join(&entry.path),
        Source::Archive(_) => return None,
    };
    p.is_file().then_some(p)
}

/// Music going into the `music` folder put where its table's script plays it from (`names`, as the
/// script names them: `BLIZZARD/01 - I Don_t Wanna Stop.mp3`): a file the script names, at that
/// name; one below a folder the script plays from, from that folder on (ours: a media pack's
/// `Music Files/BLIZZARD/…` would have gone where Blizzard of Ozz's script doesn't look).
fn music_as_its_script_plays(steps: &mut [Step], names: &[String], layout: &Layout) {
    let names: Vec<String> = names
        .iter()
        .map(|n| {
            let n = n.replace('\\', "/");
            let n = n.trim_start_matches("./");
            n.strip_prefix("music/").or_else(|| n.strip_prefix("Music/")).unwrap_or(n).to_string()
        })
        .collect();
    let folders: Vec<&str> = names.iter().filter_map(|n| n.rsplit_once('/').map(|(f, _)| f)).filter(|f| !f.is_empty() && !f.contains('*')).collect();
    for s in steps.iter_mut().filter(|s| s.kind == Kind::Music) {
        let parts: Vec<&str> = s.from.split('/').collect();
        let file = parts.last().copied().unwrap_or_default();
        let named = names.iter().find(|n| !n.contains('*') && file_name(n).eq_ignore_ascii_case(file)).cloned();
        let below_its_folder = || {
            folders.iter().find_map(|f| {
                let first = f.split('/').next()?;
                let at = parts.iter().position(|p| p.eq_ignore_ascii_case(first))?;
                (at + 1 < parts.len()).then(|| parts[at..].join("/"))
            })
        };
        if let Some(rel) = named.or_else(below_its_folder) {
            s.to = Some(layout.tables.join("music").join(rel));
        }
    }
}

/// Colour files and an AltSound pack made for another set than the one their table plays (`rom`),
/// put where they work, or said why they won't — `family`, the sets of the table's game PinMAME
/// knows; `known`, whether PinMAME knows a set at all:
/// - a colour file named for another version of the game kept as that version's, as a colouring
///   fits its own version's pictures: Change the ROM to that version shows it;
/// - an AltSound pack for another version put in as the table's set's, as packs play across a
///   game's versions;
/// - a colour file named for nothing PinMAME knows (`Batman_ColorDMD_v1.02.vni`) put in under the
///   name the player looks for, `<rom>.cRZ` or `pin2dmd.pal`, `.vni` and `.pac`;
/// - another machine's left out, saying so.
///
/// The steps left out, by index.
fn fitted_to_its_rom(steps: &mut [Step], contents: &Contents, rom: &str, family: &[String], known: &dyn Fn(&str) -> bool, layout: &Layout) -> Vec<usize> {
    let mut left_out = Vec::new();
    let altcolor = layout.vpinmame.join("altcolor");
    let installed = |set: &str| crate::roms::installed(&layout.vpinmame.join("roms"), set);
    let of_the_game = |set: &str| family.iter().any(|f| f.eq_ignore_ascii_case(set));
    for (i, s) in steps.iter_mut().enumerate() {
        let Kind::Colouring(colouring) = s.kind else { continue };
        let name = file_name(&s.from).to_string();
        let (own, ext) = name.rsplit_once('.').map_or((name.as_str(), ""), |(stem, ext)| (stem, ext));
        let pin2dmd = !matches!(colouring, crate::analysis::Colouring::Serum);
        if own.eq_ignore_ascii_case(rom) || (pin2dmd && own.eq_ignore_ascii_case("pin2dmd")) {
            continue;
        }
        if known(own) {
            if of_the_game(own) {
                s.to = Some(altcolor.join(own).join(&name));
                let get = if installed(own) { "" } else { " (its ROM isn't among yours yet)" };
                s.note = Some(format!("made for {own}, another version of its game: kept as {own}'s, as a coloring fits its own version's pictures — Change the ROM on its page to {own} to see it{get}"));
            } else {
                s.note = Some(format!("a color file for {own}, another machine: left out"));
                left_out.push(i);
            }
            continue;
        }
        // named for nothing PinMAME knows: as the player looks for the table's colouring
        let named = if pin2dmd { format!("pin2dmd.{ext}") } else { format!("{rom}.{ext}") };
        s.to = Some(altcolor.join(rom).join(&named));
        s.note = Some(format!("named {named}, as the player looks for its table's coloring"));
    }
    // an AltSound pack, by its folder's name
    if let Some(own) = contents.altsound_folder.as_deref().map(|f| f.rsplit('/').next().unwrap_or(f)).filter(|own| !own.eq_ignore_ascii_case(rom)) {
        let (note, out) = if !known(own) {
            (format!("the pack's folder `{own}` put in as {rom}'s, the ROM its table plays"), false)
        } else if of_the_game(own) {
            (format!("made for {own}, another version of its game: put in as {rom}'s, as AltSound packs play across a game's versions"), false)
        } else {
            (format!("an AltSound pack for {own}, another machine: left out"), true)
        };
        for (i, s) in steps.iter_mut().enumerate().filter(|(_, s)| s.kind == Kind::AltSound) {
            s.note = Some(note.clone());
            if out {
                left_out.push(i);
            }
        }
    }
    left_out
}

/// Pictures nothing placed that are named as one of the table's own pictures (`table`, its file on
/// disk: `backdrop2.png` for its picture `backdrop2`) are pictures for the table: kept in `<table>
/// pictures` beside it, where the player reads each in its own's place, the table file left as it
/// is (ours: Terrifier's media pack brings its alternative desktop backdrop so, for VPX users to
/// swap into the table in its editor).
fn pictures_for_the_table(steps: &mut [Step], table: Option<&Path>, table_stem: &str, layout: &Layout) {
    let picture = |p: &str| p.rsplit_once('.').is_some_and(|(_, e)| crate::analysis::PICTURE_SUFFIXES.contains(&e.to_lowercase().as_str()));
    let wanted: Vec<usize> = steps.iter().enumerate().filter(|(_, s)| s.to.is_none() && matches!(s.kind, Kind::Other | Kind::Media) && picture(&s.from)).map(|(i, _)| i).collect();
    if wanted.is_empty() {
        return;
    }
    let Some(names) = table.and_then(|t| crate::table::picture_names(t).ok()) else { return };
    for i in wanted {
        let s = &mut steps[i];
        if let Some(name) = names.iter().find(|n| n.eq_ignore_ascii_case(stem(&s.from))) {
            s.kind = Kind::TablePicture;
            s.to = Some(layout.tables.join(format!("{table_stem} pictures")).join(file_name(&s.from)));
            s.note = Some(format!("played in place of the table's own picture “{name}” once ticked; the table file stays as it is"));
        }
    }
}

/// A download listed and its table read, before it's sorted out: its name, where it came from, its
/// listing and contents (or why they couldn't be read), and the table it brings, with a note.
type Listed = (String, PathBuf, Result<(Listing, Contents), String>, Option<(TableFound, Option<String>)>);

/// A drop's downloads, sorted out: the tables they bring matched to VPS, the rest tied to tables in
/// the library or brought by the drop itself — or by the downloads the window shows already,
/// `before` — as `tie` says, every file's place in `layout`.
pub fn inspect(paths: &[PathBuf], db: &Database, known: &[Known], layout: &Layout, tie: Tie, before: &[Known]) -> Vec<Download> {
    let items: Vec<(String, PathBuf, Result<Listing, String>)> = paths.iter().flat_map(|p| downloads_in(p)).collect();
    let mut out: Vec<Download> = Vec::new();
    // the tables first, so the rest can be tied to the ones the drop brings
    let mut brought: Vec<Known> = Vec::new();
    let mut read: Vec<Listed> = Vec::new();
    for (name, source, listing) in items {
        match listing {
            Ok(l) => {
                let contents = analyse(&l);
                let table = contents.tables().first().map(|e| found(&l, e, db));
                if let Some((t, _)) = &table {
                    brought.push(Known::from(t));
                }
                read.push((name, source, Ok((l, contents)), table));
            }
            Err(e) => read.push((name, source, Err(e), None)),
        }
    }
    // the tables being added: this drop's and those the window shows already, each once
    for b in before {
        if !brought.iter().any(|x| x.stem.eq_ignore_ascii_case(&b.stem)) {
            brought.push(b.clone());
        }
    }
    let all_known = known_with(&brought, known);
    for (name, source, listing, table) in read {
        match listing {
            Ok((listing, contents)) => out.push(sorted(name, source, listing, contents, table, &all_known, &brought, db, layout, tie)),
            Err(e) => out.push(Download { source, name, what: String::new(), table: None, for_table: None, for_note: None, rom: None, pup: None, stem: None, media_name: None, notes: Vec::new(), rows: Vec::new(), readme: None, error: Some(e), listing: None, steps: Vec::new() }),
        }
    }
    note_missing_roms(&mut out, layout);
    out
}

/// A table the drop brings whose machine's ROM is neither in the drop nor among the ROMs: said, as
/// the table won't start without it.
fn note_missing_roms(downloads: &mut [Download], layout: &Layout) {
    let brought: Vec<String> = downloads
        .iter()
        .flat_map(|d| d.steps.iter().filter(|s| s.kind == Kind::Rom).filter_map(|s| Some(s.to.as_ref()?.file_stem()?.to_string_lossy().to_lowercase())))
        .collect();
    let roms = layout.vpinmame.join("roms");
    for d in downloads.iter_mut() {
        let Some(rom) = d.table.as_ref().filter(|t| t.pinmame).and_then(|t| t.game_name.clone()) else { continue };
        if !brought.contains(&rom.to_lowercase()) && !crate::roms::installed(&roms, &rom) {
            d.notes.push(format!("Its ROM, {rom}, isn't in this drop or among your ROMs: the table won't start until it's added (VPS lists its ROMs)"));
        }
    }
}

/// The tables being added, then the library's: a table being added counts once, before the
/// library's own of the same name.
pub fn known_with(brought: &[Known], known: &[Known]) -> Vec<Known> {
    brought.iter().cloned().chain(known.iter().filter(|k| !brought.iter().any(|b| b.stem.eq_ignore_ascii_case(&k.stem))).cloned()).collect()
}

/// A download sorted out again for the table the user chose for it.
pub fn with_table(d: &Download, db: &Database, layout: &Layout, table: &Known) -> Download {
    let Some(listing) = d.listing.clone() else { return d.clone() };
    let contents = analyse(&listing);
    let mut out = sorted(d.name.clone(), d.source.clone(), listing, contents, None, &[], &[], db, layout, Tie::On(table));
    out.for_note = Some("as you chose".into());
    out
}

/// A download that brings no table and was tied to none sorted out again, a table having joined
/// the window since, as files belonging to the table being added: `all_known` and `brought` as
/// `inspect` makes them.
pub fn again(d: &Download, db: &Database, all_known: &[Known], brought: &[Known], layout: &Layout) -> Download {
    let Some(listing) = d.listing.clone() else { return d.clone() };
    let contents = analyse(&listing);
    sorted(d.name.clone(), d.source.clone(), listing, contents, None, all_known, brought, db, layout, Tie::Found)
}

/// One download sorted out: its table or the table it's for — `all_known`, the tables being added
/// (`brought`) and the library's, as `tie` says — its files' places, its rows.
#[allow(clippy::too_many_arguments)]
fn sorted(name: String, source: PathBuf, listing: Listing, mut contents: Contents, table: Option<(TableFound, Option<String>)>, all_known: &[Known], brought: &[Known], db: &Database, layout: &Layout, tie: Tie) -> Download {
    {
        let mut notes = Vec::new();
        let tables = contents.tables().len();
        if tables > 1 {
            notes.push(format!("{tables} tables in it: the first is added; drop the others by themselves"));
        }
        let (table, for_table_stem, for_note, rom, pup, helpers, music, table_file, home) = match table {
            Some((t, note)) => {
                notes.extend(note);
                notes.extend(backglass_of_another(&contents, t.game_id.as_deref(), db));
                let rom = t.game_name.clone().or_else(|| contents.rom_hints().into_iter().next());
                let pup = t.pup_names.first().cloned();
                let stem = t.stem.clone();
                let helpers = t.helpers.clone();
                let (music, file) = (t.music.clone(), t.on_disk.clone());
                let home = game_folder(layout, db, t.game_id.as_deref(), &stem);
                (Some(t), Some(stem), None, rom, pup, helpers, music, file, home)
            }
            None => {
                let (k, how) = match tie {
                    Tie::On(k) => (Some(k), Some("dropped on it".to_string())),
                    // a table being added with it by what it is, else the window's own
                    Tie::Joining(k) => match table_for(&contents, brought, db) {
                        Ok((b, how)) => (Some(b), Some(how)),
                        Err(_) => (Some(k), Some("dropped on it".to_string())),
                    },
                    Tie::Found => match table_for(&contents, all_known, db) {
                        Ok((k, how)) => (Some(k), Some(how)),
                        // nothing saying which, and one table being added: that one
                        Err(Untied::Unknown) if brought.len() == 1 => (Some(&brought[0]), Some("being added with it".to_string())),
                        Err(why) => (None, Some(why.said())),
                    },
                };
                if k.is_some() {
                    crate::analysis::loose_music_for_a_table(&mut contents);
                }
                notes.extend(backglass_of_another(&contents, k.and_then(|k| k.vps_game.as_deref()), db));
                let rom = k.and_then(|k| k.game_name.clone()).or_else(|| contents.rom_hints().into_iter().next());
                let pup = k.and_then(|k| k.pup_names.first().cloned());
                let helpers = k.map(|k| k.helpers.clone()).unwrap_or_default();
                let music = k.map(|k| k.music.clone()).unwrap_or_default();
                // its file: as the table being added knows it, else the library's, named for it
                let file = k.and_then(|k| k.file.clone().or_else(|| Some(layout.tables.join(format!("{}.vpx", k.stem))).filter(|p| p.is_file())));
                // a table being added (not in the library yet): its game's folder, where it'll go
                let home = k.filter(|_| !file.as_ref().is_some_and(|f| f.starts_with(&layout.tables))).and_then(|k| game_folder(layout, db, k.vps_game.as_deref(), &k.stem));
                (None, k.map(|k| k.stem.clone()), how, rom, pup, helpers, music, file, home)
            }
        };
        let mut steps = plan(&contents, &listing.source, for_table_stem.as_deref(), rom.as_deref(), pup.as_deref(), &helpers, layout);
        let mut left_out = Vec::new();
        if let Some(stem) = for_table_stem.as_deref() {
            script_by_its_name(&mut steps, &listing, &contents, stem, &helpers, layout);
            music_as_its_script_plays(&mut steps, &music, layout);
            pictures_for_the_table(&mut steps, table_file.as_deref(), stem, layout);
            if let Some(dir) = home.clone().or_else(|| table_file.as_deref().and_then(Path::parent).map(Path::to_path_buf)) {
                // a table being added goes into its game's folder, and a folder made for it gets
                // the folders its ROM and packs go in
                if home.is_some() {
                    for s in steps.iter_mut().filter(|s| s.kind == Kind::Table) {
                        s.to = s.to.as_deref().and_then(Path::file_name).map(|n| dir.join(n));
                    }
                }
                into_its_own_folder(&mut steps, layout, &dir, home.is_some() && !dir.exists());
            }
            if let Some(rom) = rom.as_deref()
                && contents.files.iter().any(|(_, k)| matches!(k, Kind::Colouring(_) | Kind::AltSound))
            {
                let family: Vec<String> = crate::roms::same_game(rom).into_iter().map(|k| k.set).collect();
                let all = crate::roms::all_sets().unwrap_or_default();
                let known = |set: &str| all.iter().any(|k| k.set.eq_ignore_ascii_case(set));
                left_out = fitted_to_its_rom(&mut steps, &contents, rom, &family, &known, layout);
            }
        }
        let checks = check_roms(&mut steps, &listing, rom.as_deref(), layout);
        let mut rows = rows_of(&steps);
        // media placed: in the front end's folders they go in, by its name
        if rows.iter().any(|r| r.kind == "media" && r.folder.is_some())
            && let Some(target) = crate::frontends::media_target(layout)
        {
            for row in rows.iter_mut().filter(|r| r.kind == "media" && r.folder.is_some()) {
                row.folder = Some(format!("{}'s media", target.front().name()));
            }
        }
        // another machine's colour file or pack: not ticked
        for row in rows.iter_mut().filter(|r| r.steps.iter().any(|i| left_out.contains(i))) {
            row.include = false;
        }
        for (i, (look, text)) in checks {
            if let Some(row) = rows.iter_mut().find(|r| r.steps.contains(&i)) {
                // a set that isn't the files PinMAME expects isn't put over what's there unasked
                if look == "bad" {
                    row.include = false;
                }
                row.check = Some(RowCheck { look, text });
            }
        }
        // which table it's for unknown, nothing is ticked until the user says (its files would land
        // named for no table)
        if for_table_stem.is_none() {
            rows.iter_mut().for_each(|r| r.include = false);
        }
        let readme = contents.readme.as_ref().map(|r| r.lines().filter(|l| !l.trim().is_empty()).take(6).collect::<Vec<_>>().join("\n"));
        Download {
            source,
            name,
            what: described(&listing, contents.files.len()),
            for_table: if table.is_some() { None } else { for_table_stem.clone() },
            table,
            for_note,
            rom,
            pup,
            media_name: for_table_stem.as_deref().and_then(|s| crate::frontends::media_name_for(layout, s)),
            stem: for_table_stem.clone(),
            notes,
            rows,
            readme,
            error: None,
            listing: Some(listing),
            steps,
        }
    }
}

/// The folder a table being added goes in, in a library kept in a folder for each game: its
/// game's, named as VPS names the game (`Attack from Mars (Bally 1995)`), else for its file.
fn game_folder(layout: &Layout, db: &Database, vps_game: Option<&str>, stem: &str) -> Option<PathBuf> {
    if !layout.per_game {
        return None;
    }
    let name = vps_game.and_then(|g| db.game(g)).map(|g| crate::rename::vps_name(&g.name, Some(&g.manufacturer), g.year)).unwrap_or_else(|| stem.to_string());
    let name = crate::layout_switch::folder_name(&name);
    (!name.is_empty()).then(|| layout.tables.join(name))
}

/// A table kept in a folder of its own (`Tables\<folder>\<table>.vpx`, VPX 10.8.1's layout): its
/// files go in that folder — what goes beside a table, its music, its pictures — and its ROM, AltSound
/// pack, colouring and PinUP pack in the folders it keeps them in there (`pinmame`, `pupvideos`),
/// where it has them, or all of them in a folder made for it (`new_folder`), as the switch to a
/// folder for each game lays one out; VPinMAME's and PinUP's shared folders otherwise. A table
/// beside the others changes nothing.
fn into_its_own_folder(steps: &mut [Step], layout: &Layout, dir: &Path, new_folder: bool) {
    let same = |a: &Path, b: &Path| a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy());
    if same(dir, &layout.tables) || !dir.starts_with(&layout.tables) {
        return;
    }
    // each shared folder and the table's own in its place, where the table has that one, the first
    // there as VPX's plugins look: its ROMs only where it has `pinmame\roms` (PinMAME then plays
    // from its `pinmame` folder alone), a Serum colouring in `serum` or a PIN2DMD one in `vni`
    // before `pinmame\altcolor`
    let own = |parts: &[&str]| -> Option<PathBuf> {
        let p = parts.iter().fold(dir.to_path_buf(), |p, part| p.join(part));
        (new_folder || p.is_dir()).then_some(p)
    };
    let shared_colour = layout.vpinmame.join("altcolor");
    let colour_for = |kind: Kind| match kind {
        Kind::Colouring(crate::analysis::Colouring::Serum) => own(&["serum"]).or_else(|| own(&["pinmame", "altcolor"])),
        Kind::Colouring(_) => own(&["vni"]).or_else(|| own(&["pinmame", "altcolor"])),
        _ => None,
    };
    let moves: Vec<(PathBuf, PathBuf)> = [
        (layout.vpinmame.join("roms"), own(&["pinmame", "roms"])),
        (layout.vpinmame.join("altsound"), own(&["altsound"]).or_else(|| own(&["pinmame", "altsound"]))),
        (layout.pup_videos.clone(), own(&["pupvideos"])),
    ]
    .into_iter()
    .filter_map(|(shared, own)| Some((shared, own?)))
    .collect();
    for s in steps.iter_mut() {
        let Some(to) = s.to.clone() else { continue };
        // a new table, or a mod made beside its original, stays where the plan put it
        if matches!(s.kind, Kind::Table | Kind::Patch) {
            continue;
        }
        let colour = colour_for(s.kind).and_then(|own| to.strip_prefix(&shared_colour).ok().map(|rest| own.join(rest)));
        let moved = colour.or_else(|| moves.iter().find_map(|(shared, own)| to.strip_prefix(shared).ok().map(|rest| own.join(rest))));
        let beside = || to.strip_prefix(&layout.tables).ok().map(|rest| dir.join(rest));
        if let Some(new) = moved.or_else(beside) {
            s.to = Some(new);
        }
    }
}

/// Each ROM set the drop brings, as a zip, checked against the files PinMAME expects for it
/// (`roms`): under its own name, and when that's not a complete set, against the set its table asks
/// for (`asked`), named for that one when it is it under another name. Each one's verdict, by its
/// step's index; one that isn't sound where the set installed is says so, to be left out unless the
/// user ticks it (found trying it: a clone's own file alone would have replaced Attack from Mars's
/// whole set, its parent's files with it).
fn check_roms(steps: &mut [Step], listing: &Listing, asked: Option<&str>, layout: &Layout) -> Vec<(usize, (&'static str, String))> {
    let shared_roms = layout.vpinmame.join("roms");
    let is_set = |s: &Step| s.kind == Kind::Rom && s.to.as_ref().is_some_and(|t| t.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")));
    // the sets the drop brings, for a clone's parent brought beside it
    let brought: Vec<String> = steps.iter().filter(|s| is_set(s)).filter_map(|s| Some(s.to.as_ref()?.file_stem()?.to_string_lossy().into_owned())).collect();
    let mut out = Vec::new();
    for (i, step) in steps.iter_mut().enumerate() {
        if !is_set(step) {
            continue;
        }
        let found = match rom_files_of(step, listing) {
            Ok(f) => f,
            Err(e) => {
                out.push((i, ("warn", format!("Its files couldn't be read to check them: {e}"))));
                continue;
            }
        };
        let own = step.to.as_ref().and_then(|t| t.file_stem()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // the folder it goes in: VPinMAME's, or its table's own `pinmame\roms`
        let rom_dir = step.to.as_ref().and_then(|t| t.parent()).map(Path::to_path_buf).unwrap_or_else(|| shared_roms.clone());
        let mut c = match crate::roms::set(&own) {
            Ok(s) => crate::roms::check(&s, &found),
            Err(e) => {
                out.push((i, ("warn", format!("Not checked: {e}"))));
                continue;
            }
        };
        // the set its table asks for, under another name: named for it
        if let Some(asked) = asked.filter(|a| !a.eq_ignore_ascii_case(&own))
            && !c.complete()
            && let Ok(s) = crate::roms::set(asked)
        {
            let a = crate::roms::check(&s, &found);
            if a.complete() {
                step.note = Some(format!("named {}.zip, the set its table asks for (it came as {own}.zip)", s.set));
                step.to = Some(rom_dir.join(format!("{}.zip", s.set)));
                c = a;
            }
        }
        let parent_there = c.parent.as_deref().is_some_and(|p| crate::roms::installed(&rom_dir, p) || brought.iter().any(|b| b.eq_ignore_ascii_case(p)));
        let (mut look, mut text) = crate::roms::verdict(&c, parent_there, crate::roms::Of::Drop);
        // the set there now, when it's sound and this one isn't: kept
        if look != "good"
            && let Some(to) = step.to.as_ref().filter(|t| t.is_file())
            && let Ok(there) = std::fs::File::open(to).map_err(|e| e.to_string()).and_then(crate::roms::zip_files)
            && let Ok(set) = crate::roms::set(&c.set)
            && crate::roms::verdict(&crate::roms::check(&set, &there), parent_there, crate::roms::Of::Table).0 == "good"
        {
            look = "bad";
            text.push_str(" The ROM there now is sound, so it's kept unless you tick this.");
        }
        out.push((i, (look, text)));
    }
    out
}

/// The files of a ROM set the drop brings as a zip: from the zip's own index — the drop itself, or
/// a zip inside it — or, a set dropped whole in another kind of archive, each file read for its CRC.
fn rom_files_of(step: &Step, listing: &Listing) -> Result<Vec<crate::roms::Found>, String> {
    if let Source::Archive(a) = &listing.source
        && step.from == file_name(&a.path().to_string_lossy().replace('\\', "/"))
    {
        if a.format == archive::Format::Zip {
            return crate::roms::zip_files(std::fs::File::open(a.path()).map_err(|e| format!("{}: {e}", a.path().display()))?);
        }
        return listing
            .entries
            .iter()
            .map(|e| {
                let mut bytes = Vec::new();
                listing.copy_out(&e.path, &mut bytes, 64 << 20)?;
                Ok(crate::roms::found(&e.path, &bytes))
            })
            .collect();
    }
    let mut bytes = Vec::new();
    listing.copy_out(&step.from, &mut bytes, 64 << 20)?;
    crate::roms::zip_files_in(bytes)
}

/// A script about to be put beside the table under its name, to play in place of the table's own,
/// that isn't a table's script (`table::is_table_script`) is one its table loads under another
/// name: it goes with VPX's scripts under the name the table's script asks for it by — the one
/// script it loads that isn't VPX's own and isn't in the drop under its own name, or of several,
/// the one not there yet — else it's left out, saying why (ours: Beavis and Butt-head's system
/// script came named for its table, which would have played it in the table's own's place).
fn script_by_its_name(steps: &mut [Step], listing: &Listing, contents: &Contents, stem: &str, helpers: &[String], layout: &Layout) {
    let beside = layout.tables.join(format!("{stem}.vbs"));
    let in_drop = |h: &String| contents.files.iter().any(|(e, _)| file_name(&e.path).eq_ignore_ascii_case(h));
    let candidates: Vec<&String> = helpers.iter().filter(|h| !crate::installed::is_vpx_script(h) && !in_drop(h)).collect();
    let there = |h: &String| layout.scripts.join(h).exists() || layout.tables.join(h).exists();
    let missing: Vec<&String> = candidates.iter().copied().filter(|h| !there(h)).collect();
    let pick = match (candidates.as_slice(), missing.as_slice()) {
        ([one], _) | (_, [one]) => Some(*one),
        _ => None,
    };
    for s in steps.iter_mut().filter(|s| s.kind == Kind::Script && s.to.as_deref() == Some(beside.as_path())) {
        let mut text = Vec::new();
        if listing.copy_out(&s.from, &mut text, 4 << 20).is_err() || crate::table::is_table_script(&String::from_utf8_lossy(&text)) {
            continue;
        }
        match pick {
            Some(name) => {
                s.to = Some(layout.scripts.join(name));
                s.kind = Kind::HelperScript;
                s.note = Some(format!("not a table's script but the one its table loads as `{name}`, put with VPX's scripts under that name (named for the table, it would play in place of the table's own)"));
            }
            None => {
                s.to = None;
                s.note = Some("not a table's script, nor one its table asks for by a name we can tell: named for the table, it would play in place of the table's own, so it's left out".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-drop-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn a_folder_of_downloads_is_sorted_into_them() {
        let d = scratch("many");
        touch(&d.join("Taxi.vpx"));
        touch(&d.join("Taxi.directb2s"));
        touch(&d.join("Medieval Madness/Medieval Madness.vpx"));
        touch(&d.join("Medieval Madness/mm_109c.zip"));
        touch(&d.join("afm_113b.zip"));
        touch(&d.join("Some backglass.directb2s"));
        let got: Vec<(String, usize)> = downloads_in(&d).into_iter().map(|(n, _, l)| (n, l.map(|l| l.entries.len()).unwrap_or(0))).collect();
        assert!(got.contains(&("afm_113b.zip".into(), 0)), "{got:?}");
        assert!(got.contains(&("Medieval Madness".into(), 2)), "{got:?}");
        assert!(got.contains(&("Taxi.vpx".into(), 2)), "{got:?}");
        assert!(got.contains(&("Some backglass.directb2s".into(), 1)), "{got:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_pinup_packs_folder_stays_whole_beside_other_downloads() {
        let d = scratch("pup");
        touch(&d.join("munsters_2screen.zip"));
        touch(&d.join("mmunsters/screens.pup"));
        touch(&d.join("mmunsters/playlists.pup"));
        touch(&d.join("mmunsters/munsters_3screen_fulldmd.zip"));
        touch(&d.join("mmunsters/videoattract/attract.mp4"));
        let got: Vec<(String, usize)> = downloads_in(&d).into_iter().map(|(n, _, l)| (n, l.map(|l| l.entries.len()).unwrap_or(0))).collect();
        assert!(got.contains(&("mmunsters".into(), 4)), "{got:?}");
        assert_eq!(got.len(), 2, "{got:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn one_tables_folder_is_one_download() {
        let d = scratch("one");
        touch(&d.join("Taxi.vpx"));
        touch(&d.join("Taxi.directb2s"));
        touch(&d.join("roms/taxi_l4.zip"));
        let got = downloads_in(&d);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].2.as_ref().unwrap().entries.len(), 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_system_script_named_for_its_table_goes_by_the_name_the_table_asks() {
        let d = scratch("named");
        let name = "Beavis and Butt-head_Pinballed.vbs";
        std::fs::write(d.join(name), "LoadCore\nPrivate Sub LoadCore\nEnd Sub\nConst swLRFlip = 82\n").unwrap();
        std::fs::write(d.join("Other.vbs"), "Const cGameName = \"beav_butt\"\nSub Table1_KeyDown(ByVal keycode)\nEnd Sub\n").unwrap();
        let listing = archive::list(&d).unwrap();
        let contents = analyse(&listing);
        let layout = Layout::under(&d.join("layout"));
        let stem = "Beavis and Butt-head_Pinballed";
        let step = |from: &str| Step { from: from.into(), kind: Kind::Script, size: 1, to: Some(layout.tables.join(format!("{stem}.vbs"))), note: None };
        let mut steps = vec![step(name), step("Other.vbs")];
        script_by_its_name(&mut steps, &listing, &contents, stem, &["core.vbs".into(), "beav_butt.vbs".into()], &layout);
        // the system script by the name the table's script loads it by; a table's script left as it was
        assert_eq!((steps[0].kind, steps[0].to.clone()), (Kind::HelperScript, Some(layout.scripts.join("beav_butt.vbs"))));
        assert_eq!((steps[1].kind, steps[1].to.clone()), (Kind::Script, Some(layout.tables.join(format!("{stem}.vbs")))));
        // nothing the table asks for: left out
        let mut steps = vec![step(name)];
        script_by_its_name(&mut steps, &listing, &contents, stem, &["core.vbs".into()], &layout);
        assert_eq!(steps[0].to, None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn files_dropped_after_a_table_are_tied_to_it() {
        let d = scratch("tie");
        let layout = Layout::under(&d.join("layout"));
        std::fs::create_dir_all(d.join("drop")).unwrap();
        // a backglass named for nothing: the one table being added, as the window shows it already
        touch(&d.join("drop/Some backglass.directb2s"));
        let db = Database::default();
        let afm = Known { stem: "Attack from Mars (Bally 1995)".into(), game_name: Some("afm_113b".into()), ..Default::default() };
        let tz = Known { stem: "Twilight Zone (Bally 1993)".into(), game_name: Some("tz_92".into()), ..Default::default() };
        let got = inspect(&[d.join("drop/Some backglass.directb2s")], &db, &[tz.clone()], &layout, Tie::Found, std::slice::from_ref(&afm));
        assert_eq!((got[0].for_table.as_deref(), got[0].for_note.as_deref()), (Some(afm.stem.as_str()), Some("being added with it")));
        assert!(got[0].rows.iter().any(|r| r.include), "its rows ticked once it's tied");
        // two tables being added: nothing says which, so it's to choose
        let mm = Known { stem: "Medieval Madness (Williams 1997)".into(), ..Default::default() };
        let got = inspect(&[d.join("drop/Some backglass.directb2s")], &db, &[], &layout, Tie::Found, &[afm.clone(), mm]);
        assert_eq!(got[0].for_table, None);
        // the window opened for a table: a later file goes with it, as the first ones did
        let got = inspect(&[d.join("drop/Some backglass.directb2s")], &db, &[], &layout, Tie::Joining(&tz), &[]);
        assert_eq!(got[0].for_table.as_deref(), Some(tz.stem.as_str()));
        // but one named for a table being added goes with that one
        touch(&d.join("drop/Attack from Mars (Bally 1995).directb2s"));
        let got = inspect(&[d.join("drop/Attack from Mars (Bally 1995).directb2s")], &db, &[], &layout, Tie::Joining(&tz), std::slice::from_ref(&afm));
        assert_eq!((got[0].for_table.as_deref(), got[0].for_note.as_deref()), (Some(afm.stem.as_str()), Some("named for it")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn music_goes_where_its_tables_script_plays_it() {
        let layout = Layout::under(Path::new("V"));
        let s = |from: &str| Step { from: from.into(), kind: Kind::Music, size: 1, to: Some(layout.tables.join("music").join(from)), note: None };
        let mut steps = vec![s("Media Pack/Music Files/BLIZZARD/01 - I Don_t Wanna Stop.mp3"), s("Media Pack/Music Files/BLIZZARD/02 - Perry Mason.mp3"), s("Other/song.mp3")];
        // one named by the script; another below a folder the script plays from; one it doesn't name
        music_as_its_script_plays(&mut steps, &["BLIZZARD\\01 - I Don_t Wanna Stop.mp3".into(), "BLIZZARD\\*".into(), "Music\\BLIZZARD\\x.mp3".into()], &layout);
        assert_eq!(steps[0].to, Some(layout.tables.join("music").join("BLIZZARD/01 - I Don_t Wanna Stop.mp3")));
        assert_eq!(steps[1].to, Some(layout.tables.join("music").join("BLIZZARD/02 - Perry Mason.mp3")));
        assert_eq!(steps[2].to, Some(layout.tables.join("music").join("Other/song.mp3")));
    }

    #[test]
    fn a_picture_named_as_one_of_the_tables_is_a_picture_for_it() {
        let d = scratch("picture");
        // a table file with one picture, `backdrop2`: its stream's first record, NAME
        let table = d.join("Terrifier.vpx");
        {
            let mut stg = cfb::create(&table).unwrap();
            stg.create_storage("/GameStg").unwrap();
            let mut s = stg.create_stream("/GameStg/Image0").unwrap();
            let name = b"backdrop2";
            use std::io::Write as _;
            s.write_all(&((8 + name.len()) as u32).to_le_bytes()).unwrap();
            s.write_all(b"NAME").unwrap();
            s.write_all(&(name.len() as u32).to_le_bytes()).unwrap();
            s.write_all(name).unwrap();
            s.flush().unwrap();
            drop(s);
            stg.flush().unwrap();
        }
        assert_eq!(crate::table::picture_names(&table).unwrap(), ["backdrop2"]);
        let layout = Layout::under(&d.join("layout"));
        let s = |from: &str| Step { from: from.into(), kind: Kind::Other, size: 1, to: None, note: None };
        let mut steps = vec![s("Alternate Desktop users backdrop/backdrop2.png"), s("Other/backdrop3.png")];
        pictures_for_the_table(&mut steps, Some(&table), "Terrifier", &layout);
        assert_eq!((steps[0].kind, steps[0].to.clone()), (Kind::TablePicture, Some(layout.tables.join("Terrifier pictures").join("backdrop2.png"))));
        assert_eq!((steps[1].kind, steps[1].to.clone()), (Kind::Other, None));
        // offered, not ticked
        let rows = rows_of(&steps);
        assert_eq!((rows[0].kind.as_str(), rows[0].include), ("tablePicture", false));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_tables_files_go_in_its_own_folder_where_it_keeps_one() {
        use crate::analysis::Colouring;
        let root = std::env::temp_dir().join(format!("vpi-own-folder-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::under(&root);
        let dir = layout.tables.join("Attack from Mars (Bally 1995)");
        // its own ROMs and Serum folders; no AltSound or PinUP folder of its own
        std::fs::create_dir_all(dir.join("pinmame").join("roms")).unwrap();
        std::fs::create_dir_all(dir.join("serum")).unwrap();
        let step = |kind: Kind, to: PathBuf| Step { from: "x".into(), kind, size: 1, to: Some(to), note: None };
        let mut steps = vec![
            step(Kind::Backglass, layout.tables.join("afm.directb2s")),
            step(Kind::Rom, layout.vpinmame.join("roms").join("afm_113b.zip")),
            step(Kind::Colouring(Colouring::Serum), layout.vpinmame.join("altcolor").join("afm_113b").join("afm_113b.cRZ")),
            step(Kind::Colouring(Colouring::Pin2dmd), layout.vpinmame.join("altcolor").join("afm_113b").join("pin2dmd.pal")),
            step(Kind::AltSound, layout.vpinmame.join("altsound").join("afm_113b").join("altsound.csv")),
            step(Kind::Music, layout.tables.join("music").join("theme.mp3")),
            step(Kind::PupPack, layout.pup_videos.join("afm_113b").join("screens.pup")),
            step(Kind::Table, layout.tables.join("afm 2.vpx")),
        ];
        into_its_own_folder(&mut steps, &layout, &dir, false);
        let got: Vec<PathBuf> = steps.iter().map(|s| s.to.clone().unwrap()).collect();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            got,
            [
                dir.join("afm.directb2s"),
                dir.join("pinmame").join("roms").join("afm_113b.zip"),
                dir.join("serum").join("afm_113b").join("afm_113b.cRZ"),
                // a PIN2DMD colouring with no folder of its own there: VPinMAME's
                layout.vpinmame.join("altcolor").join("afm_113b").join("pin2dmd.pal"),
                layout.vpinmame.join("altsound").join("afm_113b").join("altsound.csv"),
                dir.join("music").join("theme.mp3"),
                layout.pup_videos.join("afm_113b").join("screens.pup"),
                // a new table stays where the plan put it
                layout.tables.join("afm 2.vpx"),
            ]
        );
    }

    #[test]
    fn a_table_added_to_a_library_kept_per_game_gets_a_folder_of_its_own() {
        use crate::analysis::Colouring;
        let mut layout = Layout::under(Path::new("V-no-such-folder"));
        // kept together: no folder of its own
        assert_eq!(game_folder(&layout, &Database::default(), None, "afm VPW"), None);
        layout.per_game = true;
        // its game's folder, named for its file where VPS doesn't know its game
        let dir = game_folder(&layout, &Database::default(), None, "afm VPW").unwrap();
        assert_eq!(dir, layout.tables.join("afm VPW"));
        // a folder made for it: every kind in the folder VPX 10.8.1 reads it from there
        let step = |kind: Kind, to: PathBuf| Step { from: "x".into(), kind, size: 1, to: Some(to), note: None };
        let mut steps = vec![
            step(Kind::Rom, layout.vpinmame.join("roms").join("afm_113b.zip")),
            step(Kind::Colouring(Colouring::Serum), layout.vpinmame.join("altcolor").join("afm_113b").join("afm_113b.cRZ")),
            step(Kind::Colouring(Colouring::Pin2dmd), layout.vpinmame.join("altcolor").join("afm_113b").join("pin2dmd.pal")),
            step(Kind::AltSound, layout.vpinmame.join("altsound").join("afm_113b").join("altsound.csv")),
            step(Kind::PupPack, layout.pup_videos.join("afm_113b").join("screens.pup")),
            step(Kind::Backglass, layout.tables.join("afm VPW.directb2s")),
        ];
        into_its_own_folder(&mut steps, &layout, &dir, true);
        let got: Vec<PathBuf> = steps.iter().map(|s| s.to.clone().unwrap()).collect();
        assert_eq!(
            got,
            [
                dir.join("pinmame").join("roms").join("afm_113b.zip"),
                dir.join("serum").join("afm_113b").join("afm_113b.cRZ"),
                dir.join("vni").join("afm_113b").join("pin2dmd.pal"),
                dir.join("altsound").join("afm_113b").join("altsound.csv"),
                dir.join("pupvideos").join("afm_113b").join("screens.pup"),
                dir.join("afm VPW.directb2s"),
            ]
        );
    }

    #[test]
    fn colour_files_and_packs_fitted_to_the_rom_the_table_plays() {
        use crate::analysis::Colouring;
        let layout = Layout::under(Path::new("V-no-such-folder"));
        let colour = |from: &str, c| Step { from: from.into(), kind: Kind::Colouring(c), size: 1, to: Some(PathBuf::from("as planned")), note: None };
        let mut steps = vec![
            colour("tz_94h.cRZ", Colouring::Serum),
            colour("Batman_ColorDMD_v1.02.pal", Colouring::Pin2dmd),
            colour("afm_113b.cRZ", Colouring::Serum),
            colour("tz_92.cRZ", Colouring::Serum),
            Step { from: "altsound/tz_94h/altsound.csv".into(), kind: Kind::AltSound, size: 1, to: None, note: None },
        ];
        let contents = Contents { files: Vec::new(), pup_root: None, altsound_folder: Some("altsound/tz_94h".into()), dmd_folder: None, music_folder: None, readme: None, rom_set: None };
        let family = ["tz_92".to_string(), "tz_94h".to_string()];
        let known = |set: &str| ["tz_92", "tz_94h", "afm_113b"].contains(&set.to_lowercase().as_str());
        let left_out = fitted_to_its_rom(&mut steps, &contents, "tz_92", &family, &known, &layout);
        let altcolor = layout.vpinmame.join("altcolor");
        // another version's colouring kept as its own, Change the ROM said
        assert_eq!(steps[0].to, Some(altcolor.join("tz_94h").join("tz_94h.cRZ")));
        assert!(steps[0].note.as_deref().is_some_and(|n| n.contains("Change the ROM")));
        // a pair named for nothing PinMAME knows, under the name the player looks for
        assert_eq!(steps[1].to, Some(altcolor.join("tz_92").join("pin2dmd.pal")));
        // another machine's left out; the table's own as planned
        assert_eq!(left_out, [2]);
        assert_eq!(steps[3].to, Some(PathBuf::from("as planned")));
        // another version's AltSound pack put in as the table's set's
        assert!(steps[4].note.as_deref().is_some_and(|n| n.contains("another version")));
    }

    #[test]
    fn a_rar_archives_later_parts_come_with_its_first() {
        assert!(later_part(Path::new("x.part2.rar")));
        assert!(!later_part(Path::new("x.part1.rar")));
        assert!(later_part(Path::new("x.r00")));
        assert!(!later_part(Path::new("x.rar")));
    }

    #[test]
    fn a_packs_files_are_one_row() {
        let s = |from: &str, kind, to: &str| Step { from: from.into(), kind, size: 10, to: Some(PathBuf::from(to)), note: None };
        let steps = vec![
            s("Taxi.vpx", Kind::Table, "T/Taxi.vpx"),
            s("pack/screens.pup", Kind::PupPack, "P/taxi/screens.pup"),
            s("pack/Backglass/a.mp4", Kind::PupPack, "P/taxi/Backglass/a.mp4"),
            Step { from: "read me.txt".into(), kind: Kind::Readme, size: 1, to: None, note: Some("shown".into()) },
        ];
        let rows = rows_of(&steps);
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[1].kind.as_str(), rows[1].files, rows[1].name.as_str()), ("pupPack", 2, "taxi"));
        assert_eq!((rows[2].state.as_str(), rows[2].include), ("skip", false));
    }
}
