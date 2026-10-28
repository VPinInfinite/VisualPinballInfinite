//! The library as the app shows it: the tables installed, each with VPS's facts and
//! pictures and what it holds (`library::installed`), and the user's own — favourites, when each
//! table was last played, the screens' preferences — read on a thread of its own at start and told
//! to the screens as it goes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use library::installed::{self, Holds, Media, OnVps, Read, Remembered};
use library::plan::Layout;
use library::vps::{self, Asset, Database, Game, TableFile};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{added, pictures};

/// The app's state, shared by its commands, its picture protocol and its library thread.
#[derive(Default)]
pub struct Shared(pub Mutex<State>);

#[derive(Default)]
pub struct State {
    pub status: Status,
    pub layout: Option<Layout>,
    pub db: Option<Arc<Database>>,
    /// VPS's pictures, by the ID of the game, table file or backglass file they show.
    pub vps_pictures: HashMap<String, String>,
    pub tables: Vec<Table>,
    pub user: User,
    /// The table playing, by its ID.
    pub playing: Option<String>,
    /// Each installed ROM set's check, by set in lower case: how it looks (good, warn, bad), and
    /// what it says (`rom_checks`).
    pub rom_checks: HashMap<String, (String, String)>,
    /// Each installed AltSound pack's check, by its folder in lower case: how it looks and what it
    /// says (`library::altsound`), made as the library's read.
    pub alt_checks: HashMap<String, (String, String)>,
    /// The AltSound packs made from their ROMs by rom2altsound, by folder in lower case.
    pub alt_made: std::collections::HashSet<String>,
    /// The tables whose own script changed since their script was edited here, by table file's
    /// path (lower case): told on their pages, to carry the changes over or not.
    pub script_updates: std::collections::HashSet<String>,
    /// Each table's PinUP pack's check and the layout in place, by its folder in lower case
    /// (`pup_packs`), made as the library's read and after a layout's applied.
    pub pup_checks: HashMap<String, crate::pup_packs::Facts>,
    /// Each table's music for cabinet mode, by its table file (lower case): `tunes::of`.
    pub tunes: HashMap<String, PathBuf>,
    /// PinUP Popper's own names for its games, by their table files' names in lower case: its media
    /// are named for them.
    pub popper_names: HashMap<String, String>,
    /// PinballX's and PinballY's media folders and their names for the tables, where either plays
    /// the library's tables.
    pub fronts: Vec<::library::frontends::Found>,
    /// The ratings the front ends keep for the tables, by `library::ratings::key`.
    pub front_ratings: HashMap<String, ::library::ratings::Rating>,
    /// PinUP Popper's list kept since the app started, before ratings were first put in it.
    pub popper_kept_for_ratings: bool,
}

/// The front end the user chose for a drop's media (`prefs.mediaFrontEnd`), if any.
pub fn media_front(prefs: &serde_json::Value) -> Option<::library::frontends::Front> {
    prefs.get("mediaFrontEnd").and_then(serde_json::Value::as_str).and_then(::library::frontends::Front::from_key)
}

/// How far the library's reading has come, for the screens.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// starting, vps (fetching VPS's database), reading, ready
    pub phase: &'static str,
    pub done: usize,
    pub of: usize,
    pub current: Option<String>,
    /// What went wrong, in words.
    pub problem: Option<String>,
    /// When the VPS database in use was fetched, in ms since 1970.
    pub vps_fetched: Option<i64>,
}

/// A table installed.
pub struct Table {
    pub id: String,
    pub path: PathBuf,
    pub stem: String,
    pub read: Read,
    pub on_vps: OnVps,
    pub holds: Holds,
    /// When its file came onto this PC (made in its folder), in ms since 1970.
    pub added: i64,
    /// What the drop window added for it that none of its page's other rows shows, still there.
    pub other: Vec<added::File>,
}

/// The user's own: kept in `Library\library.json` in the player's data folder.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    pub version: u32,
    /// The favourites, by their table files' paths (lower case).
    pub favourites: BTreeSet<String>,
    pub played: BTreeMap<String, Played>,
    /// The user's word on a table's VPS match, where the matcher could only guess.
    pub vps: BTreeMap<String, VpsChoice>,
    /// The screens' own preferences (picture shown, tile size, view, sort), as they keep them.
    pub prefs: serde_json::Value,
    /// Each table's last test, by its table file's path (lower case).
    pub tests: BTreeMap<String, Tested>,
    /// VPS's pictures the user hid from a table's page, by table file's path (lower case): VPS's
    /// picture IDs (a table file's, a backglass's, a game's).
    pub hidden: BTreeMap<String, BTreeSet<String>>,
    /// The ROMs changed on a table's page, by table file's path (lower case): what was done, to go
    /// back on it.
    pub rom_changes: BTreeMap<String, ::library::rom_change::Changed>,
    /// What was changed in a table's script in the app's script editor, by table file's path (lower
    /// case), to carry its options over when the table's own script changes.
    pub script_edits: BTreeMap<String, crate::script_editor::ScriptEdit>,
    /// The user's own playlists, in their order (managed in Settings › Library).
    pub playlists: Vec<Playlist>,
    /// The picture or video of each kind used for a table where it has several — VPS's, PinUP
    /// Popper's, ours: by table file's
    /// path (lower case), then kind (`playfield`, `wheel`, `playfield-video`…), its `MediaView::pick`.
    pub chosen: BTreeMap<String, BTreeMap<String, String>>,
    /// The tables renamed to VPS's names for them ("Overwrite for all players"), by their table
    /// files' new paths (lower case): what was done, to put back.
    pub renames: BTreeMap<String, ::library::rename::Renamed>,
    /// The pictures and videos put in PinUP Popper's folders from VPS's or ours, by table file's path
    /// (lower case), then the source's `MediaView::pick`: the Popper file standing in its place
    /// (lower case), which the table's page shows instead of the two.
    pub in_popper: BTreeMap<String, BTreeMap<String, String>>,
    /// A table's details as the user set them over VPS's and its file's ("Edit its details"), by
    /// table file's path (lower case).
    pub details: BTreeMap<String, Details>,
    /// The tables hidden from the library, by their table files' paths (lower case): shown only in
    /// the Hidden smart playlist.
    pub hidden_tables: BTreeSet<String>,
    /// The layout last chosen for each PinUP pack, by its folder (lower case): its option file,
    /// told apart where two layouts' files are the same.
    pub pup_layouts: BTreeMap<String, String>,
    /// The PinUP packs renamed here for a table to play them, by their folders now (lower case):
    /// each one's name before, to put back.
    pub pup_renames: BTreeMap<String, String>,
    /// How a picture or video is turned, by the user's word (a front end's kept on its side the
    /// other way, or any turned by quarters): by table, then by its `MediaView::pick`, 0 to 270.
    pub turns: BTreeMap<String, BTreeMap<String, u16>>,
    /// The library moved into a folder for each game: what was done, to put back.
    pub switched: Option<::library::layout_switch::Switched>,
    /// The user's own ratings, one to five stars, by table file's path (lower case); 0 one cleared
    /// here, over a front end's. One the front ends' lists took is theirs to keep, not ours.
    pub ratings: BTreeMap<String, u8>,
    /// The ratings a front end's list didn't take when they were set (it was running, or its list
    /// couldn't be changed), by table file's path (lower case): put in when the library's read again.
    pub ratings_unsent: BTreeSet<String>,
}

/// A table's details as the user set them ("Edit its details"): each one the app shows in place of
/// VPS's and the table file's, those left alone left out. Kept in VPI's library only: the table's file and other programs keep theirs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Details {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Its type, as VPS's badges name them: SS, EM, PM, DG or Original.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
}

impl Details {
    /// Each one trimmed, those left empty or unknown left out.
    fn tidied(self) -> Details {
        let text = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && !s.contains(['\r', '\n']));
        let authors: Vec<String> = self.authors.unwrap_or_default().into_iter().filter_map(|a| text(Some(a))).collect();
        Details {
            name: text(self.name),
            maker: text(self.maker),
            year: self.year.filter(|y| (1800..=2200).contains(y)),
            badge: self.badge.filter(|b| kind_named(b).is_some()),
            version: text(self.version),
            authors: (!authors.is_empty()).then_some(authors),
        }
    }
}

/// A type's name from its badge, as `kind_of` names VPS's.
fn kind_named(badge: &str) -> Option<String> {
    Some(
        match badge {
            "SS" => "Solid state",
            "EM" => "Electro-mechanical",
            "PM" => "Pure mechanical",
            "DG" => "Digital",
            "Original" => "Original",
            _ => return None,
        }
        .to_string(),
    )
}

/// A table's details kept as the user set them, or VPS's again for an empty set.
pub fn set_details(st: &mut State, id: &str, details: Details) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    let details = details.tidied();
    if details == Details::default() {
        st.user.details.remove(&key);
    } else {
        st.user.details.insert(key, details);
    }
    st.user.save()
}

/// One of the user's own playlists: its tables by their table files' paths (lower case), in their order.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub tables: Vec<String>,
}

/// A table's last test: "Test it" on its page, or the full test of a new table.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Tested {
    /// When, in ms since 1970.
    pub at: i64,
    pub report: ::library::test::Report,
}

/// The table at `path` tested with no window, as the compatibility dashboard tests it.
pub fn test_path(path: &Path) -> Result<Tested, String> {
    let runner = ::library::test::runner().ok_or("The headless runner isn't beside the app, so tables can't be tested.")?;
    Ok(Tested { at: now_ms(), report: ::library::test::run(&runner, path) })
}

/// A test's findings kept with the user's own; in the test mode, for the page alone.
pub fn remember_test(app: &AppHandle, path: &Path, tested: &Tested) {
    let testing = app.state::<Option<crate::shot::Shot>>().is_some();
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    st.user.tests.insert(key(path), tested.clone());
    if !testing && let Err(e) = st.user.save() {
        eprintln!("the test wasn't kept: {e}");
    }
}

/// "Test it": the library's table `id` tested, its findings kept.
pub fn test_table(app: &AppHandle, id: &str) -> Result<Tested, String> {
    let path = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        st.tables.iter().find(|t| t.id == id).map(|t| t.path.clone()).ok_or("That table isn't in the library any more.")?
    };
    before_a_run(app, id);
    let tested = test_path(&path)?;
    remember_test(app, &path, &tested);
    let _ = app.emit("library-changed", ());
    Ok(tested)
}

/// Before a table runs in our player for anything but play — its media made, a test — VPX's copy of
/// what it keeps taken as ours to start from where ours has none, as PLAY takes it, so a run of ours
/// never stands in the way of VPX's high scores carrying on (`scores::start_from_vpx`). Whether its
/// machine has never run here even so — no memory of ours, none of VPX's to start from — its first
/// start, which restores its factory settings, still to come.
pub fn before_a_run(app: &AppHandle, id: &str) -> bool {
    let (layout, rom, saved_under) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let Some(t) = st.tables.iter().find(|t| t.id == id) else { return false };
        (st.layout.clone(), t.holds.rom_set.clone(), t.read.saved_under.clone())
    };
    if let Some(layout) = &layout
        && let Err(e) = ::library::scores::start_from_vpx(layout, rom.as_deref(), &saved_under)
    {
        eprintln!("VPX's copy of what {id} keeps couldn't be taken: {e}");
    }
    // the player keeps its machines' memory in the user's own data folder, whatever the layout
    rom.is_some_and(|rom| data_folder().is_some_and(|d| !d.join("PinMAME").join("nvram").join(format!("{rom}.nv")).exists()))
}

/// A table's game and table file on VPS, as the user chose them; no game: not on VPS.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VpsChoice {
    pub game: Option<String>,
    pub table_file: Option<String>,
}

impl VpsChoice {
    fn on_vps(&self) -> OnVps {
        OnVps { game: self.game.clone(), table_file: self.table_file.clone(), found: Some("as you chose"), sure: true }
    }
}

/// A table's match: the user's choice where there is one, else the matcher's.
fn match_of(db: Option<&Database>, user: &User, path: &Path, stem: &str, read: &Read) -> OnVps {
    if let Some(choice) = user.vps.get(&key(path)) {
        return choice.on_vps();
    }
    db.map(|db| installed::match_on_vps(db, stem, read)).unwrap_or_default()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Played {
    /// When last played, in ms since 1970.
    pub last: i64,
    pub count: u32,
    /// How long it's been played in all, in seconds.
    pub seconds: u64,
    /// Each play, the newest last (when it was played and the score), the
    /// latest `PLAY_LOG` kept.
    pub log: Vec<Play>,
}

/// One play: from PLAY until the player closed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Play {
    /// When it started, in ms since 1970.
    pub at: i64,
    pub seconds: u64,
    /// The last game's scores, each player's, where they're known: from the machine's memory (a ROM
    /// table's, where its map says where they are), else as the script last showed them on its
    /// backglass (`B2SSetScorePlayer`).
    pub scores: Vec<u64>,
}

/// The plays kept for each table.
const PLAY_LOG: usize = 200;

pub fn data_folder() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite"))
}

/// Whether the app's window opens full screen: it was left so (the preferences' `fullscreen`).
pub fn starts_fullscreen() -> bool {
    User::load().prefs.get("fullscreen").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

impl User {
    fn path() -> Option<PathBuf> {
        Some(data_folder()?.join("Library").join("library.json"))
    }

    fn load() -> User {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written beside its place first, then moved there, so a failed write leaves the old copy.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("no APPDATA folder")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let part = path.with_extension("json.part");
        std::fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
        std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

pub fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// A table's ID for the screens: FNV-1a of its path, lower case.
pub fn id_of(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key(path).bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn modified_ms(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

fn set_status(app: &AppHandle, change: impl FnOnce(&mut Status)) {
    let status = {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        change(&mut st.status);
        st.status.clone()
    };
    let _ = app.emit("library-status", status);
}

/// Reads the library on a thread of its own, telling the screens as it goes.
pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("library".into())
        .spawn(move || {
            if let Err(e) = load(&app) {
                set_status(&app, |s| {
                    s.phase = "ready";
                    s.problem = Some(e);
                });
                let _ = app.emit("library-changed", ());
            }
        })
        .expect("the library's thread");
}

fn load(app: &AppHandle) -> Result<(), String> {
    // the library the settings name, as first setup keeps it; none named, first setup
    // shows (the test mode keeps the reference install's, for its pictures); `VPI_LAYOUT=<folder>`,
    // VPX's layout in that folder instead, for checks, the library's reads remembered there too
    let scratch = std::env::var_os("VPI_LAYOUT").map(PathBuf::from);
    let layout = match scratch.as_deref() {
        // `VPI_LAYOUT_PER_GAME` set: kept in a folder for each game
        Some(root) => Layout { per_game: std::env::var_os("VPI_LAYOUT_PER_GAME").is_some(), ..Layout::under(root) },
        None => match vpi_settings::library_folders_now() {
            Some(f) => Layout::from_folders(&f),
            None if app.state::<Option<crate::shot::Shot>>().inner().is_some() => Layout::reference(),
            None => {
                set_status(app, |s| s.phase = "setup");
                let _ = app.emit("library-changed", ());
                return Ok(());
            }
        },
    };
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        st.user = User::load();
        let mut layout = layout.clone();
        layout.media_front = media_front(&st.user.prefs);
        st.layout = Some(layout);
        st.status.phase = "starting";
    }
    let vps_path = vps::default_path().ok_or("no APPDATA folder")?;
    let mut problem = None;
    if !vps_path.exists() {
        set_status(app, |s| s.phase = "vps");
        if let Err(e) = vps::fetch(&vps_path) {
            problem = Some(format!("VPS's database couldn't be fetched, so tables show without its facts and pictures ({e})."));
        }
    }
    let db = Database::load(&vps_path).ok().map(Arc::new);
    let fetched = modified_ms(&vps_path);

    let remembered_path = match &scratch {
        Some(root) => root.join("tables.json"),
        None => Remembered::default_path().ok_or("no APPDATA folder")?,
    };
    let mut remembered = Remembered::load(&remembered_path);
    let found = installed::scan(&layout.tables, &mut remembered, |done, of, path| {
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        set_status(app, |s| {
            s.phase = "reading";
            s.done = done;
            s.of = of;
            s.current = name;
        });
    })?;
    if let Err(e) = remembered.save(&remembered_path) {
        eprintln!("the library's reads weren't kept: {e}");
    }
    let (tables, chosen) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        (build(&layout, db.as_deref(), &st.user, &added::load(), found), st.user.pup_layouts.clone())
    };
    // each table's music, for cabinet mode
    let tunes: HashMap<String, PathBuf> = tables.iter().filter_map(|t| Some((key(&t.path), crate::tunes::of(&t.holds)?))).collect();
    // each table's PinUP pack checked, its layouts read, before the library's held
    let pup_checks = tables
        .iter()
        .filter_map(|t| t.holds.pup_pack.as_ref())
        .map(|p| {
            let k = key(p);
            let facts = crate::pup_packs::facts(p, chosen.get(&k).map(String::as_str));
            (k, facts)
        })
        .collect();
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        st.vps_pictures = db.as_deref().map(vps_pictures).unwrap_or_default();
        st.db = db;
        // each AltSound pack checked, as libaltsound reads it
        st.alt_checks = tables.iter().filter_map(|t| t.holds.altsound.as_ref()).map(|p| (key(p), ::library::altsound::check(p))).map(|(k, (look, text))| (k, (look.to_string(), text))).collect();
        st.alt_made = tables.iter().filter_map(|t| t.holds.altsound.as_ref()).filter(|p| crate::altsound_make::made_from_rom(p)).map(|p| key(p)).collect();
        // the tables updated in place since their scripts were edited here
        st.script_updates = tables
            .iter()
            .filter(|t| st.user.script_edits.get(&key(&t.path)).is_some_and(|e| crate::script_editor::beside(&t.path).is_file() && crate::script_editor::updated(&t.path, e)))
            .map(|t| key(&t.path))
            .collect();
        st.pup_checks = pup_checks;
        st.tunes = tunes;
        st.tables = tables;
        let around = ::library::installed::Around::read(&layout);
        st.popper_names = around.popper_names;
        st.fronts = around.fronts;
        st.front_ratings = around.ratings;
        st.status = Status { phase: "ready", problem, vps_fetched: fetched, ..Default::default() };
        // ratings a front end didn't take when they were set, put in now while the user wants that
        if ratings_to_fronts(&st.user.prefs) && !st.user.ratings_unsent.is_empty() {
            let keys: Vec<String> = st.user.ratings_unsent.iter().cloned().collect();
            let problems = send_ratings(&mut st, &keys);
            for p in problems {
                eprintln!("a rating wasn't put in a front end's list: {p}");
            }
        }
    }
    // the library's ratings and tables for VPIDB, where they're shared
    crate::vpidb::library_changed(app);
    note_popper_copies(app);
    set_status(app, |_| {});
    let _ = app.emit("library-changed", ());
    // every table's ROM checked, those checked before shown at once
    crate::rom_checks::start(app);

    // a day-old database fetched afresh, the tables matched again with it
    if vps::is_stale(&vps_path)
        && vps::fetch(&vps_path).is_ok()
        && let Ok(fresh) = Database::load(&vps_path)
    {
        let fresh = Arc::new(fresh);
        {
            let shared = app.state::<Shared>();
            let mut guard = shared.0.lock().unwrap();
            let st = &mut *guard;
            for t in &mut st.tables {
                t.on_vps = match_of(Some(&fresh), &st.user, &t.path, &t.stem, &t.read);
            }
            st.vps_pictures = vps_pictures(&fresh);
            st.db = Some(fresh);
            st.status.vps_fetched = modified_ms(&vps_path);
        }
        set_status(app, |_| {});
        let _ = app.emit("library-changed", ());
    }
    Ok(())
}

fn build(layout: &Layout, db: Option<&Database>, user: &User, record: &added::Added, found: Vec<(PathBuf, Read)>) -> Vec<Table> {
    let aliases = installed::Aliases::load(&layout.vpinmame);
    let media = installed::media_folders(layout);
    let around = installed::Around::read(layout);
    let mut tables: Vec<Table> = found
        .into_iter()
        .map(|(path, read)| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let on_vps = match_of(db, user, &path, &stem, &read);
            let holds = installed::holds(layout, &aliases, &media, &around, &path, &read);
            let added = std::fs::metadata(&path)
                .and_then(|m| m.created())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(read.modified, |d| d.as_millis() as i64);
            let other = record.tables.get(&key(&path)).map(|files| not_shown(&holds, &path, files)).unwrap_or_default();
            Table { id: id_of(&path), path, stem, read, on_vps, holds, added, other }
        })
        .collect();
    // a pack there for a table but named so the player won't find it, that no other table finds
    let claimed: std::collections::HashSet<PathBuf> = tables.iter().filter_map(|t| t.holds.pup_pack.clone()).collect();
    for t in &mut tables {
        t.holds.pup_misnamed = installed::misnamed_pack(layout, &t.path, &t.read, &t.holds, &claimed);
    }
    tables
}

/// What the drop window added for a table that none of its page's other rows shows — not the table,
/// its companions, ROM, packs, colouring, music, scripts, folders or media — and is still there.
fn not_shown(holds: &Holds, table: &Path, files: &[added::File]) -> Vec<added::File> {
    let norm = |p: &Path| key(p).replace('/', "\\");
    let mut singles: Vec<String> = [Some(table), holds.backglass.as_deref(), holds.rom.as_deref(), holds.pov.as_deref(), holds.physics.as_deref(), holds.settings.as_deref(), holds.script.as_deref()]
        .into_iter()
        .flatten()
        .map(norm)
        .collect();
    singles.extend(
        holds.music.iter().chain(&holds.beside).chain(&holds.backglasses).chain(&holds.pictures).chain(holds.media.iter().map(|m| &m.path)).chain(holds.helpers.iter().filter_map(|h| h.path.as_ref())).map(|p| norm(p)),
    );
    let folders: Vec<String> = [holds.pup_pack.as_deref(), holds.altsound.as_deref(), holds.colour.as_ref().map(|c| c.folder.as_path())]
        .into_iter()
        .flatten()
        .chain(holds.folders.iter().map(PathBuf::as_path))
        .map(|d| norm(d) + "\\")
        .collect();
    files
        .iter()
        .filter(|f| {
            let p = norm(&f.path);
            !singles.contains(&p) && !folders.iter().any(|d| p.starts_with(d)) && f.path.exists()
        })
        .cloned()
        .collect()
}

/// Every picture VPS's database names, by the ID of what it shows.
fn vps_pictures(db: &Database) -> HashMap<String, String> {
    let mut all = HashMap::new();
    for g in &db.games {
        if let Some(url) = &g.img_url {
            all.insert(g.id.clone(), url.clone());
        }
        for t in &g.table_files {
            if let Some(url) = &t.img_url {
                all.insert(t.id.clone(), url.clone());
            }
        }
        for b in &g.b2s_files {
            if let Some(url) = &b.img_url {
                all.insert(b.id.clone(), url.clone());
            }
        }
    }
    all
}

/// The kinds of a table's media, in the order its page shows them, each in plain words: PinUP Popper's
/// screens', a video a kind of its own.
const MEDIA_KINDS: [(&str, &str); 15] = [
    ("playfield", "Playfield"),
    ("playfield-video", "Playfield video"),
    ("backglass", "Backglass"),
    ("backglass-video", "Backglass video"),
    ("dmd", "Score display"),
    ("dmd-video", "Score display video"),
    ("topper", "Topper"),
    ("topper-video", "Topper video"),
    ("wheel", "Wheel"),
    ("gameinfo", "Flyer"),
    ("gamehelp", "Instruction card"),
    ("loading", "Loading"),
    ("menu", "Menu"),
    ("gameselect", "Game select"),
    ("other", "Other"),
];

/// The front ends whose folders media are shown from, as `MediaView::from` names them.
const FRONT_ENDS: [&str; 3] = ["Popper", "PinballX", "PinballY"];

/// Ours, as our player made them, and the kind each shows as.
const OWN_KINDS: [(&str, &str); 5] = [("playfield", "playfield"), ("video", "playfield-video"), ("backglass", "backglass"), ("dmd", "dmd"), ("wheel", "wheel")];

/// Ours already copied into PinUP Popper's folders, the same file byte for byte, remembered as put in
/// there (`User::in_popper`) so each shows once: copies made before the app kept that record (a
/// wheel showed twice). A pair is compared only when their sizes agree.
fn note_popper_copies(app: &AppHandle) {
    let candidates: Vec<(String, String, PathBuf, Vec<PathBuf>)> = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        st.tables
            .iter()
            .flat_map(|t| {
                let k = key(&t.path);
                let known = st.user.in_popper.get(&k).cloned().unwrap_or_default();
                OWN_KINDS
                    .iter()
                    .filter(move |(own, _)| !known.contains_key(&format!("ours:{own}")))
                    .filter_map(move |(own, kind)| {
                        let (ours, _) = crate::own_pictures::made(&t.id, own)?;
                        let popper: Vec<PathBuf> = t.holds.media.iter().filter(|m| media_kind(m) == *kind).map(|m| m.path.clone()).collect();
                        (!popper.is_empty()).then(|| (k.clone(), format!("ours:{own}"), ours, popper))
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    let mut found: Vec<(String, String, String)> = Vec::new();
    for (k, pick, ours, popper) in candidates {
        let Some(n) = size(&ours) else { continue };
        let same = popper.iter().filter(|p| size(p) == Some(n)).find(|p| std::fs::read(p).ok().is_some_and(|b| std::fs::read(&ours).ok().is_some_and(|o| o == b)));
        if let Some(copy) = same.and_then(|p| p.file_name()) {
            found.push((k, pick, copy.to_string_lossy().to_lowercase()));
        }
    }
    if found.is_empty() {
        return;
    }
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    for (k, pick, copy) in found {
        st.user.in_popper.entry(k).or_default().insert(pick, copy);
    }
    if let Err(e) = st.user.save() {
        eprintln!("PinUP Popper's copies of ours weren't kept: {e}");
    }
}

/// Ours made again, `kinds` of them: a copy in PinUP Popper's folders is the old one now, so ours
/// shows again beside it.
pub fn forget_popper_copies(app: &AppHandle, id: &str, kinds: &[String]) {
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    let Some(k) = st.tables.iter().find(|t| t.id == id).map(|t| key(&t.path)) else { return };
    let Some(copies) = st.user.in_popper.get_mut(&k) else { return };
    let before = copies.len();
    for kind in kinds {
        copies.remove(&format!("ours:{kind}"));
    }
    if copies.len() != before
        && let Err(e) = st.user.save()
    {
        eprintln!("PinUP Popper's copies of ours weren't kept: {e}");
    }
}

/// A Popper media file's kind, as `MEDIA_KINDS` names it.
fn media_kind(m: &Media) -> String {
    let kind = m.kind.to_lowercase();
    let kind = if MEDIA_KINDS.iter().any(|(k, _)| *k == kind) { kind } else { "other".to_string() };
    if m.video { format!("{kind}-video") } else { kind }
}

/// A kind's words.
pub fn kind_label(kind: &str) -> String {
    MEDIA_KINDS.iter().find(|(k, _)| *k == kind).map_or_else(
        || kind.strip_suffix("-video").map_or_else(|| "Other".to_string(), |k| format!("{} video", kind_label(k))),
        |(_, label)| (*label).to_string(),
    )
}

/// What the screens show of a table.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableView<'a> {
    pub id: &'a str,
    /// Its table file's path in lower case, as the user's own lists keep it.
    pub key: String,
    pub path: &'a Path,
    pub file: &'a str,
    pub name: String,
    /// The name without a subtitle, for buttons.
    pub short: String,
    pub maker: Option<String>,
    pub year: Option<i32>,
    /// The badge: SS, EM, PM or Original.
    pub badge: Option<String>,
    /// Solid state, Electro-mechanical, Pure mechanical, Original.
    pub kind: Option<String>,
    pub players: Option<i32>,
    pub board: Option<String>,
    pub designers: Vec<String>,
    pub theme: Vec<String>,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub features: Vec<String>,
    pub ipdb: Option<String>,
    pub vps_link: Option<String>,
    pub on_vps: &'a OnVps,
    /// The matcher's guess, to confirm before anything of VPS's shows.
    pub guess: Option<Guess>,
    pub read_error: Option<&'a str>,
    /// The one of VPX's eight global physics sets it plays, 1 to 8, where it plays one.
    pub physics_set: Option<u32>,
    /// The ROM set its script asks for (after VPinMAME's aliases), for a table that drives PinMAME.
    pub rom: Option<&'a str>,
    pub holds: &'a Holds,
    /// What the drop window added for it that its other rows don't show.
    pub other: &'a [added::File],
    pub vps_has: BTreeMap<&'static str, usize>,
    pub picture: Pictures,
    pub media: Vec<MediaView>,
    /// How many of VPS's pictures the user hid from its page.
    pub hidden: usize,
    /// How many screenshots our player took of it.
    pub screenshots: usize,
    /// The one on top of their stack.
    pub screenshot: Option<String>,
    pub size: u64,
    /// When its file was last changed, in ms since 1970.
    pub changed: i64,
    /// When its file came onto this PC (made in its folder), in ms since 1970.
    pub added: i64,
    pub favourite: bool,
    /// Its rating, one to five stars (`rating_of`).
    pub rating: Option<u8>,
    /// Where its rating's kept: `ours`, else the front end's (`Popper`, `PinballX`, `PinballY`).
    pub rating_from: Option<&'static str>,
    /// Hidden from the library: shown only in the Hidden smart playlist.
    pub hidden_table: bool,
    pub played: Option<&'a Played>,
    /// Its last test.
    pub tested: Option<&'a Tested>,
    /// Its ROM's check, made in the background: how it looks (good, warn, bad), and what it says.
    pub rom_check: Option<&'a (String, String)>,
    /// Its AltSound pack's check.
    pub alt_check: Option<&'a (String, String)>,
    /// Its AltSound pack made from its ROM, with a page of its sounds.
    pub alt_made: bool,
    /// Its own script changed since its script was edited here: what was changed then.
    pub script_update: Option<&'a crate::script_editor::ScriptEdit>,
    /// Its PinUP pack's check, and the layout in place.
    pub pup_check: Option<&'a crate::pup_packs::Facts>,
    /// Its PinUP pack's name before the app renamed it for this table, where it did.
    pub pup_renamed_from: Option<&'a str>,
    /// Its music, to hear while it's chosen in cabinet mode (`tunes::of`, else the sound of the
    /// video our player made of it).
    pub audio: Option<String>,
    /// The kinds of media the app has made of it (`own_pictures::KINDS`), to ask before they're made
    /// again.
    pub made: Vec<&'static str>,
    /// The kinds our player found it has none of when they were last made (no backglass file, no
    /// score display: `own_pictures::none`), not offered to make again.
    pub none: Vec<String>,
    /// The name its script hands PinUP for its pack, the likeliest.
    pub pup_name: Option<&'a str>,
    /// Its name before it was renamed to VPS's, to put back.
    pub renamed_from: Option<&'a str>,
    /// The details the user set ("Edit its details"), and what VPS and its file say, which they're
    /// shown over.
    pub own_details: Option<&'a Details>,
    pub usual_details: Details,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Guess {
    pub name: String,
    pub maker: Option<String>,
    pub year: Option<i32>,
    pub found: Option<&'static str>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pictures {
    pub playfield: Option<String>,
    pub backglass: Option<String>,
    pub wheel: Option<String>,
    /// A playfield video of Popper's, where it has one.
    pub video: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaView {
    /// Its kind, as `MEDIA_KINDS` names it: the stack it's in.
    pub kind: String,
    pub label: String,
    pub url: String,
    pub video: bool,
    /// Where it comes from: VPS, a front end's media folder (Popper, PinballX, PinballY), or ours
    /// (made by our player).
    pub from: &'static str,
    /// Shown whole, as a wheel is, rather than filling its frame.
    pub contain: bool,
    /// What names it for the user's choice of its kind: `vps:<picture>`, `file:<its file's name>`,
    /// `ours:<kind>` — the same whatever else comes and goes.
    pub pick: String,
    /// The one of its kind used: the user's choice, else the first by VPS, Popper, ours.
    pub chosen: bool,
    /// One of a front end's own that it shows, named for its name for the game without a number.
    pub front: bool,
    /// Turned this many degrees clockwise to show upright: the user's word for it (`User::turns`), else
    /// 90 or 270 for a front end's playfield picture or video kept on its side for a cabinet's screen
    /// (`media_facts::on_its_side`, `upright_turn`), else 0.
    pub turn: u16,
    /// A video carrying sound (`media_facts::has_sound`), heard when the user turns it on.
    pub sound: bool,
    /// The file on this PC it's shown from (Popper's, or ours), for what's read of it.
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

fn kind_of(game: &Game) -> (Option<String>, Option<String>) {
    if game.manufacturer == "Original" {
        return (Some("Original".into()), Some("Original".into()));
    }
    let kind = match game.kind.as_deref() {
        Some("SS") => "Solid state",
        Some("EM") => "Electro-mechanical",
        Some("PM") => "Pure mechanical",
        Some("DG") => "Digital",
        _ => return (None, None),
    };
    (game.kind.clone(), Some(kind.to_string()))
}

/// A name and its maker and year from a file name: `Name (Maker Year) …`.
fn from_file_name(stem: &str) -> (String, Option<String>, Option<i32>) {
    let Some(open) = stem.find(" (") else { return (stem.trim().to_string(), None, None) };
    let name = stem[..open].trim().to_string();
    let inside = stem[open + 2..].split(')').next().unwrap_or("");
    let mut words: Vec<&str> = inside.split_whitespace().collect();
    let year = words.last().and_then(|w| w.parse::<i32>().ok()).filter(|y| (1930..=2100).contains(y));
    if year.is_some() {
        words.pop();
    }
    let maker = (!words.is_empty()).then(|| words.join(" "));
    (name, maker, year)
}

fn short_name(name: &str) -> String {
    let cut = [" - ", ": ", " ("].iter().filter_map(|s| name.find(s)).min().unwrap_or(name.len());
    name[..cut].trim().to_string()
}

fn vps_has(game: &Game) -> BTreeMap<&'static str, usize> {
    let vpx = game.table_files.iter().filter(|t| t.table_format.as_deref().is_none_or(|f| f == "VPX")).count();
    BTreeMap::from([
        ("tables", vpx),
        ("backglasses", game.b2s_files.len()),
        ("roms", game.rom_files.len()),
        ("pupPacks", game.pup_pack_files.len()),
        ("altSound", game.alt_sound_files.len()),
        ("colour", game.alt_color_files.len()),
        ("pov", game.pov_files.len()),
        ("wheels", game.wheel_art_files.len()),
        ("toppers", game.topper_files.len()),
        ("mediaPacks", game.media_pack_files.len()),
        ("sounds", game.sound_files.len()),
        ("rules", game.rule_files.len()),
        ("tutorials", game.tutorial_files.len()),
    ])
}

pub fn view<'a>(st: &'a State, t: &'a Table) -> TableView<'a> {
    let db = st.db.as_deref();
    let (matched, matched_file) = db.map_or((None, None), |db| installed::of(db, &t.on_vps));
    // a guess names nothing, shows nothing of VPS's, until the user confirms it
    let (game, file) = if t.on_vps.sure { (matched, matched_file) } else { (None, None) };
    let guess = matched.filter(|_| !t.on_vps.sure).map(|g| Guess {
        name: g.name.clone(),
        maker: Some(g.manufacturer.clone()).filter(|m| !m.is_empty()),
        year: g.year,
        found: t.on_vps.found,
    });
    let placeholder = |n: &str| ["visual pinball demo table", "table1"].contains(&n.to_lowercase().as_str());
    let (file_name, file_maker, file_year) = from_file_name(&t.stem);
    let name = game
        .map(|g| g.name.clone())
        .or_else(|| t.read.info_name.clone().filter(|n| !placeholder(n)))
        .unwrap_or(file_name);
    let (badge, kind) = game.map(kind_of).unwrap_or_default();
    let authors = match file {
        Some(f) if !f.authors.is_empty() => f.authors.clone(),
        _ => t.read.info_authors.as_deref().map(|a| a.split([',', '&']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
    };
    let base = pictures::base();
    // VPS's pictures, but those the user hid from this table
    let hidden = st.user.hidden.get(&key(&t.path));
    let vps_picture = |id: &str| (st.vps_pictures.contains_key(id) && !hidden.is_some_and(|h| h.contains(id))).then(|| format!("{base}v/{id}"));
    let game_tables = |g: &Game| -> Option<String> {
        g.table_files.iter().filter(|f| f.table_format.as_deref().is_none_or(|x| x == "VPX")).find_map(|f| vps_picture(&f.id))
    };
    // the pictures our player made of it, each marked with when, so a new one shows at once
    let own = |kind: &str| crate::own_pictures::made(&t.id, kind).map(|(_, when)| format!("{base}o/{}/{kind}?v={when}", t.id));
    // every picture and video it has, by kind, each kind's first as they were chosen before there
    // was a choice: a playfield VPS's, else Popper's, else ours; a backglass Popper's, else VPS's
    // (its backglass file's, else its game's), else ours; the rest Popper's, else ours
    let mut media: Vec<MediaView> = Vec::new();
    // what was put in Popper's folders from VPS's or ours, while Popper's copy is there: shown once
    let in_popper = st.user.in_popper.get(&key(&t.path));
    let held: BTreeSet<String> = t.holds.media.iter().filter_map(|m| m.path.file_name().map(|n| n.to_string_lossy().to_lowercase())).collect();
    let copied = |pick: &str| in_popper.and_then(|c| c.get(pick)).is_some_and(|f| held.contains(f));
    let listed = st.layout.as_ref().map(|l| ::library::installed::listed_name(&l.tables, &t.path)).unwrap_or_default();
    let popper_name = st.popper_names.get(&listed.to_lowercase()).cloned().unwrap_or_else(|| t.stem.clone());
    // the name each front end's media for it go under: Popper's own name for the game, PinballX's
    // and PinballY's descriptions of it, else its file's name
    let file_name = t.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let front_name = |from: ::library::frontends::Front| match from {
        ::library::frontends::Front::Popper => popper_name.clone(),
        other => st.fronts.iter().find(|f| f.front.front == other).map_or_else(|| t.stem.clone(), |f| f.media_name(&file_name)),
    };
    // which way Popper's playfield media kept on their side stand up: the user's word for one, else
    // matched against an upright picture of the table (VPS's on this PC, else ours), its videos as
    // its picture on its side, else a quarter right
    let turns = st.user.turns.get(&key(&t.path));
    let upright = file
        .filter(|f| st.vps_pictures.contains_key(&f.id))
        .or_else(|| game.and_then(|g| g.table_files.iter().filter(|f| f.table_format.as_deref().is_none_or(|x| x == "VPX")).find(|f| st.vps_pictures.contains_key(&f.id))))
        .and_then(|f| st.vps_pictures.get(&f.id))
        .and_then(|url| pictures::vps_on_pc(url))
        .or_else(|| crate::own_pictures::made(&t.id, "playfield").map(|(path, _)| path));
    let picture_turn = t
        .holds
        .media
        .iter()
        .find(|m| !m.video && media_kind(m) == "playfield" && crate::media_facts::on_its_side(&m.path, false))
        .map(|m| crate::media_facts::upright_turn(&m.path, upright.as_deref()));
    let mut add = |kind: &str, url: String, video: bool, from: &'static str, pick: String, front: bool, file: Option<PathBuf>| {
        if !media.iter().any(|v| v.url == url) && !copied(&pick) {
            let contain = kind == "wheel" || (from == "ours" && (kind == "backglass" || kind == "dmd"));
            // a front end's playfield pictures and videos made for a cabinet lie on their side
            let side = FRONT_ENDS.contains(&from) && kind.starts_with("playfield") && file.as_deref().is_some_and(|f| crate::media_facts::on_its_side(f, video));
            // the user's word for any picture or video; else Popper's on their side stood up
            let turn = match (turns.and_then(|m| m.get(&pick)), side) {
                (Some(&said), _) => said,
                (None, false) => 0,
                (None, true) if video => picture_turn.unwrap_or(90),
                (None, true) => file.as_deref().map_or(90, |f| crate::media_facts::upright_turn(f, upright.as_deref())),
            };
            let sound = video && file.as_deref().is_some_and(crate::media_facts::has_sound);
            media.push(MediaView { kind: kind.to_string(), label: kind_label(kind), url, video, from, contain, pick, chosen: false, front, turn, sound, file });
        }
    };
    let vps_pick = |url: &str| format!("vps:{}", url.rsplit('/').next().unwrap_or_default());
    if let Some(url) = file.and_then(|f| vps_picture(&f.id)).or_else(|| game.and_then(game_tables)) {
        let pick = vps_pick(&url);
        add("playfield", url, false, "VPS", pick, false, None);
    }
    // Popper's own, a file kept twice shown once: a copy in the same kind's folder (numbered beside
    // it), or one in the playfield's folder that's another kind's file (a topper's intro video put
    // there too), shown as that kind's
    let prints: Vec<(String, Option<u64>)> = t.holds.media.iter().map(|m| (media_kind(m), crate::media_facts::fingerprint(&m.path))).collect();
    for (n, m) in t.holds.media.iter().enumerate() {
        let (kind, print) = &prints[n];
        let twice = print.is_some() && prints.iter().enumerate().any(|(k, (other, p))| p == print && ((other == kind && k < n) || (other != kind && kind.starts_with("playfield") && !other.starts_with("playfield"))));
        if twice {
            continue;
        }
        let name = m.path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let front = m.path.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(&front_name(m.from)));
        add(kind, format!("{base}t/{}/{n}", t.id), m.video, m.from.key(), format!("file:{name}"), front, Some(m.path.clone()));
    }
    if let Some(url) = game.and_then(|g| g.b2s_files.iter().find_map(|b| vps_picture(&b.id))).or_else(|| game.and_then(|g| vps_picture(&g.id))) {
        let pick = vps_pick(&url);
        add("backglass", url, false, "VPS", pick, false, None);
    }
    for (own_kind, kind) in OWN_KINDS {
        if let Some(url) = own(own_kind) {
            let file = crate::own_pictures::made(&t.id, own_kind).map(|(path, _)| path);
            add(kind, url, own_kind == "video", "ours", format!("ours:{own_kind}"), false, file);
        }
    }
    // in the kinds' order, the one used first in each: the user's choice, while it's there
    let chosen = st.user.chosen.get(&key(&t.path));
    // a kind's place; a video of a kind with none of its own (a loading video) beside its pictures
    let place = |kind: &str| {
        let at = |k: &str| MEDIA_KINDS.iter().position(|(m, _)| *m == k);
        at(kind).or_else(|| kind.strip_suffix("-video").and_then(at)).unwrap_or(MEDIA_KINDS.len())
    };
    let mut kinds: Vec<String> = Vec::new();
    for m in &media {
        if !kinds.contains(&m.kind) {
            kinds.push(m.kind.clone());
        }
    }
    let wheels = media.iter().filter(|m| m.kind == "wheel").count();
    // a backglass: Popper's own first, then the one our player drew from the table's own backglass
    // file, then VPS's (a small picture of it, at times cut short) — a copy of ours or VPS's put in
    // Popper's folder counting as what it copies
    let copy_of = |m: &MediaView| m.pick.strip_prefix("file:").and_then(|name| in_popper.and_then(|c| c.iter().find(|(_, f)| f.as_str() == name).map(|(pick, _)| pick.clone())));
    let backglass_rank = |m: &MediaView| match (FRONT_ENDS.contains(&m.from), m.from, copy_of(m)) {
        (true, _, Some(of)) if of.starts_with("vps:") => 2,
        (true, _, Some(_)) | (_, "ours", _) => 1,
        (true, _, None) => 0,
        _ => 2,
    };
    for kind in &kinds {
        let wanted = chosen.and_then(|c| c.get(kind));
        let n = media
            .iter()
            .position(|m| &m.kind == kind && Some(&m.pick) == wanted)
            // a wheel, where there's a choice: the first that looks like one (a front end's wheel
            // folder can hold a rule card or a banner under the table's name)
            .or_else(|| (kind == "wheel" && wheels > 1).then(|| media.iter().position(|m| m.kind == "wheel" && m.file.as_deref().is_some_and(crate::media_facts::looks_like_wheel))).flatten())
            .or_else(|| (kind == "backglass").then(|| media.iter().enumerate().filter(|(_, m)| m.kind == "backglass").min_by_key(|(_, m)| backglass_rank(m)).map(|(n, _)| n)).flatten())
            .or_else(|| media.iter().position(|m| &m.kind == kind));
        if let Some(n) = n {
            media[n].chosen = true;
        }
    }
    media.sort_by_cached_key(|m| (place(&m.kind), m.kind.clone(), !m.chosen));
    let used = |kind: &str| media.iter().find(|m| m.kind == kind && m.chosen).map(|m| m.url.clone());
    let (playfield, backglass, wheel, video) = (used("playfield"), used("backglass"), used("wheel"), used("playfield-video"));

    let key = key(&t.path);
    // the details the user set, shown over VPS's and the file's
    let maker = game.map(|g| g.manufacturer.clone()).filter(|m| !m.is_empty()).or(file_maker);
    let year = game.and_then(|g| g.year).or(file_year);
    let version = file.and_then(|f| f.version.clone()).or_else(|| t.read.info_version.clone());
    let usual_details = Details { name: Some(name.clone()), maker: maker.clone(), year, badge: badge.clone(), version: version.clone(), authors: Some(authors.clone()) };
    let own_details = st.user.details.get(&key);
    let mine = own_details.cloned().unwrap_or_default();
    let name = mine.name.unwrap_or(name);
    let (badge, kind) = match mine.badge {
        Some(b) => (Some(b.clone()), kind_named(&b)),
        None => (badge, kind),
    };
    let authors = mine.authors.unwrap_or(authors);
    TableView {
        id: &t.id,
        key: key.clone(),
        path: &t.path,
        file: &t.stem,
        short: short_name(&name),
        name,
        maker: mine.maker.or(maker),
        year: mine.year.or(year),
        badge,
        kind,
        players: game.and_then(|g| g.players),
        board: game.and_then(|g| g.mpu.clone()),
        designers: game.map(|g| g.designers.clone()).unwrap_or_default(),
        theme: game.map(|g| g.theme.clone()).unwrap_or_default(),
        version: mine.version.or(version),
        authors,
        features: file.map(|f| f.features.clone()).unwrap_or_default(),
        ipdb: game.and_then(|g| g.ipdb_url.clone()).filter(|u| !u.is_empty()),
        vps_link: game.map(|g| format!("{}{}", vps::GAME_LINK, g.id)),
        on_vps: &t.on_vps,
        guess,
        read_error: t.read.error.as_deref(),
        physics_set: t.read.physics_set,
        rom: t.holds.rom_set.as_deref(),
        holds: &t.holds,
        other: &t.other,
        vps_has: game.map(vps_has).unwrap_or_default(),
        picture: Pictures { playfield, backglass, wheel, video },
        media,
        hidden: hidden.map_or(0, BTreeSet::len),
        screenshots: crate::screenshots::count(&t.stem),
        screenshot: crate::screenshots::first(&t.stem, &t.id),
        size: t.read.size,
        changed: t.read.modified,
        added: t.added,
        favourite: st.user.favourites.contains(&key),
        rating: rating_of(st, t).map(|r| r.0),
        rating_from: rating_of(st, t).map(|r| r.1),
        hidden_table: st.user.hidden_tables.contains(&key),
        played: st.user.played.get(&key),
        tested: st.user.tests.get(&key),
        rom_check: t.holds.rom.as_ref().and(t.holds.rom_set.as_ref()).and_then(|s| st.rom_checks.get(&s.to_lowercase())),
        alt_check: t.holds.altsound.as_ref().and_then(|p| st.alt_checks.get(&self::key(p))),
        alt_made: t.holds.altsound.as_ref().is_some_and(|p| st.alt_made.contains(&self::key(p))),
        script_update: st.script_updates.contains(&key).then(|| st.user.script_edits.get(&key)).flatten(),
        pup_check: t.holds.pup_pack.as_ref().and_then(|p| st.pup_checks.get(&self::key(p))),
        pup_renamed_from: t.holds.pup_pack.as_ref().and_then(|p| st.user.pup_renames.get(&self::key(p))).map(String::as_str),
        // its music as `tunes::of` found it, else the sound of the video our player made of it
        audio: if st.tunes.contains_key(&key) {
            Some(format!("{}a/{}", pictures::base(), t.id))
        } else {
            crate::own_pictures::made(&t.id, "video").filter(|(path, _)| crate::media_facts::has_sound(path)).map(|(_, when)| format!("{}a/{}?v={when}", pictures::base(), t.id))
        },
        made: crate::own_pictures::KINDS.iter().filter(|(kind, _)| crate::own_pictures::made(&t.id, kind).is_some()).map(|(kind, _)| *kind).collect(),
        none: crate::own_pictures::none(&t.id),
        pup_name: t.read.pup_names.first().map(String::as_str),
        renamed_from: st.user.renames.get(&key).map(|r| r.from.as_str()),
        own_details,
        usual_details,
    }
}

/// One file VPS lists for a game, for "More on VPS".
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpsFile {
    pub id: String,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub comment: Option<String>,
    /// When it was added to VPS, in ms since 1970 (VPS's own change dates were all reset at once
    /// in May 2026, so they say little).
    pub added: Option<i64>,
    pub links: Vec<String>,
    pub picture: Option<String>,
    /// A table file's format and features, a colouring's kind.
    pub format: Option<String>,
    pub features: Vec<String>,
    /// Whether it's the table file the table matched.
    pub this_one: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpsKind {
    pub kind: &'static str,
    pub label: &'static str,
    pub files: Vec<VpsFile>,
}

fn links(urls: &[vps::Link]) -> Vec<String> {
    urls.iter().filter(|l| l.broken != Some(true) && (l.url.starts_with("https://") || l.url.starts_with("http://"))).map(|l| l.url.clone()).collect()
}

/// Every file VPS lists for a table's game, by kind, the newest first.
pub fn vps_files(st: &State, t: &Table) -> Vec<VpsKind> {
    let Some(db) = st.db.as_deref().filter(|_| t.on_vps.sure) else { return Vec::new() };
    let (Some(game), file) = installed::of(db, &t.on_vps) else { return Vec::new() };
    game_files(st, game, file)
}

/// A VPS game for the drop window's "See them", a panel beside its downloads with the VPS table, its
/// files and links: its name, maker, year, picture and link, and every file VPS lists for it, the
/// release `file_id` names marked as the one dropped.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpsGameView {
    pub name: String,
    pub maker: Option<String>,
    pub year: Option<i32>,
    pub picture: Option<String>,
    pub link: String,
    pub kinds: Vec<VpsKind>,
}

pub fn vps_game(st: &State, game_id: &str, file_id: Option<&str>) -> Option<VpsGameView> {
    let db = st.db.as_deref()?;
    let game = db.game(game_id)?;
    let file = file_id.and_then(|id| game.table_files.iter().find(|f| f.id == id));
    let base = pictures::base();
    let picture = |id: &str| st.vps_pictures.contains_key(id).then(|| format!("{base}v/{id}"));
    Some(VpsGameView {
        name: game.name.clone(),
        maker: Some(game.manufacturer.clone()).filter(|m| !m.is_empty()),
        year: game.year,
        picture: file.and_then(|f| picture(&f.id)).or_else(|| game.b2s_files.iter().find_map(|b| picture(&b.id))).or_else(|| picture(&game.id)),
        link: format!("{}{}", vps::GAME_LINK, game.id),
        kinds: game_files(st, game, file),
    })
}

/// Every file VPS lists for `game`, by kind, `file` marked as the user's own.
fn game_files(st: &State, game: &Game, file: Option<&TableFile>) -> Vec<VpsKind> {
    let base = pictures::base();
    let picture = |id: &str| st.vps_pictures.contains_key(id).then(|| format!("{base}v/{id}"));
    // the user's own release first, then the newest on VPS
    let mut tables: Vec<&TableFile> = game.table_files.iter().collect();
    tables.sort_by_key(|f| (file.is_none_or(|m| m.id != f.id), std::cmp::Reverse(f.created_at.or(f.updated_at))));
    let mut kinds = vec![VpsKind {
        kind: "tables",
        label: "Table releases",
        files: tables
            .into_iter()
            .map(|f| VpsFile {
                id: f.id.clone(),
                version: f.version.clone(),
                authors: f.authors.clone(),
                comment: f.comment.clone().filter(|c| !c.is_empty()),
                added: f.created_at.or(f.updated_at),
                links: links(&f.urls),
                picture: picture(&f.id),
                format: f.table_format.clone(),
                features: f.features.clone(),
                this_one: file.is_some_and(|m| m.id == f.id),
            })
            .collect(),
    }];
    let assets: [(&'static str, &'static str, &Vec<Asset>); 12] = [
        ("backglasses", "Backglasses", &game.b2s_files),
        ("roms", "ROMs", &game.rom_files),
        ("pupPacks", "PinUP packs", &game.pup_pack_files),
        ("altSound", "AltSound packs", &game.alt_sound_files),
        ("colour", "Color DMDs", &game.alt_color_files),
        ("pov", "POV files", &game.pov_files),
        ("wheels", "Wheels", &game.wheel_art_files),
        ("toppers", "Toppers", &game.topper_files),
        ("mediaPacks", "Media packs", &game.media_pack_files),
        ("sounds", "Sound packs", &game.sound_files),
        ("rules", "Rule cards", &game.rule_files),
        ("tutorials", "Tutorials", &game.tutorial_files),
    ];
    for (kind, label, list) in assets {
        let mut list: Vec<&Asset> = list.iter().collect();
        list.sort_by_key(|a| std::cmp::Reverse(a.created_at.or(a.updated_at)));
        let files: Vec<VpsFile> = list
            .into_iter()
            .map(|a| {
                let mut links = links(&a.urls);
                if let Some(u) = a.url.as_ref().filter(|u| u.starts_with("https://") || u.starts_with("http://")) {
                    links.push(u.clone());
                }
                VpsFile {
                    id: a.id.clone(),
                    version: a.version.clone().or_else(|| a.title.clone()),
                    authors: a.authors.clone(),
                    comment: a.comment.clone().filter(|c| !c.is_empty()),
                    added: a.created_at.or(a.updated_at),
                    links,
                    picture: picture(&a.id),
                    format: a.kind.clone(),
                    features: Vec::new(),
                    this_one: false,
                }
            })
            .collect();
        if !files.is_empty() {
            kinds.push(VpsKind { kind, label, files });
        }
    }
    kinds
}

/// One machine VPS lists, as the VPS browser's cards and filters need it (the catalog's filters,
/// "VPS has" chips, cards with the machine's picture, name, maker and year, its
/// type and what VPS has, and whether it's in the library).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub maker: Option<String>,
    pub year: Option<i32>,
    /// SS, EM, PM, DG or Original, and the type in words.
    pub badge: Option<String>,
    pub kind: Option<String>,
    pub players: Option<i32>,
    pub designers: Vec<String>,
    pub theme: Vec<String>,
    /// The table formats made of it: VPX, FP and the like.
    pub formats: Vec<String>,
    /// How many of each kind of file VPS lists (VPX tables only, as the library counts them).
    pub has: BTreeMap<&'static str, usize>,
    /// Its table releases in every format.
    pub releases: usize,
    /// Its newest table release, in ms since 1970.
    pub newest: Option<i64>,
    pub picture: Option<String>,
    /// The library's table of it, if it's in the library.
    pub mine: Option<String>,
    /// For the search: who made its tables, its ROM sets' names.
    pub authors: Vec<String>,
    pub roms: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    pub machines: Vec<Machine>,
    /// When VPS's list was last fetched, in ms since 1970 (it's fetched once a day).
    pub refreshed: Option<i64>,
}

/// Every machine VPS lists, for the VPS browser: the newest first, as VPS's own site shows them
/// (the screens sort them their own way).
pub fn browser(st: &State) -> Browser {
    let Some(db) = st.db.as_deref() else { return Browser { machines: Vec::new(), refreshed: st.status.vps_fetched } };
    let base = pictures::base();
    let picture = |id: &str| st.vps_pictures.contains_key(id).then(|| format!("{base}v/{id}"));
    let mut machines: Vec<Machine> = db
        .games
        .iter()
        .filter(|g| g.broken != Some(true) && !g.name.trim().is_empty())
        .map(|g| {
            let (badge, kind) = kind_of(g);
            let mut formats: Vec<String> = Vec::new();
            for f in g.table_files.iter().filter_map(|f| f.table_format.clone()) {
                if !formats.contains(&f) {
                    formats.push(f);
                }
            }
            let mut authors: Vec<String> = Vec::new();
            for a in g.table_files.iter().flat_map(|f| f.authors.iter()) {
                if !authors.iter().any(|x| x.eq_ignore_ascii_case(a)) {
                    authors.push(a.clone());
                }
            }
            // its newest VPX table's picture, as a table's tile shows its own; else its backglass's
            let mut tables: Vec<&TableFile> = g.table_files.iter().filter(|f| f.table_format.as_deref().is_none_or(|x| x == "VPX")).collect();
            tables.sort_by_key(|f| std::cmp::Reverse(f.created_at.or(f.updated_at)));
            Machine {
                id: g.id.clone(),
                name: g.name.clone(),
                maker: Some(g.manufacturer.clone()).filter(|m| !m.is_empty()),
                year: g.year,
                badge,
                kind,
                players: g.players,
                designers: g.designers.clone(),
                theme: g.theme.clone(),
                formats,
                has: vps_has(g),
                releases: g.table_files.len(),
                newest: g.table_files.iter().filter_map(|f| f.created_at.or(f.updated_at)).max(),
                picture: tables.iter().find_map(|f| picture(&f.id)).or_else(|| g.b2s_files.iter().find_map(|b| picture(&b.id))).or_else(|| picture(&g.id)),
                mine: st.tables.iter().find(|t| t.on_vps.sure && t.on_vps.game.as_deref() == Some(g.id.as_str())).map(|t| t.id.clone()),
                authors,
                roms: g.rom_files.iter().filter_map(|r| r.version.clone()).collect(),
            }
        })
        .collect();
    machines.sort_by_key(|m| std::cmp::Reverse(m.newest));
    Browser { machines, refreshed: st.status.vps_fetched }
}

/// The user's word on a table's VPS match: the guess confirmed, or the table not on VPS.
pub fn choose_vps(st: &mut State, id: &str, confirm: bool) -> Result<(), String> {
    let t = st.tables.iter_mut().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let choice = if confirm { VpsChoice { game: t.on_vps.game.clone(), table_file: t.on_vps.table_file.clone() } } else { VpsChoice::default() };
    t.on_vps = choice.on_vps();
    let key = key(&t.path);
    st.user.vps.insert(key, choice);
    st.user.save()
}

/// The user's own VPS match for a table, chosen by hand (after Not this one,
/// VPS searched and the table and its version picked), kept as VPI's own
/// ("Associate in VPI only"): the table's files left as they are.
pub fn set_vps(st: &mut State, id: &str, game: String, table_file: Option<String>) -> Result<(), String> {
    let db = st.db.clone().ok_or("VPS's database isn't read yet.")?;
    let g = db.game(&game).ok_or("VPS doesn't list that machine.")?;
    if table_file.as_deref().is_some_and(|f| !g.table_files.iter().any(|x| x.id == f)) {
        return Err("VPS doesn't list that table for it.".into());
    }
    let t = st.tables.iter_mut().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let choice = VpsChoice { game: Some(game), table_file };
    t.on_vps = choice.on_vps();
    let key = key(&t.path);
    st.user.vps.insert(key, choice);
    st.user.save()
}

/// What "Overwrite for all players" renames for a table matched to VPS game `game` (PinUP Popper's
/// game list changing with it): the table file and the
/// files beside it, PinUP Popper's media named for it and its game in Popper's list, each to VPS's
/// name for the game, `Name (Maker Year)` (`library::rename`) — shown before anything is done.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Renames {
    pub name: String,
    pub files: Vec<(PathBuf, PathBuf)>,
    /// Popper's own name for its game and the name it shows, each before and after.
    pub popper: Option<[String; 4]>,
    /// Why it can't be done now, if it can't.
    pub blocked: Option<String>,
}

/// Each table in the shared tables folder with what it holds and the folder its game would get:
/// VPS's name for its game, `Name (Maker Year)`, else its file's name.
pub fn layout_tables(st: &State) -> Vec<(PathBuf, Holds, String)> {
    st.tables
        .iter()
        .map(|t| {
            let folder = t.on_vps.game.as_deref().and_then(|g| vps_file_name(st, g).ok()).unwrap_or_else(|| t.stem.clone());
            (t.path.clone(), t.holds.clone(), folder)
        })
        .collect()
}

/// After the library's moved into folders of its games (or back): each table's own records, the
/// pictures our player made of it and its screenshots carried to its new place, as a rename
/// carries them, and the record of what was done kept or let go.
pub fn tables_moved(st: &mut State, moved: &[(PathBuf, PathBuf)], switched: Option<::library::layout_switch::Switched>) -> Result<(), String> {
    for (from, to) in moved {
        let stem = from.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        move_own(&mut st.user, from, to, &stem, &stem);
        move_folders(from, to);
        if let Some(r) = st.user.renames.remove(&key(from)) {
            st.user.renames.insert(key(to), r);
        }
    }
    st.user.switched = switched;
    st.user.save()
}

fn vps_file_name(st: &State, game: &str) -> Result<String, String> {
    let db = st.db.as_deref().ok_or("VPS's database isn't read yet.")?;
    let g = db.game(game).ok_or("VPS doesn't list that machine.")?;
    Ok(::library::rename::vps_name(&g.name, Some(&g.manufacturer), g.year))
}

pub fn vps_names(st: &State, id: &str, game: &str) -> Result<Renames, String> {
    let name = vps_file_name(st, game)?;
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let layout = st.layout.clone().ok_or("The library isn't read yet.")?;
    if name.eq_ignore_ascii_case(&t.stem) {
        return Ok(Renames { name, files: Vec::new(), popper: None, blocked: None });
    }
    Ok(match ::library::rename::plan(&layout, &t.path, &name) {
        Ok(p) => {
            let running = ::library::rename::holders_running();
            Renames {
                popper: p.popper.as_ref().map(|(b, a)| [b.name.clone(), a.name.clone(), b.display.clone(), a.display.clone()]),
                files: p.files,
                blocked: (!running.is_empty()).then(|| format!("Close {} first: {} the files or the game list.", running.join(" and "), if running.len() == 1 { "it holds" } else { "they hold" })),
                name,
            }
        }
        Err(e) => Renames { name, files: Vec::new(), popper: None, blocked: Some(e) },
    })
}

/// The user's own moved from a table's old path to its new one, as a rename moves the table: its
/// favourite, plays, VPS match, test, hidden pictures, ROM change, playlists and chosen media (a
/// chosen Popper picture renamed with it), and what the drop window added for it.
fn move_own(user: &mut User, from: &Path, to: &Path, from_stem: &str, to_stem: &str) {
    let (a, b) = (key(from), key(to));
    if user.favourites.remove(&a) {
        user.favourites.insert(b.clone());
    }
    if user.hidden_tables.remove(&a) {
        user.hidden_tables.insert(b.clone());
    }
    fn moved<V>(map: &mut BTreeMap<String, V>, a: &str, b: &str) {
        if let Some(v) = map.remove(a) {
            map.insert(b.to_string(), v);
        }
    }
    moved(&mut user.played, &a, &b);
    moved(&mut user.ratings, &a, &b);
    if user.ratings_unsent.remove(&a) {
        user.ratings_unsent.insert(b.clone());
    }
    moved(&mut user.vps, &a, &b);
    moved(&mut user.tests, &a, &b);
    moved(&mut user.hidden, &a, &b);
    moved(&mut user.rom_changes, &a, &b);
    moved(&mut user.script_edits, &a, &b);
    moved(&mut user.details, &a, &b);
    // the script a ROM's change kept beside the table is renamed with it
    if let Some(c) = user.rom_changes.get_mut(&b)
        && c.before.is_some()
    {
        c.before = Some(to.with_extension("vbs.before-rom-change"));
    }
    moved(&mut user.chosen, &a, &b);
    if let Some(picks) = user.chosen.get_mut(&b) {
        let (old, new) = (format!("file:{}", from_stem.to_lowercase()), format!("file:{}", to_stem.to_lowercase()));
        for pick in picks.values_mut() {
            if let Some(rest) = pick.strip_prefix(&old) {
                *pick = format!("{new}{rest}");
            }
        }
    }
    for pl in &mut user.playlists {
        for k in &mut pl.tables {
            if *k == a {
                k.clone_from(&b);
            }
        }
    }
    crate::added::moved(from, to);
}

/// A table's rating: the user's own here (0 one cleared, over a front end's), else the first front
/// end's that rates it; with where it's kept (`ours`, or the front end's key).
pub fn rating_of(st: &State, t: &Table) -> Option<(u8, &'static str)> {
    match st.user.ratings.get(&key(&t.path)) {
        Some(0) => None,
        Some(s) => Some((*s, "ours")),
        None => {
            let layout = st.layout.as_ref()?;
            let r = st.front_ratings.get(&::library::ratings::key(&installed::listed_name(&layout.tables, &t.path)))?;
            Some((r.stars, r.from.key()))
        }
    }
}

/// The library as VPIDB counts it, where the player shares it: each table matched to VPS for sure —
/// its game and table file there — with its rating; nothing else (no file names, folders or plays).
/// Each game and table file once.
pub fn vpidb_library(st: &State) -> Vec<serde_json::Value> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for t in &st.tables {
        let Some(game) = t.on_vps.game.as_ref().filter(|_| t.on_vps.sure) else { continue };
        if !seen.insert((game.clone(), t.on_vps.table_file.clone())) {
            continue;
        }
        out.push(serde_json::json!({ "game": game, "file": t.on_vps.table_file, "rating": rating_of(st, t).map(|r| r.0) }));
    }
    out
}

/// Whether the user wants the ratings set here put in the front ends' lists too
/// (`prefs.ratingsToFronts`, asked on the first rating).
pub fn ratings_to_fronts(prefs: &serde_json::Value) -> bool {
    prefs.get("ratingsToFronts").and_then(serde_json::Value::as_bool).unwrap_or(false)
}

/// The front ends the library's ratings can go in, by name, for the screens to ask and say.
pub fn rating_fronts(st: &State) -> Vec<&'static str> {
    st.layout.as_ref().map(|l| ::library::ratings::fronts_there(l, &st.fronts).into_iter().map(|f| f.name()).collect()).unwrap_or_default()
}

/// A table rated here (none: cleared), kept as the user's own, and put in the front ends' lists too
/// while the user wants that; what went wrong in a front end's list, in words.
pub fn set_rating(st: &mut State, id: &str, stars: Option<u8>) -> Result<Vec<String>, String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let k = key(&t.path);
    st.user.ratings.insert(k.clone(), stars.unwrap_or(0).min(5));
    let problems = if ratings_to_fronts(&st.user.prefs) { send_ratings(st, &[k]) } else { Vec::new() };
    st.user.save()?;
    Ok(problems)
}

/// Every rating the user set here put in the front ends' lists (the user switched that on).
pub fn send_all_ratings(st: &mut State) -> Result<Vec<String>, String> {
    let keys: Vec<String> = st.user.ratings.keys().cloned().collect();
    let problems = send_ratings(st, &keys);
    st.user.save()?;
    Ok(problems)
}

/// The user's own ratings of the tables `keys` put in each front end's list that holds them, those
/// running left until the library's read again (`ratings_unsent`), PinUP Popper's list kept the
/// first time each run. A rating every front end there took is theirs from then on, so a change
/// made in one later shows here; one none lists stays ours. What went wrong, in words.
fn send_ratings(st: &mut State, keys: &[String]) -> Vec<String> {
    let Some(layout) = st.layout.clone() else { return Vec::new() };
    let running = ::library::ratings::running();
    let there = ::library::ratings::fronts_there(&layout, &st.fronts);
    let mut problems = Vec::new();
    for k in keys {
        let Some(t) = st.tables.iter().find(|t| key(&t.path) == *k) else {
            st.user.ratings_unsent.remove(k);
            continue;
        };
        let Some(own) = st.user.ratings.get(k).copied() else {
            st.user.ratings_unsent.remove(k);
            continue;
        };
        let stars = (own > 0).then_some(own);
        let listed = installed::listed_name(&layout.tables, &t.path);
        let kept_dir = data_folder().map(|d| d.join("Library").join("Kept"));
        let stamp = stamp();
        let keep = match (&kept_dir, st.popper_kept_for_ratings) {
            (Some(dir), false) => Some((dir.as_path(), stamp.as_str())),
            _ => None,
        };
        let done = ::library::ratings::write(&layout, &st.fronts, &listed, stars, &running, keep);
        if done.iter().any(|(f, r)| *f == ::library::frontends::Front::Popper && r.is_ok()) {
            st.popper_kept_for_ratings = true;
        }
        let mut held_by = None;
        let mut left = false;
        for (front, r) in &done {
            match r {
                Ok(true) => held_by = held_by.or(Some(*front)),
                Ok(false) => {}
                Err(e) => {
                    left = true;
                    problems.push(format!("{}: {e}", front.name()));
                }
            }
        }
        let skipped: Vec<&'static str> = there.iter().filter(|f| running.contains(f)).map(|f| f.name()).collect();
        if !skipped.is_empty() {
            left = true;
            problems.push(format!("{} is open, so its list keeps its own rating until it's closed and the library's read again.", skipped.join(" and ")));
        }
        // the front ends hold it now: theirs from here on, unless one's still to take it
        if let Some(front) = held_by {
            let rkey = ::library::ratings::key(&listed);
            match stars {
                Some(s) => {
                    st.front_ratings.insert(rkey, ::library::ratings::Rating { stars: s, from: front });
                }
                None => {
                    st.front_ratings.remove(&rkey);
                }
            }
            if !left {
                st.user.ratings.remove(k);
            }
        }
        if left {
            st.user.ratings_unsent.insert(k.clone());
        } else {
            st.user.ratings_unsent.remove(k);
        }
    }
    problems
}

/// Our own folders for a table moved with it: the pictures our player made (by its ID) and its
/// screenshots (by its name).
fn move_folders(from: &Path, to: &Path) {
    if let (Some(a), Some(b)) = (crate::own_pictures::folder(&id_of(from)), crate::own_pictures::folder(&id_of(to)))
        && a.is_dir()
        && !b.exists()
    {
        let _ = std::fs::rename(a, b);
    }
    let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if let (Some(a), Some(b)) = (crate::screenshots::folder(&stem(from)), crate::screenshots::folder(&stem(to)))
        && a.is_dir()
        && !b.exists()
    {
        let _ = std::fs::rename(a, b);
    }
}

/// Now, local time, for a file's name: `2025-01-02 18-04-05`.
pub fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    // the local offset isn't known here; UTC, marked so
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02} {:02}-{:02}-{:02} UTC", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Days since 1970 as a date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// "Overwrite for all players": the table matched to VPS's game `game` (and its release `file`),
/// then renamed to VPS's name for it with its files, Popper's media and Popper's game (Popper's
/// list kept first, in `Library\Kept`); the user's own moved with it; what was done remembered, to
/// put back. Its new ID.
pub fn overwrite_vps(st: &mut State, id: &str, game: String, file: Option<String>) -> Result<String, String> {
    set_vps(st, id, game.clone(), file)?;
    let name = vps_file_name(st, &game)?;
    let layout = st.layout.clone().ok_or("The library isn't read yet.")?;
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let (from_path, from_stem) = (t.path.clone(), t.stem.clone());
    if name.eq_ignore_ascii_case(&from_stem) {
        return Ok(id.to_string());
    }
    let plan = ::library::rename::plan(&layout, &from_path, &name)?;
    let kept = data_folder().ok_or("no APPDATA folder")?.join("Library").join("Kept");
    let done = ::library::rename::carry_out(&plan, &kept, &stamp(), &from_stem, &name)?;
    let to_path = from_path.with_file_name(format!("{name}.{}", from_path.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| "vpx".into())));
    move_own(&mut st.user, &from_path, &to_path, &from_stem, &name);
    move_folders(&from_path, &to_path);
    st.user.renames.remove(&key(&from_path));
    st.user.renames.insert(key(&to_path), done);
    st.user.save()?;
    Ok(id_of(&to_path))
}

/// What removing a table could take (`library::remove::plan`), and what's ours of it: the record
/// of its plays, the pictures our player made for it and its screenshots. Its plays, its high scores
/// and its screenshots are the user's own to give up, so they start unticked.
pub fn remove_plan(st: &State, id: &str) -> Result<Vec<::library::remove::Item>, String> {
    use ::library::remove::{Item, Seen, plan};
    let layout = st.layout.as_ref().ok_or("The library isn't read yet.")?;
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    // the other tables by their files' names: two releases of a machine can carry the same name inside
    let names: Vec<String> = st.tables.iter().map(|t| t.stem.clone()).collect();
    fn seen<'a>((t, name): (&'a Table, &'a String)) -> Seen<'a> {
        Seen { name, path: &t.path, read: &t.read, holds: &t.holds }
    }
    let others: Vec<Seen> = st.tables.iter().zip(&names).filter(|(o, _)| o.id != id).map(seen).collect();
    let this = st.tables.iter().zip(&names).find(|(o, _)| o.id == id).map(seen).ok_or("That table isn't in the library any more.")?;
    let mut items = plan(layout, &this, &others);
    for i in &mut items {
        if i.key == "scores" {
            i.ticked = false;
        }
    }
    items.extend(crate::own_pictures::folder(id).and_then(|f| Item::recycle("pictures", "The pictures our player made of it", vec![f], Vec::new())));
    items.extend(crate::screenshots::folder(&t.stem).and_then(|f| Item::recycle("screenshots", "Its screenshots", vec![f], Vec::new())).map(|i| Item { ticked: false, ..i }));
    if let Some(p) = st.user.played.get(&key(&t.path)).filter(|p| p.count > 0) {
        items.push(Item::own("play-log", "Its play log", format!("{} {}", p.count, if p.count == 1 { "play" } else { "plays" }), false));
    }
    Ok(items)
}

/// The ticked of what removing a table could take carried out (`library::remove::carry_out`), the
/// record of its plays forgotten if ticked; what was done.
pub fn remove(st: &mut State, id: &str, keys: &BTreeSet<String>) -> Result<::library::remove::Done, String> {
    let items = remove_plan(st, id)?;
    let path = st.tables.iter().find(|t| t.id == id).map(|t| t.path.clone()).ok_or("That table isn't in the library any more.")?;
    let kept = data_folder().ok_or("no APPDATA folder")?.join("Library").join("Kept");
    let done = ::library::remove::carry_out(&items, keys, &kept, &stamp())?;
    if keys.contains("play-log") && st.user.played.remove(&key(&path)).is_some() {
        st.user.save()?;
    }
    Ok(done)
}

/// A table renamed by "Overwrite for all players" given its names back, the user's own moved back.
/// Its ID then.
pub fn put_names_back(st: &mut State, id: &str) -> Result<String, String> {
    let layout = st.layout.clone().ok_or("The library isn't read yet.")?;
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let now = t.path.clone();
    let r = st.user.renames.get(&key(&now)).cloned().ok_or("It wasn't renamed here.")?;
    ::library::rename::put_back(&layout, &r)?;
    let back = now.with_file_name(format!("{}.{}", r.from, now.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| "vpx".into())));
    move_own(&mut st.user, &now, &back, &r.to, &r.from);
    move_folders(&now, &back);
    st.user.renames.remove(&key(&now));
    st.user.save()?;
    Ok(id_of(&back))
}

/// A table's ROM choices for its Change window: the set its script asks for now, the sets known for
/// it (VPS's for its game, those of the same game here), every set here, and the change made
/// before, to go back on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomChoices {
    pub current: String,
    pub known: Vec<::library::rom_change::Candidate>,
    pub all: Vec<String>,
    pub changed: Option<::library::rom_change::Changed>,
}

pub fn rom_choices(shared: &Shared, id: &str) -> Result<RomChoices, String> {
    let (current, roms, on_vps, changed) = {
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let current = t.read.game_name.clone().filter(|_| t.read.pinmame).ok_or("Its script drives no machine.")?;
        let layout = st.layout.as_ref().ok_or("The library isn't read yet.")?;
        let on_vps: Vec<String> = st
            .db
            .as_deref()
            .zip(t.on_vps.game.as_deref())
            .and_then(|(db, g)| db.game(g))
            .map(|g| g.rom_files.iter().filter_map(|r| r.version.clone()).collect())
            .unwrap_or_default();
        // the ROMs PinMAME plays it from: its own `pinmame\roms`, where it has one, else VPinMAME's
        let own = t.path.parent().map(|d| d.join("pinmame").join("roms")).filter(|d| d.is_dir());
        (current, own.unwrap_or_else(|| layout.vpinmame.join("roms")), on_vps, st.user.rom_changes.get(&key(&t.path)).cloned())
    };
    // PinMAME asked each set's name, the library free meanwhile
    Ok(RomChoices { known: ::library::rom_change::candidates(&roms, &current, &on_vps), all: ::library::rom_change::installed(&roms), changed, current })
}

/// What changing a table's ROM to `to` changes, in words, said before it's done.
pub fn rom_effects(st: &State, id: &str, to: &str) -> Result<Vec<String>, String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let layout = st.layout.as_ref().ok_or("The library isn't read yet.")?;
    let from = t.read.game_name.clone().unwrap_or_default();
    let mut out = vec![if t.holds.script.is_some() {
        format!("The script beside it, {}.vbs, is changed to ask PinMAME for {to} in place of {from}; the one there now is kept beside it, to go back to.", t.stem)
    } else {
        format!("A script of its own is put beside it, {}.vbs: its own script, asking PinMAME for {to} in place of {from}. VPX and our player play it in its own's place; the table file isn't changed.", t.stem)
    }];
    let here = |dir: &str| layout.vpinmame.join(dir).join(to).is_dir();
    if t.holds.altsound.is_some() && !here("altsound") {
        out.push(format!("Its AltSound pack is {from}'s: {to} has none, so the machine's own sound plays."));
    }
    if t.holds.colour.is_some() && !here("altcolor") {
        out.push(format!("Its coloring is {from}'s: {to} has none, so the display shows in one color."));
    }
    if t.holds.pup_pack.as_ref().is_some_and(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&from))) && t.read.pup_names.iter().any(|n| n.eq_ignore_ascii_case(&from)) {
        out.push(format!("Its PinUP pack is looked for by the ROM's name: under {to}, its pack, named {from}, isn't found."));
    }
    out.push(format!("The machine keeps its memory — high scores and settings — for each ROM: {to} starts with its own."));
    Ok(out)
}

/// A table's ROM changed to `to`, the change kept to go back on.
pub fn change_rom(st: &mut State, id: &str, to: &str) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let from = t.read.game_name.clone().ok_or("Its script names no ROM.")?;
    if from.eq_ignore_ascii_case(to) {
        return Ok(());
    }
    let script = ::library::table::script_that_plays(&t.path)?;
    let mut changed = ::library::rom_change::change(&t.path, &script, &from, to)?;
    let key = key(&t.path);
    // changed again: going back still goes back to before the first change (the script the first
    // change made isn't kept)
    if let Some(first) = st.user.rom_changes.get(&key) {
        if first.made
            && let Some(kept) = &changed.before
        {
            let _ = std::fs::remove_file(kept);
        }
        changed = ::library::rom_change::Changed { from: first.from.clone(), to: to.to_string(), made: first.made, before: first.before.clone() };
    }
    st.user.rom_changes.insert(key, changed);
    st.user.save()
}

/// A table's script saved beside it from the script editor, what was changed remembered.
pub fn save_script(st: &mut State, id: &str, text: &str, crlf: bool) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    let rom = st.user.rom_changes.get(&key).map(|c| (c.from.clone(), c.to.clone()));
    let edit = crate::script_editor::save(&t.path, text, crlf, rom.as_ref().map(|(f, to)| (f.as_str(), to.as_str())))?;
    st.user.script_edits.insert(key.clone(), edit);
    st.script_updates.remove(&key);
    st.user.save()
}

/// A table's own script put back: the one beside it to the Recycle Bin, with what was remembered of
/// it (a ROM changed on its page lived in it too).
pub fn put_back_script(st: &mut State, id: &str) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    let file = crate::script_editor::beside(&t.path);
    if file.exists() {
        ::library::install::to_recycle_bin(&file)?;
    }
    st.user.script_edits.remove(&key);
    st.user.rom_changes.remove(&key);
    st.script_updates.remove(&key);
    st.user.save()
}

/// A table updated since its script was edited here: its changed options carried over to its new
/// script (`carry`), what can't be said, the script before to the Recycle Bin; or its new script
/// played as it is. What was done, in words.
pub fn resolve_script_update(st: &mut State, id: &str, carry: bool) -> Result<String, String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let path = t.path.clone();
    let key = key(&path);
    if !carry {
        put_back_script(st, id)?;
        return Ok("It plays its new script as it is; the script before is in the Recycle Bin.".into());
    }
    let edit = st.user.script_edits.get(&key).cloned().ok_or("Nothing was changed in its script here.")?;
    let rom = st.user.rom_changes.get(&key).map(|c| (c.from.clone(), c.to.clone()));
    let carried = crate::script_editor::carry(&path, &edit, rom.as_ref().map(|(f, to)| (f.as_str(), to.as_str())))?;
    let file = crate::script_editor::beside(&path);
    if file.exists() {
        ::library::install::to_recycle_bin(&file)?;
    }
    std::fs::write(&file, carried.script.as_bytes()).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut said = format!("Carried over to its new script: {} option{}", edit.options.len() - carried.dropped.iter().filter(|d| edit.options.contains_key(*d)).count(), if edit.options.len() == 1 { "" } else { "s" });
    if !carried.dropped.is_empty() {
        said.push_str(&format!("; not carried, as the new script has no such line: {}", carried.dropped.join(", ")));
    }
    if edit.other {
        said.push_str(". Your other changes to the whole script weren't carried over: the script before is in the Recycle Bin");
    }
    said.push('.');
    // what's remembered now: the new script as its base, with the options carried
    let (size, modified) = crate::script_editor::stamp(&path);
    let base = ::library::table::own_script(&path).map(|own| ::library::script_edit::digest(&own)).unwrap_or_default();
    let options = edit.options.into_iter().filter(|(n, _)| !carried.dropped.contains(n)).collect();
    st.user.script_edits.insert(key.clone(), crate::script_editor::ScriptEdit { base, size, modified, options, other: false });
    if rom.is_some() && carried.dropped.iter().any(|d| d.starts_with("its ROM")) {
        st.user.rom_changes.remove(&key);
    }
    st.script_updates.remove(&key);
    st.user.save()?;
    Ok(said)
}

/// A table's ROM put back as it was before it was changed here.
pub fn undo_rom(st: &mut State, id: &str) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    let changed = st.user.rom_changes.get(&key).cloned().ok_or("Its ROM wasn't changed here.")?;
    ::library::rom_change::undo(&t.path, &changed, &::library::install::to_recycle_bin)?;
    st.user.rom_changes.remove(&key);
    st.user.save()
}

/// Marks a table's play: when, and how many times. When it started, in ms since 1970.
pub fn record_play(st: &mut State, path: &Path) -> i64 {
    let at = now_ms();
    let entry = st.user.played.entry(key(path)).or_default();
    entry.last = at;
    entry.count += 1;
    if let Err(e) = st.user.save() {
        eprintln!("the library's own wasn't kept: {e}");
    }
    at
}

/// A play's end: how long it lasted and its scores, kept in the table's play log.
pub fn record_end(st: &mut State, path: &Path, at: i64, scores: Vec<u64>) {
    let seconds = (now_ms() - at).max(0) as u64 / 1000;
    let entry = st.user.played.entry(key(path)).or_default();
    entry.seconds += seconds;
    entry.log.push(Play { at, seconds, scores });
    if entry.log.len() > PLAY_LOG {
        let over = entry.log.len() - PLAY_LOG;
        entry.log.drain(..over);
    }
    if let Err(e) = st.user.save() {
        eprintln!("the play wasn't kept: {e}");
    }
}

/// A picture or video of a table's, on its page's right-click: one of PinUP
/// Popper's media or ours sent to the Recycle Bin; one of VPS's, which isn't the user's to delete,
/// hidden from the table's page. What became of it: `deleted` or `hidden`.
pub fn delete_media(st: &mut State, id: &str, url: &str) -> Result<&'static str, String> {
    let rest = url.strip_prefix(pictures::base()).ok_or("That isn't one of the table's pictures.")?;
    let rest = rest.split('?').next().unwrap_or_default();
    let t = st.tables.iter_mut().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        ["t", table, n] if *table == id => {
            let n: usize = n.parse().map_err(|_| "That isn't one of the table's pictures.")?;
            let m = t.holds.media.get(n).ok_or("That picture isn't there any more.")?;
            ::library::install::to_recycle_bin(&m.path)?;
            t.holds.media.remove(n);
            Ok("deleted")
        }
        ["o", table, kind] if *table == id => {
            let (path, _) = crate::own_pictures::made(id, kind).ok_or("That picture isn't there any more.")?;
            ::library::install::to_recycle_bin(&path)?;
            Ok("deleted")
        }
        ["v" | "g", picture] => {
            st.user.hidden.entry(key(&t.path)).or_default().insert((*picture).to_string());
            st.user.save()?;
            Ok("hidden")
        }
        ["s", table, name] if *table == id => {
            let path = crate::screenshots::file(&t.stem, name).ok_or("That screenshot isn't there any more.")?;
            ::library::install::to_recycle_bin(&path)?;
            Ok("deleted")
        }
        _ => Err("That isn't one of the table's pictures.".into()),
    }
}

/// VPS's pictures hidden from a table's page shown on it again.
pub fn unhide_media(st: &mut State, id: &str) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    if st.user.hidden.remove(&key).is_some() {
        st.user.save()?;
    }
    Ok(())
}

/// The picture or video of a kind used for a table where it has several (a
/// blue check on the one used, brought to the front of its stack): its `MediaView::pick`.
pub fn choose_media(st: &mut State, id: &str, kind: &str, pick: &str) -> Result<(), String> {
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    st.user.chosen.entry(key).or_default().insert(kind.to_string(), pick.to_string());
    st.user.save()
}

/// A picture or video turned on the user's word — a front end's kept on its side the other way, or
/// any turned by quarters (the right-click on a table's page): 0, 90, 180 or 270 degrees clockwise
/// (`MediaView::turn`), kept for it, in VPI alone (the file isn't touched).
pub fn turn_media(st: &mut State, id: &str, pick: &str, turn: u16) -> Result<(), String> {
    if ![0, 90, 180, 270].contains(&turn) {
        return Err("A picture is turned by quarters: 0, 90, 180 or 270 degrees.".into());
    }
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = key(&t.path);
    st.user.turns.entry(key).or_default().insert(pick.to_string(), turn);
    st.user.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tables_details_kept_tidy() {
        let d = Details {
            name: Some("  Attack from Mars LE ".into()),
            maker: Some("   ".into()),
            year: Some(95),
            badge: Some("XX".into()),
            version: None,
            authors: Some(vec![" JPSalas".into(), "".into()]),
        }
        .tidied();
        assert_eq!(d, Details { name: Some("Attack from Mars LE".into()), authors: Some(vec!["JPSalas".into()]), ..Details::default() });
        assert_eq!(kind_named("EM").as_deref(), Some("Electro-mechanical"));
        assert_eq!(Details { maker: Some("".into()), ..Details::default() }.tidied(), Details::default());
    }

    #[test]
    fn a_renamed_tables_own_follow_it() {
        let from = Path::new(r"C:\Tables\Attack from Mars (Bally 1995) JPSalas 6.0.0.vpx");
        let to = Path::new(r"C:\Tables\Attack from Mars (Bally 1995).vpx");
        let (a, b) = (key(from), key(to));
        let mut user = User::default();
        user.favourites.insert(a.clone());
        user.played.insert(a.clone(), Played { count: 2, ..Default::default() });
        user.ratings.insert(a.clone(), 4);
        user.ratings_unsent.insert(a.clone());
        user.playlists.push(Playlist { id: "p1".into(), name: "Friday".into(), tables: vec![r"c:\other.vpx".into(), a.clone()] });
        user.chosen.insert(a.clone(), BTreeMap::from([("wheel".to_string(), "file:attack from mars (bally 1995) jpsalas 6.0.001.png".to_string()), ("playfield".to_string(), "vps:abc".to_string())]));
        move_own(&mut user, from, to, "Attack from Mars (Bally 1995) JPSalas 6.0.0", "Attack from Mars (Bally 1995)");
        assert!(user.favourites.contains(&b) && !user.favourites.contains(&a));
        assert_eq!(user.played.get(&b).map(|p| p.count), Some(2));
        assert_eq!(user.ratings.get(&b), Some(&4));
        assert!(user.ratings_unsent.contains(&b) && !user.ratings_unsent.contains(&a));
        assert_eq!(user.playlists[0].tables, vec![r"c:\other.vpx".to_string(), b.clone()]);
        let picks = &user.chosen[&b];
        assert_eq!(picks["wheel"], "file:attack from mars (bally 1995)01.png");
        assert_eq!(picks["playfield"], "vps:abc");
    }

    #[test]
    fn ratings_ours_over_the_front_ends_and_shared_as_vpidb_counts() {
        let root = Path::new(r"C:\Pinball\Visual Pinball");
        let mut layout = Layout::under(root);
        layout.tables = root.join("Tables");
        let table = |file: &str, game: Option<&str>, table_file: Option<&str>, sure: bool| Table {
            id: file.into(),
            path: layout.tables.join(file),
            stem: Path::new(file).file_stem().unwrap().to_string_lossy().into_owned(),
            read: Read::default(),
            on_vps: OnVps { game: game.map(str::to_string), table_file: table_file.map(str::to_string), found: None, sure },
            holds: Holds::default(),
            added: 0,
            other: Vec::new(),
        };
        let mut st = State { layout: Some(layout.clone()), ..Default::default() };
        st.tables = vec![table("A.vpx", Some("g1"), Some("f1"), true), table("B.vpx", Some("g9"), None, false), table(r"C\C.vpx", Some("g2"), None, true)];
        st.front_ratings.insert("c\\c".into(), ::library::ratings::Rating { stars: 3, from: ::library::frontends::Front::Popper });
        st.user.ratings.insert(key(&st.tables[0].path), 5);
        // ours over none, a front end's where ours is none
        assert_eq!(rating_of(&st, &st.tables[0]), Some((5, "ours")));
        assert_eq!(rating_of(&st, &st.tables[1]), None);
        assert_eq!(rating_of(&st, &st.tables[2]), Some((3, "Popper")));
        // for VPIDB: the sure matches alone, with their ratings
        assert_eq!(
            serde_json::Value::from(vpidb_library(&st)),
            serde_json::json!([{ "game": "g1", "file": "f1", "rating": 5 }, { "game": "g2", "file": null, "rating": 3 }])
        );
        // cleared here, over the front end's
        st.user.ratings.insert(key(&st.tables[2].path), 0);
        assert_eq!(rating_of(&st, &st.tables[2]), None);
        assert!(!ratings_to_fronts(&serde_json::Value::Null));
        assert!(ratings_to_fronts(&serde_json::json!({ "ratingsToFronts": true })));
    }

    #[test]
    fn names_from_file_names() {
        assert_eq!(from_file_name("Attack from Mars (Bally 1995) JPSalas 6.0.0"), ("Attack from Mars".into(), Some("Bally".into()), Some(1995)));
        assert_eq!(from_file_name("Medieval Madness (Williams 1997) VPW v1.0.1"), ("Medieval Madness".into(), Some("Williams".into()), Some(1997)));
        assert_eq!(from_file_name("Blood Machines 2.0"), ("Blood Machines 2.0".into(), None, None));
        assert_eq!(from_file_name("Bram Stokers Dracula (Data East 1993)"), ("Bram Stokers Dracula".into(), Some("Data East".into()), Some(1993)));
        assert_eq!(short_name("Indiana Jones - The Pinball Adventure"), "Indiana Jones");
        assert_eq!(short_name("Harry Potter and the Goblet of Fire"), "Harry Potter and the Goblet of Fire");
    }
}
