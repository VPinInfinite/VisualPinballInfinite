//! The tables installed, for the app's library: each table file in the
//! tables folder read and matched to VPS, and what it holds beside it — its backglass file, ROM,
//! PinUP pack, AltSound pack, colouring, POV and settings files, and PinUP Popper's media for it —
//! each found where the player looks for it. What a table file says is remembered
//! (`Library\tables.json` in the player's data folder), so a table is read again only when its
//! file changes. Ours.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::matcher::{self, Clues, Found};
use crate::plan::Layout;
use crate::vps::{Database, Game, TableFile};

/// What a table file says about itself, remembered with its size and date.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Read {
    pub size: u64,
    /// Milliseconds since 1970.
    pub modified: i64,
    pub info_name: Option<String>,
    pub info_authors: Option<String>,
    pub info_version: Option<String>,
    /// The script's game name: a PinMAME table's ROM, an original's name for its files.
    pub game_name: Option<String>,
    /// Whether the script drives PinMAME, and so needs its game name's ROM.
    pub pinmame: bool,
    /// The names its script may hand PinUP for its pack, the likeliest first.
    pub pup_names: Vec<String>,
    /// The script files its script loads.
    pub helpers: Vec<String>,
    /// The music its script plays, `*` standing for what the script works out as it plays.
    pub music: Vec<String>,
    /// The folders beside it its script names for FlexDMD's or UltraDMD's pictures.
    pub folders: Vec<String>,
    /// The names its script keeps its values under (`SaveValue`): its own high scores, among them.
    #[serde(default)]
    pub saved_under: Vec<String>,
    /// Whether its script makes PinUP's object, and so plays a PinUP pack.
    pub uses_pup: bool,
    /// A script beside the table (`<table>.vbs`), played in its own script's place, as VPX and the
    /// player do: its size and date, the script facts above read from it.
    pub script_beside: Option<(u64, i64)>,
    /// The one of VPX's eight global physics sets it plays, 1 to 8, where it plays one.
    #[serde(default)]
    pub physics_set: Option<u32>,
    /// Why it couldn't be read.
    pub error: Option<String>,
}

impl Read {
    pub fn clues(&self, stem: &str) -> Clues {
        Clues {
            file_name: stem.to_string(),
            info_name: self.info_name.clone(),
            info_authors: self.info_authors.clone(),
            info_version: self.info_version.clone(),
            made: Some(self.modified),
            rom: self.game_name.clone(),
        }
    }
}

/// The remembered reads, by each table file's path.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Remembered {
    pub version: u32,
    pub tables: HashMap<String, Read>,
}

/// Raised when what's remembered is read differently (2: scripts read without their comments;
/// 3: the names scripts hand PinUP; 4: a script beside the table read in its own's place; 5: the
/// script files a script loads; 6 and 7: its music and DMD folders, no script names in messages
/// nor object names taken for folders; 8: the names it keeps its values under; 9: Scorbit's folder
/// not taken for its DMD's; 10: whether it plays a PinUP pack; 11: a ROM table whose script loads
/// PinMAME naming no version; 12: the global physics set it plays).
const VERSION: u32 = 12;

impl Remembered {
    /// The default place: `Library\tables.json` in the player's data folder.
    pub fn default_path() -> Option<PathBuf> {
        Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("Library").join("tables.json"))
    }

    /// What's remembered at `path`; nothing when it's missing, unreadable or of another version.
    pub fn load(path: &Path) -> Remembered {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Remembered>(&t).ok())
            .filter(|r| r.version == VERSION)
            .unwrap_or_default()
    }

    /// Written beside its place first, then moved there, so a failed write leaves the old copy.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string(&Remembered { version: VERSION, tables: self.tables.clone() }).map_err(|e| e.to_string())?;
        let part = path.with_extension("json.part");
        std::fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
        std::fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

fn unix_ms(t: std::time::SystemTime) -> i64 {
    t.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// The folders beside a table that hold what it uses, never tables of their own: VPX 10.8.1's in a
/// table's own folder (`pinmame`, `altsound`, `serum`, `vni`, `pupvideos`, `music`, `user`, `cache`,
/// `autosave`), VPX's own `scripts`, and a FlexDMD's or UltraDMD's pictures.
fn holds_what_a_table_uses(name: &str) -> bool {
    let low = name.to_lowercase();
    ["pinmame", "altsound", "altcolor", "serum", "vni", "pupvideos", "music", "user", "cache", "autosave", "scripts"].contains(&low.as_str())
        || low.ends_with(".ultradmd")
        || low.ends_with(".flexdmd")
        || low.ends_with(" pictures")
}

fn vpx_files_in(folder: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx"))).collect()
}

/// The folders below the tables folder a table may keep its own files in, as VPX 10.8.1 reads a
/// table in a folder of its own (`Tables\<folder>\<table>.vpx`): one level down, never one of the
/// folders a table's files go in.
fn table_folders(folder: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !holds_what_a_table_uses(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect()
}

/// Every table file in `folder`, and in a folder of its own one level down (VPX 10.8.1's layout),
/// sorted by path.
pub fn table_files(folder: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::metadata(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut found = vpx_files_in(folder);
    for sub in table_folders(folder) {
        found.extend(vpx_files_in(&sub));
    }
    found.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    Ok(found)
}

/// A table's file as front ends' game lists name it: its path below the tables folder, a folder
/// of its own included (`Attack from Mars\afm.vpx`, Windows' separator), else its file's name.
pub fn listed_name(tables: &Path, table: &Path) -> String {
    match table.strip_prefix(tables) {
        Ok(rel) => rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("\\"),
        Err(_) => table.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
    }
}

/// Every table file in `folder` with what it says: remembered where its size and date are as
/// remembered, read again where not (`progress` told each read: how many done, of how many, which).
/// What's remembered is brought up to date, tables gone from the folder forgotten.
pub fn scan(folder: &Path, remembered: &mut Remembered, mut progress: impl FnMut(usize, usize, &Path)) -> Result<Vec<(PathBuf, Read)>, String> {
    let files = table_files(folder)?;
    let mut stale = Vec::new();
    let mut found = Vec::with_capacity(files.len());
    let stamp = |p: &Path| {
        let meta = std::fs::metadata(p).ok().filter(|m| m.is_file())?;
        Some((meta.len(), meta.modified().map_or(0, unix_ms)))
    };
    for path in files {
        let (size, modified) = stamp(&path).unwrap_or_default();
        let script_beside = stamp(&path.with_extension("vbs"));
        match remembered.tables.get(&key(&path)) {
            Some(r) if r.size == size && r.modified == modified && r.script_beside == script_beside => found.push((path, r.clone())),
            _ => {
                stale.push(found.len());
                found.push((path, Read { size, modified, script_beside, ..Default::default() }));
            }
        }
    }
    for (done, &i) in stale.iter().enumerate() {
        let (path, read) = &mut found[i];
        progress(done, stale.len(), path);
        match crate::table::read(path) {
            Ok(facts) => {
                read.info_name = facts.clues.info_name;
                read.info_authors = facts.clues.info_authors;
                read.info_version = facts.clues.info_version;
                read.game_name = facts.game_name;
                read.pinmame = facts.pinmame;
                read.pup_names = facts.pup_names;
                read.helpers = facts.helpers;
                read.music = facts.music;
                read.folders = facts.folders;
                read.saved_under = facts.saved_under;
                read.uses_pup = facts.uses_pup;
                read.physics_set = facts.physics_set;
            }
            Err(e) => read.error = Some(e),
        }
        // a script beside the table plays in its own's place: what it asks for counts
        if read.script_beside.is_some()
            && let Ok(bytes) = std::fs::read(path.with_extension("vbs"))
        {
            let s = crate::table::script_facts(&String::from_utf8_lossy(&bytes));
            read.game_name = s.game_name;
            read.pinmame = s.pinmame;
            read.pup_names = s.pup_names;
            read.helpers = s.helpers;
            read.music = s.music;
            read.folders = s.folders;
            read.saved_under = s.saved_under;
            read.uses_pup = s.uses_pup;
        }
    }
    remembered.tables = found.iter().map(|(p, r)| (key(p), r.clone())).collect();
    Ok(found)
}

/// A table's match on VPS, by IDs.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnVps {
    pub game: Option<String>,
    pub table_file: Option<String>,
    /// How it was found, in words.
    pub found: Option<&'static str>,
    /// Sure enough to take without asking.
    pub sure: bool,
}

pub fn match_on_vps(db: &Database, stem: &str, read: &Read) -> OnVps {
    let m = matcher::match_table(db, &read.clues(stem));
    match m.game {
        Some(game) => OnVps {
            game: Some(game.id.clone()),
            table_file: m.table_file.map(|t| t.id.clone()),
            found: Some(m.found.describe()),
            sure: m.found.sure() && !matches!(m.found, Found::Loosely),
        },
        None => OnVps::default(),
    }
}

/// The game and table file a match names.
pub fn of<'a>(db: &'a Database, on: &OnVps) -> (Option<&'a Game>, Option<&'a TableFile>) {
    let game = on.game.as_deref().and_then(|id| db.game(id));
    let file = game.and_then(|g| on.table_file.as_deref().and_then(|id| g.table_files.iter().find(|t| t.id == id)));
    (game, file)
}

/// VPinMAME's ROM aliases (`VPMAlias.txt`): a name a table asks for, and the set that's there.
#[derive(Clone, Debug, Default)]
pub struct Aliases(Vec<(String, String)>);

impl Aliases {
    pub fn load(vpinmame: &Path) -> Aliases {
        let Ok(text) = std::fs::read_to_string(vpinmame.join("VPMAlias.txt")) else { return Aliases::default() };
        // read as libpinmame reads its own: the first two words, split at commas and spaces,
        // and nothing after a `#`, `;` or `'`
        Aliases(
            text.lines()
                .filter_map(|line| {
                    let line = line.split(['#', ';', '\'']).next().unwrap_or("");
                    let mut words = line.split([',', ' ', '\t']).filter(|w| !w.is_empty());
                    Some((words.next()?.to_lowercase(), words.next()?.to_string()))
                })
                .collect(),
        )
    }

    /// The set a game name stands for.
    pub fn resolve(&self, game: &str) -> String {
        let low = game.to_lowercase();
        self.0.iter().find(|(from, _)| *from == low).map_or_else(|| game.to_string(), |(_, to)| to.clone())
    }
}

/// A game's colouring for its display.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Colour {
    /// Serum, PIN2DMD (.pac) or PIN2DMD.
    pub kind: &'static str,
    pub folder: PathBuf,
}

/// One of a front end's media files for a table: PinUP Popper's, PinballX's or PinballY's.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    /// Popper's own name for its kind, whichever front end's it is: Wheel, BackGlass, PlayField,
    /// Topper, DMD, GameInfo, GameHelp…
    pub kind: String,
    pub path: PathBuf,
    pub video: bool,
    /// The front end whose folder it's in.
    pub from: crate::frontends::Front,
}

/// What a table holds beside it, each where the player looks for it.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holds {
    /// The backglass file named as the table (`.directb2s`).
    pub backglass: Option<PathBuf>,
    /// Its other backglasses beside it, numbered as VPin Studio keeps a backglass's versions
    /// (`<table> (1).directb2s` and on): which one plays is chosen on its page (`backglass::play`).
    pub backglasses: Vec<PathBuf>,
    /// Pictures the player reads in place of its own, kept in `<table> pictures` beside it, each
    /// named as the picture it stands in for (`backdrop2.png`).
    pub pictures: Vec<PathBuf>,
    /// The ROM set its script asks for, after VPinMAME's aliases; only for tables that drive PinMAME.
    pub rom_set: Option<String>,
    /// That set's zip in VPinMAME's `roms`.
    pub rom: Option<PathBuf>,
    /// Its PinUP pack: in `pupvideos` beside the table, else in PinUP's shared `PUPVideos`.
    pub pup_pack: Option<PathBuf>,
    /// A pack for it that's there but named so the player won't find it (`misnamed_pack`).
    pub pup_misnamed: Option<PathBuf>,
    /// Its AltSound pack, in VPinMAME's `altsound`.
    pub altsound: Option<PathBuf>,
    /// Its colouring, in VPinMAME's `altcolor`.
    pub colour: Option<Colour>,
    /// Its views (`<table>.pov`).
    pub pov: Option<PathBuf>,
    /// Its physics, as VPX 5436 reads them before play: `<table>.vpp`, else `autovpp.vpp` beside it.
    pub physics: Option<PathBuf>,
    /// Its own settings (`<table>.ini`).
    pub settings: Option<PathBuf>,
    /// A script beside it, played in its own script's place (`<table>.vbs`).
    pub script: Option<PathBuf>,
    /// PinUP Popper's media for it.
    pub media: Vec<Media>,
    /// The music its script plays, found where the player looks: the `music` folder beside it, then
    /// VPX's.
    pub music: Vec<PathBuf>,
    /// The scripts it loads that aren't VPX's own, each where the player finds it.
    pub helpers: Vec<Helper>,
    /// The folders beside it its script names for FlexDMD's or UltraDMD's pictures and fonts, or
    /// named for its game (`kiss.UltraDMD`).
    pub folders: Vec<PathBuf>,
    /// Those its script names that aren't beside it.
    pub folders_missing: Vec<String>,
    /// Other files beside it named for it, which nothing reads: a second POV (`<table> (1).pov`),
    /// a read-me.
    pub beside: Vec<PathBuf>,
}

/// A script a table loads, and where it is: beside the table, else with VPX's scripts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Helper {
    pub name: String,
    pub path: Option<PathBuf>,
}

/// VPX's own scripts (VPX 5436's `scripts` folder), which every install has: not a table's own.
const VPX_SCRIPTS: [&str; 70] = [
    "6803.vbs", "ali.vbs", "alving.vbs", "antar.vbs", "atari.vbs", "atari1.vbs", "atari1b.vbs", "atari2.vbs", "b2b.vbs", "b2bcollision.vbs", "b2s.vbs", "bally.vbs", "ballyg.vbs", "capcom.vbs",
    "class1812.vbs", "controller.vbs", "core.vbs", "de.vbs", "de2.vbs", "fpvpx.vbs", "gameplan.vbs", "gts1.vbs", "gts3.vbs", "hankin.vbs", "idsa.vbs", "inder.vbs", "inder_atleta.vbs",
    "inder_centaur.vbs", "inder_skateboard.vbs", "ironballs.vbs", "joctronic.vbs", "juegos.vbs", "juegos2.vbs", "jvh.vbs", "lancelot.vbs", "ltd.vbs", "ltd3.vbs", "ltd4a.vbs", "ltdpecmen.vbs",
    "mac.vbs", "mrgame.vbs", "nuova.vbs", "pdb.vbs", "peyper.vbs", "play1.vbs", "play2.vbs", "play4.vbs", "playbar.vbs", "s11.vbs", "s4.vbs", "s6.vbs", "s7.vbs", "s8_stillcrazy.vbs", "sam.vbs",
    "sega.vbs", "sega2.vbs", "sleic.vbs", "solarwars.vbs", "spinball.vbs", "stern.vbs", "sys80.vbs", "taito.vbs", "ultradmd_options.vbs", "vpmkeys.vbs", "wpc.vbs", "wpc_ph.vbs", "zac.vbs", "zac1.vbs",
    "zac2.vbs", "zacproto.vbs",
];

/// Whether a script is one of VPX's own, which every install has.
pub fn is_vpx_script(name: &str) -> bool {
    VPX_SCRIPTS.contains(&name.to_lowercase().as_str())
}

/// The tables folder's own files and folders, by name, read once for every table's, and each
/// folder of tables' below it; and PinUP Popper's own names for its games, by their table files'
/// names in lower case, a folder of their own included (its media are named for them: VPin Studio
/// gives a game VPS's name, `Attack from Mars (Bally 1995)`, while its file keeps its own).
#[derive(Clone, Debug, Default)]
pub struct Around {
    pub files: Vec<String>,
    pub folders: Vec<String>,
    /// A table folder's files and folders below the tables folder, by its path in lower case.
    pub below: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    pub popper_names: std::collections::HashMap<String, String>,
    /// PinballX's and PinballY's media folders, where either plays the library's tables.
    pub fronts: Vec<crate::frontends::Found>,
    /// The ratings the front ends keep, by `ratings::key`.
    pub ratings: std::collections::HashMap<String, crate::ratings::Rating>,
}

impl Around {
    pub fn read(layout: &Layout) -> Around {
        let mut around = Around::default();
        if let Some(db) = crate::popper::database(layout)
            && let Ok(games) = crate::popper::games(&db, &layout.tables)
        {
            around.popper_names = games.into_iter().filter(|g| !g.name.trim().is_empty()).map(|g| (g.file.to_lowercase(), g.name)).collect();
        }
        around.fronts = crate::frontends::read_all(layout);
        around.ratings = crate::ratings::read_all(layout, &around.fronts);
        (around.files, around.folders) = listing(&layout.tables);
        for sub in table_folders(&layout.tables).into_iter().filter(|s| !vpx_files_in(s).is_empty()) {
            around.below.insert(sub.to_string_lossy().to_lowercase(), listing(&sub));
        }
        around
    }

    /// The files and folders beside a table in `dir`: the tables folder's, or its own folder's.
    pub fn beside(&self, dir: &Path) -> (&[String], &[String]) {
        match self.below.get(&dir.to_string_lossy().to_lowercase()) {
            Some((files, folders)) => (files, folders),
            None => (&self.files, &self.folders),
        }
    }
}

/// A folder's files and folders, by name.
fn listing(dir: &Path) -> (Vec<String>, Vec<String>) {
    let (mut files, mut folders) = (Vec::new(), Vec::new());
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        match e.file_type() {
            Ok(t) if t.is_dir() => folders.push(name),
            Ok(t) if t.is_file() => files.push(name),
            _ => {}
        }
    }
    (files, folders)
}

/// Whether `name` fits `pattern`, `*` standing for any run of letters, case aside.
fn fits(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.to_lowercase(), name.to_lowercase());
    let parts: Vec<&str> = p.split('*').collect();
    if parts.len() == 1 {
        return p == n;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !n.starts_with(first) || !n[first.len()..].ends_with(last) || n.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &n[first.len()..n.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    true
}

/// The files the music names (`*` standing for what the script works out) stand for, in the first
/// of `folders` that has any, as the player looks. (What none holds isn't said: the names a script
/// builds take in its PinUP pack's sounds too, which no music folder holds.)
fn music_in(names: &[String], folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for name in names {
        let (sub, file) = name.rsplit_once(['\\', '/']).unwrap_or(("", name));
        if sub.contains('*') || sub.split(['\\', '/']).any(|c| c == "..") {
            continue;
        }
        for folder in folders {
            let dir = sub.split(['\\', '/']).filter(|c| !c.is_empty()).fold(folder.clone(), |d, c| d.join(c));
            let mut here: Vec<PathBuf> = match file.contains('*') {
                false => existing(dir.join(file)).filter(|p| p.is_file()).into_iter().collect(),
                true => std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()) && fits(file, &e.file_name().to_string_lossy()))
                    .map(|e| e.path())
                    .collect(),
            };
            if !here.is_empty() {
                here.sort();
                for p in here {
                    if !found.contains(&p) {
                        found.push(p);
                    }
                }
                break;
            }
        }
    }
    found
}

/// The companion files' kinds a table's page shows in rows of their own.
const OWN: [&str; 6] = ["vpx", "directb2s", "pov", "vpp", "ini", "vbs"];

/// The number of a file named `<stem> (n)` (lower case both), as a second download is numbered and
/// VPin Studio numbers a backglass's versions.
pub fn number_after(stem: &str, name_stem: &str) -> Option<u32> {
    name_stem.strip_prefix(stem)?.strip_prefix(" (")?.strip_suffix(')').filter(|n| !n.is_empty()).and_then(|n| n.parse().ok())
}

/// Files beside the table named for it that nothing reads: `<stem>.<other kind>`, or a companion
/// numbered as a second download is (`<stem> (1).pov`) — but none another table's own, and no
/// backglass, numbered so as a version (`backglass_versions`).
fn beside_named(stem: &str, files: &[String], dir: &Path) -> Vec<PathBuf> {
    let low = stem.to_lowercase();
    let stems: Vec<String> = files.iter().filter_map(|f| f.to_lowercase().strip_suffix(".vpx").map(str::to_string)).collect();
    let mut out: Vec<PathBuf> = files
        .iter()
        .filter(|f| {
            let f = f.to_lowercase();
            let Some((name_stem, ext)) = f.rsplit_once('.') else { return false };
            if name_stem == low {
                return !OWN.contains(&ext);
            }
            // numbered: `<stem> (n)`, not a table of its own
            ext != "directb2s" && number_after(&low, name_stem).is_some() && !stems.iter().any(|s| s == name_stem)
        })
        .map(|f| dir.join(f))
        .collect();
    out.sort();
    out
}

/// A table's other backglasses beside it, `<stem> (n).directb2s` — none another table's own — by
/// their numbers.
fn backglass_versions(stem: &str, files: &[String], dir: &Path) -> Vec<PathBuf> {
    let low = stem.to_lowercase();
    let stems: Vec<String> = files.iter().filter_map(|f| f.to_lowercase().strip_suffix(".vpx").map(str::to_string)).collect();
    let mut found: Vec<(u32, PathBuf)> = files
        .iter()
        .filter_map(|f| {
            let name_stem = f.to_lowercase().strip_suffix(".directb2s")?.to_string();
            let n = number_after(&low, &name_stem)?;
            (!stems.contains(&name_stem)).then(|| (n, dir.join(f)))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

/// The pictures kept for a table in `<stem> pictures` beside it, which the player reads in place of
/// its own pictures of the same names.
pub fn table_pictures(dir: &Path, stem: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir.join(format!("{stem} pictures")))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| crate::analysis::PICTURE_SUFFIXES.contains(&e.to_string_lossy().to_lowercase().as_str())))
        .collect();
    out.sort();
    out
}

/// PinUP Popper's media for VPX's tables: `POPMedia\Visual Pinball X` beside PinUP's `PUPVideos`.
pub fn pop_media(layout: &Layout) -> Option<PathBuf> {
    let dir = layout.pup_videos.parent()?.join("POPMedia").join("Visual Pinball X");
    dir.is_dir().then_some(dir)
}

const PICTURES: [&str; 5] = ["png", "jpg", "jpeg", "apng", "gif"];
const VIDEOS: [&str; 4] = ["mp4", "m4v", "webm", "mov"];

/// Popper's media named for a table (`<stem>.png`, or `<stem>01.mp4` and on for more of a kind), in
/// each of its kinds' folders. `folders` holds each kind's folder and the names in it.
fn media_for(stem: &str, folders: &[(String, PathBuf, Vec<String>)]) -> Vec<Media> {
    let low = stem.to_lowercase();
    let mut found = Vec::new();
    for (kind, dir, names) in folders {
        let mut mine: Vec<&String> = names
            .iter()
            .filter(|n| {
                let n = n.to_lowercase();
                let Some((base, ext)) = n.rsplit_once('.') else { return false };
                let rest = base.strip_prefix(low.as_str());
                rest.is_some_and(|r| r.is_empty() || r.len() == 2 && r.bytes().all(|b| b.is_ascii_digit()))
                    && (PICTURES.contains(&ext) || VIDEOS.contains(&ext))
            })
            .collect();
        mine.sort_by_key(|n| n.to_lowercase());
        for name in mine {
            let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
            found.push(Media { kind: kind.clone(), path: dir.join(name), video: VIDEOS.contains(&ext.as_str()), from: crate::frontends::Front::Popper });
        }
    }
    found
}

/// Popper's kinds of media, each with the names in its folder: read once for the whole library.
pub fn media_folders(layout: &Layout) -> Vec<(String, PathBuf, Vec<String>)> {
    let Some(root) = pop_media(layout) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&root) else { return Vec::new() };
    let mut folders: Vec<(String, PathBuf, Vec<String>)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let dir = e.path();
            let names = std::fs::read_dir(&dir)
                .map(|d| d.filter_map(Result::ok).filter(|f| f.path().is_file()).map(|f| f.file_name().to_string_lossy().into_owned()).collect())
                .unwrap_or_default();
            (e.file_name().to_string_lossy().into_owned(), dir, names)
        })
        .collect();
    folders.sort_by(|a, b| a.0.cmp(&b.0));
    folders
}

/// A game's colouring folder's kind, by the files in it.
fn colour_kind(folder: &Path) -> Option<&'static str> {
    let names: Vec<String> = std::fs::read_dir(folder).ok()?.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_lowercase()).collect();
    if names.iter().any(|n| n.ends_with(".crz") || n.ends_with(".cromc")) {
        Some("Serum")
    } else if names.iter().any(|n| n.ends_with(".pac")) {
        Some("PIN2DMD (.pac)")
    } else if names.iter().any(|n| n.ends_with(".pal") || n.ends_with(".vni")) {
        Some("PIN2DMD")
    } else {
        None
    }
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}

/// A PinUP pack that's there for a table but named so the player won't find it: for a table whose script plays a pack and finds none under the names it hands
/// PinUP, a pack folder — one holding PinUP's `screens.pup` — beside the tables (`pupvideos`) or in
/// PinUP's `PUPVideos`, named for its ROM set after VPinMAME's aliases, for another set of the same
/// game (`afm_113` for `afm_113b`, `afm`), or for the table's own name (`Attack from Mars`), or for
/// one of those names written another way (`Willy_Wonka` for `Willy Wonka`), that
/// no other table finds as its own (`claimed`). Renamed to the first name its script hands PinUP,
/// the player finds it.
pub fn misnamed_pack(layout: &Layout, path: &Path, read: &Read, holds: &Holds, claimed: &std::collections::HashSet<PathBuf>) -> Option<PathBuf> {
    if !read.uses_pup || holds.pup_pack.is_some() || read.pup_names.is_empty() {
        return None;
    }
    let letters = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_lowercase();
    // a ROM set's game: the part before its version (`afm` of `afm_113b`)
    let family = |s: &str| s.split_once('_').map_or(s, |(f, _)| f).to_lowercase();
    let mut sets: Vec<String> = read.pup_names.iter().chain(read.game_name.iter()).chain(holds.rom_set.iter()).map(|n| n.to_lowercase()).collect();
    sets.dedup();
    let families: Vec<String> = sets.iter().filter(|n| n.contains('_')).map(|n| family(n)).collect();
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let own = letters(stem.split(" (").next().unwrap_or(&stem));
    let fits = |name: &str| {
        let low = name.to_lowercase();
        sets.contains(&low) || families.iter().any(|f| *f == family(&low)) || (own.len() >= 4 && letters(name) == own) || sets.iter().any(|s| s.len() >= 4 && letters(s) == letters(name))
    };
    let places = path.parent().map(|d| d.join("pupvideos")).into_iter().chain(std::iter::once(layout.pup_videos.clone()));
    places.flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten()).map(|e| e.path()).find(|p| {
        p.is_dir() && !claimed.contains(p) && p.file_name().is_some_and(|n| fits(&n.to_string_lossy())) && p.join("screens.pup").is_file()
    })
}

/// What the table at `path` holds, by what its script asks for (`read`); `around` is the tables
/// folder's files and folders.
pub fn holds(layout: &Layout, aliases: &Aliases, media: &[(String, PathBuf, Vec<String>)], around: &Around, path: &Path, read: &Read) -> Holds {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let beside = |ext: &str| existing(path.with_extension(ext));
    let rom_set = read.game_name.as_deref().filter(|_| read.pinmame).map(|g| aliases.resolve(g));
    let table_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    // its ROM, AltSound pack and colouring in its own folder first, as VPX 10.8.1's plugins look
    // (`pinmame\roms`, `altsound` or `pinmame\altsound`, `serum`, `vni` or `pinmame\altcolor`),
    // then VPinMAME's folders
    let rom = rom_set.as_deref().and_then(|r| {
        let zip = format!("{r}.zip");
        existing(table_dir.join("pinmame").join("roms").join(&zip)).or_else(|| existing(layout.vpinmame.join("roms").join(&zip)))
    });
    // a pack is named for what the script hands PinUP (its own constant, or the game it asks for,
    // before any alias: Metallica's mtl_180h runs as mtl_170h), beside the table first, as the
    // player looks
    let pup_pack = path.parent().and_then(|table_dir| {
        read.pup_names.iter().chain(read.game_name.iter()).find_map(|name| {
            [table_dir.join("pupvideos").join(name), layout.pup_videos.join(name)].into_iter().find(|p| p.is_dir())
        })
    });
    let altsound = rom_set.as_deref().and_then(|r| {
        [table_dir.join("altsound").join(r), table_dir.join("pinmame").join("altsound").join(r), layout.vpinmame.join("altsound").join(r)].into_iter().find(|p| p.is_dir())
    });
    let colour = rom_set.as_deref().and_then(|r| {
        [table_dir.join("serum").join(r), table_dir.join("vni").join(r), table_dir.join("pinmame").join("altcolor").join(r), layout.vpinmame.join("altcolor").join(r)]
            .into_iter()
            .find_map(|folder| colour_kind(&folder).map(|kind| Colour { kind, folder }))
    });
    let (beside_files, beside_folders) = around.beside(&table_dir);
    // where the player looks for music: the `music` folder beside the table, then VPX's
    let music_folders: Vec<PathBuf> = std::iter::once(table_dir.join("music")).chain(layout.tables.parent().map(|v| v.join("Music"))).collect();
    let helpers: Vec<Helper> = read
        .helpers
        .iter()
        .filter(|h| !VPX_SCRIPTS.contains(&h.to_lowercase().as_str()))
        .map(|h| Helper { name: h.clone(), path: existing(table_dir.join(h)).or_else(|| existing(layout.scripts.join(h))) })
        .collect();
    // what its own scripts name too (KISS's songs are in `KISSMusic.vbs`, its DMD's folder in
    // `KISSDMDV2.vbs`), read afresh: they're small, and they change without the table
    let (mut music_names, mut folder_names) = (read.music.clone(), read.folders.clone());
    for h in helpers.iter().filter_map(|h| h.path.as_ref()) {
        if let Ok(bytes) = std::fs::read(h) {
            let facts = crate::table::script_facts(&String::from_utf8_lossy(&bytes));
            music_names.extend(facts.music);
            folder_names.extend(facts.folders);
        }
    }
    // the folders the scripts name, and any named for its game
    let mut folders: Vec<PathBuf> = Vec::new();
    let named_for_game = |f: &String| {
        let low = f.to_lowercase();
        read.game_name.as_deref().is_some_and(|g| [".ultradmd", ".flexdmd"].iter().any(|x| low == format!("{}{x}", g.to_lowercase())))
    };
    for f in folder_names.iter().filter_map(|n| beside_folders.iter().find(|f| f.eq_ignore_ascii_case(n))).chain(beside_folders.iter().filter(|f| named_for_game(f))) {
        let p = table_dir.join(f);
        if !folders.contains(&p) {
            folders.push(p);
        }
    }
    let music = music_in(&music_names, &music_folders);
    // the folders its scripts name that aren't beside it (a name it works out as it plays, never)
    let folders_missing: Vec<String> = folder_names.iter().filter(|n| !n.contains('*') && !beside_folders.iter().any(|f| f.eq_ignore_ascii_case(n))).cloned().collect();
    Holds {
        backglass: beside("directb2s"),
        backglasses: backglass_versions(&stem, beside_files, &table_dir),
        // only where its folder is there: a folder each table rarely has
        pictures: if beside_folders.iter().any(|f| f.eq_ignore_ascii_case(&format!("{stem} pictures"))) { table_pictures(&table_dir, &stem) } else { Vec::new() },
        rom_set,
        rom,
        pup_pack,
        pup_misnamed: None,
        altsound,
        colour,
        pov: beside("pov"),
        physics: beside("vpp").or_else(|| existing(table_dir.join("autovpp.vpp"))),
        settings: beside("ini"),
        script: beside("vbs"),
        // Popper's named for its file's name, and for Popper's own name for it where that's
        // another; PinballX's and PinballY's for theirs
        media: {
            let mut found = media_for(&stem, media);
            let file = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            if let Some(name) = around.popper_names.get(&listed_name(&layout.tables, path).to_lowercase()).filter(|n| !n.eq_ignore_ascii_case(&stem)) {
                found.extend(media_for(name, media));
            }
            for front in &around.fronts {
                found.extend(front.media_for(&file));
            }
            found
        },
        music,
        helpers,
        folders,
        folders_missing,
        beside: beside_named(&stem, beside_files, &table_dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_as_libpinmame() {
        let a = Aliases(vec![("mtl_180h".into(), "mtl_170h".into())]);
        assert_eq!(a.resolve("MTL_180H"), "mtl_170h");
        assert_eq!(a.resolve("afm_113b"), "afm_113b");
    }

    #[test]
    fn media_named_for_a_table() {
        let names = ["Blood Machines 2.0.png", "Blood Machines 2.001.png", "Blood Machines 2.0 alt.png", "Blood Machines 2.0.txt", "Other.png"]
            .map(String::from)
            .to_vec();
        let folders = vec![("Wheel".to_string(), PathBuf::from(r"C:\m\Wheel"), names)];
        let found = media_for("Blood Machines 2.0", &folders);
        let got: Vec<String> = found.iter().map(|m| m.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(got, ["Blood Machines 2.0.png", "Blood Machines 2.001.png"]);
    }

    #[test]
    fn names_with_a_star() {
        assert!(fits("bgout_T&JbackgroundMusic*.mp3", "bgout_T&JbackgroundMusic0.MP3"));
        assert!(!fits("bgout_T&JbackgroundMusic*.mp3", "bgout_T&Jmultiballmadness.mp3"));
        assert!(fits("TNA*.mp3", "TNA12.mp3"));
        assert!(fits("a*b*c.ogg", "a1b2c.ogg"));
        assert!(!fits("a*b*c.ogg", "a1c.ogg"));
        assert!(!fits("ab*ba", "aba"));
    }

    #[test]
    fn music_where_the_player_looks() {
        let root = std::env::temp_dir().join(format!("vpi-music-test-{}", std::process::id()));
        let (beside, vpx) = (root.join("Tables").join("music"), root.join("Music"));
        std::fs::create_dir_all(beside.join("TNA")).unwrap();
        std::fs::create_dir_all(&vpx).unwrap();
        for f in ["bgout_T&JbackgroundMusic0.mp3", "bgout_T&JbackgroundMusic1.mp3", "other.mp3", "TNA/TNA1.mp3"] {
            std::fs::write(beside.join(f), b"").unwrap();
        }
        std::fs::write(vpx.join("bgout_T&Jmultiballmadness.mp3"), b"").unwrap();
        let names = ["bgout_T&JbackgroundMusic*.mp3", "bgout_T&Jmultiballmadness.mp3", r"TNA\TNA*.mp3", "missing.mp3", r"..\escape.mp3"].map(String::from);
        let found = music_in(&names, &[beside.clone(), vpx.clone()]);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(found, [beside.join("bgout_T&JbackgroundMusic0.mp3"), beside.join("bgout_T&JbackgroundMusic1.mp3"), vpx.join("bgout_T&Jmultiballmadness.mp3"), beside.join("TNA").join("TNA1.mp3")]);
    }

    #[test]
    fn files_beside_named_for_it() {
        let around = Around {
            files: ["SpongeBob 2.1.vpx", "SpongeBob 2.1.directb2s", "SpongeBob 2.1 (1).directb2s", "SpongeBob 2.1.txt", "SpongeBob 2.1 (2).vpx", "SpongeBob 2.1 (2).directb2s", "Other.txt"]
                .map(String::from)
                .to_vec(),
            folders: Vec::new(),
            ..Default::default()
        };
        let dir = PathBuf::from(r"C:\t");
        // a read-me; a second backglass is a version of its backglass; the numbered table's own
        // backglass is its own
        assert_eq!(beside_named("SpongeBob 2.1", &around.files, &dir), [dir.join("SpongeBob 2.1.txt")]);
        assert_eq!(backglass_versions("SpongeBob 2.1", &around.files, &dir), [dir.join("SpongeBob 2.1 (1).directb2s")]);
    }

    #[test]
    fn a_table_in_a_folder_of_its_own_read_as_vpx_reads_it() {
        let root = std::env::temp_dir().join(format!("vpi-own-folder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::under(&root);
        let touch = |p: &Path| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        };
        // one table beside the others, one in a folder of its own with its ROM, AltSound pack,
        // Serum colouring, PinUP pack, music and backglass inside it; a pack's own folder holding a
        // file named `.vpx` is no table
        let shared = layout.tables.join("Taxi.vpx");
        touch(&shared);
        let own_dir = layout.tables.join("Attack from Mars (Bally 1995)");
        let own = own_dir.join("afm VPW.vpx");
        touch(&own);
        touch(&own_dir.join("afm VPW.directb2s"));
        touch(&own_dir.join("afm VPW (1).directb2s"));
        touch(&own_dir.join("pinmame").join("roms").join("afm_113b.zip"));
        touch(&own_dir.join("altsound").join("afm_113b").join("altsound.csv"));
        touch(&own_dir.join("serum").join("afm_113b").join("afm_113b.cRZ"));
        touch(&own_dir.join("pupvideos").join("afm_113b").join("screens.pup"));
        touch(&own_dir.join("pupvideos").join("afm_113b").join("odd.vpx"));
        touch(&own_dir.join("music").join("theme.mp3"));
        touch(&layout.tables.join("pupvideos").join("not a table.vpx"));
        let files = table_files(&layout.tables).unwrap();
        assert_eq!(files, [own.clone(), shared.clone()]);
        assert_eq!(listed_name(&layout.tables, &own), "Attack from Mars (Bally 1995)\\afm VPW.vpx");
        assert_eq!(listed_name(&layout.tables, &shared), "Taxi.vpx");
        let around = Around::read(&layout);
        let read = Read { pinmame: true, game_name: Some("afm_113b".into()), uses_pup: true, music: vec!["theme.mp3".into()], ..Default::default() };
        let h = holds(&layout, &Aliases::default(), &[], &around, &own, &read);
        let got = (h.rom.clone(), h.altsound.clone(), h.colour.as_ref().map(|c| (c.kind, c.folder.clone())), h.pup_pack.clone(), h.backglass.clone(), h.backglasses.clone(), h.music.len());
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(got.0, Some(own_dir.join("pinmame").join("roms").join("afm_113b.zip")));
        assert_eq!(got.1, Some(own_dir.join("altsound").join("afm_113b")));
        assert_eq!(got.2, Some(("Serum", own_dir.join("serum").join("afm_113b"))));
        assert_eq!(got.3, Some(own_dir.join("pupvideos").join("afm_113b")));
        assert_eq!(got.4, Some(own_dir.join("afm VPW.directb2s")));
        assert_eq!(got.5, [own_dir.join("afm VPW (1).directb2s")]);
        assert_eq!(got.6, 1);
    }

    #[test]
    fn a_pack_named_so_the_player_wont_find_it() {
        let root = std::env::temp_dir().join(format!("vpi-misnamed-{}", std::process::id()));
        let layout = Layout::under(&root);
        let pack = |name: &str| {
            let dir = layout.pup_videos.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("screens.pup"), "").unwrap();
            dir
        };
        let afm = pack("afm_113");
        let claimed_tz = pack("tz_94ch");
        std::fs::create_dir_all(layout.pup_videos.join("afm_notapack")).unwrap();
        let table = layout.tables.join("Attack from Mars (Bally 1995).vpx");
        let read = Read { uses_pup: true, pup_names: vec!["afm_113b".into()], game_name: Some("afm_113b".into()), ..Default::default() };
        let holds = Holds { rom_set: Some("afm_113b".into()), ..Default::default() };
        let claimed: std::collections::HashSet<PathBuf> = [claimed_tz.clone()].into();
        let found = misnamed_pack(&layout, &table, &read, &holds, &claimed);
        // another table's own pack is never offered; a script that plays no pack is never asked about
        let tz = Read { uses_pup: true, pup_names: vec!["tz_94h".into()], ..Default::default() };
        let tz_found = misnamed_pack(&layout, &layout.tables.join("Twilight Zone.vpx"), &tz, &Holds::default(), &claimed);
        let no_pup = misnamed_pack(&layout, &table, &Read { uses_pup: false, ..read.clone() }, &holds, &claimed);
        // the name its script asks for written another way
        let wonka = pack("Willy_Wonka");
        let ww = Read { uses_pup: true, pup_names: vec!["Willy Wonka".into()], ..Default::default() };
        let ww_found = misnamed_pack(&layout, &layout.tables.join("Willy Wonka & the Chocolate Factory (Original 2020).vpx"), &ww, &Holds::default(), &claimed);
        // renamed, it's where the player looks
        let renamed = crate::install::rename_pack(&afm, "afm_113b").unwrap();
        let there = renamed.join("screens.pup").is_file();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(found, Some(afm));
        assert_eq!((tz_found, no_pup), (None, None));
        assert_eq!(ww_found, Some(wonka));
        assert!(there && renamed.ends_with("afm_113b"));
    }
}
