//! The PinUP packs, for Settings › PinUP packs and a table's page: every pack in PinUP's
//! `PUPVideos` and beside the tables (`pupvideos`, where the player looks first), the tables whose
//! scripts ask for each, its layouts and the one in place, and its check (`library::pup_pack`); a
//! layout applied on the user's word, its option file read and its copies made by the app itself,
//! never the file run; any pack put in the Recycle Bin, on the user's word; and a pack renamed for
//! a table to play it, kept so it can be put back. Each with its game on VPS: its tables' match, or for a pack no table plays, the game
//! its name names — its ROM's, as VPS lists each game's ROMs — and the tables of it installed.

use std::path::{Path, PathBuf};

use ::library::pup_pack::Layouts;
use ::library::vps::{Database, Game};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{self, Shared, State};

/// What a table's view carries of its pack: its check, and the layout in place.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// good, warn, bad, or none: an empty folder
    pub look: &'static str,
    pub text: String,
    /// The layout in place, by its name.
    pub layout: Option<String>,
    /// How many layouts it offers.
    pub layouts: usize,
    /// What's set up: `layout` (one of its layouts in place), `own` (its screens set up, by none
    /// of them), `unchosen` (it has layouts and none is in place, so nothing shows), or `none` (it
    /// offers none).
    pub setup: &'static str,
    /// Its check in a word or two, for a list: Complete, Empty folder, No layout chosen…
    pub state: &'static str,
}

/// The pack at `pack`'s facts, its layouts read, the one last chosen for it (`chosen`) counting
/// first where its files are in place.
pub fn facts(pack: &Path, chosen: Option<&str>) -> Facts {
    facts_of(pack, &::library::pup_pack::layouts_with(pack, chosen))
}

fn facts_of(pack: &Path, all: &Layouts) -> Facts {
    let ::library::pup_pack::Check { look, short, text } = ::library::pup_pack::check(pack, all);
    let setup = if all.layouts.is_empty() {
        "none"
    } else if all.current.is_some() {
        "layout"
    } else if pack.join("screens.pup").is_file() {
        "own"
    } else {
        "unchosen"
    };
    Facts { look, text, layout: all.in_place().map(|l| l.name.clone()), layouts: all.layouts.len(), setup, state: short }
}

/// A game on VPS, as a pack's row names it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLink {
    /// Its name, maker and year: "Last Action Hero (Data East 1993)".
    pub name: String,
    pub url: String,
}

fn link(g: &Game) -> GameLink {
    let made = [Some(g.manufacturer.clone()).filter(|m| !m.trim().is_empty()), g.year.map(|y| y.to_string())].into_iter().flatten().collect::<Vec<_>>().join(" ");
    GameLink { name: if made.is_empty() { g.name.clone() } else { format!("{} ({made})", g.name) }, url: format!("{}{}", ::library::vps::GAME_LINK, g.id) }
}

/// The game a pack's folder name names, on VPS: a ROM set's of VPS's lists (`lah_112`), else
/// another version's of the same ROM (`stwr_104` for the game whose ROMs are `stwr_…`), else a
/// game whose name starts with the folder's, letters alone (`Willy_Wonka`).
fn game_named<'a>(db: &'a Database, folder: &str) -> Option<&'a Game> {
    let low = folder.to_lowercase();
    let rom = |g: &&Game, f: &dyn Fn(&str) -> bool| g.rom_files.iter().any(|r| r.version.as_deref().is_some_and(f));
    if let Some(g) = db.games.iter().find(|g| rom(g, &|v| v.eq_ignore_ascii_case(&low))) {
        return Some(g);
    }
    let family = |s: &str| s.split_once('_').map(|(f, _)| f.to_lowercase());
    if let Some(f) = family(&low).filter(|f| f.len() >= 3)
        && let Some(g) = db.games.iter().find(|g| rom(g, &|v| family(v).as_deref() == Some(f.as_str())))
    {
        return Some(g);
    }
    let letters = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_lowercase();
    let mine = letters(folder);
    (mine.len() >= 6).then(|| db.games.iter().find(|g| letters(&g.name).starts_with(&mine))).flatten()
}

/// A pack, for the list and its Layout window.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    /// Its folder's name, which a table's script asks for.
    pub name: String,
    pub path: String,
    /// The tables it plays for: their IDs and names.
    pub tables: Vec<(String, String)>,
    /// There for those tables under a name their scripts don't ask for (renamed on their pages).
    pub misnamed: bool,
    /// PinUP Popper's own, its menu's: no table's, and kept.
    pub popper: bool,
    pub facts: Facts,
    pub layouts: Layouts,
    /// Its game on VPS, for its files there: its tables' match, or, for a pack no table plays, the
    /// game its name names (`guessed`).
    pub game: Option<GameLink>,
    pub guessed: bool,
    /// For a pack no table plays: the tables of its game installed, each with what it plays.
    pub game_tables: Vec<GameTable>,
    /// The table files dropped onto its row are for: its own, else one of its game's.
    pub drop_table: Option<String>,
    /// Its archives at its top, a pack not unpacked yet.
    pub archives: Vec<String>,
    /// Its name before the app renamed it for a table, where it did.
    pub renamed_from: Option<String>,
}

/// A table of a pack's game, for a pack no table plays: what it plays, in words, and the name its
/// script asks for where no pack's there by it, which renaming this pack to would make it its own.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTable {
    pub id: String,
    pub name: String,
    pub plays: String,
    pub rename_to: Option<String>,
}

/// Every pack folder: in PinUP's `PUPVideos`, and in a `pupvideos` folder beside the tables.
fn folders(st: &State) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = st.layout.iter().map(|l| l.pup_videos.clone()).collect();
    for t in &st.tables {
        if let Some(beside) = t.path.parent().map(|d| d.join("pupvideos"))
            && beside.is_dir()
            && !roots.iter().any(|r| library::key(r) == library::key(&beside))
        {
            roots.push(beside);
        }
    }
    let mut out: Vec<PathBuf> = roots.iter().flat_map(|r| std::fs::read_dir(r).into_iter().flatten().flatten()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    out.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default());
    out
}

/// The pack at `pack`, as the list shows it, with the tables it's for.
fn pack_of(st: &State, pack: &Path) -> Pack {
    let k = library::key(pack);
    let mut tables = Vec::new();
    let mut misnamed = false;
    for t in &st.tables {
        let own = t.holds.pup_pack.as_ref().is_some_and(|p| library::key(p) == k);
        let wrong = t.holds.pup_misnamed.as_ref().is_some_and(|p| library::key(p) == k);
        if own || wrong {
            misnamed |= wrong && !own;
            tables.push((t.id.clone(), library::view(st, t).name));
        }
    }
    tables.sort_by_key(|(_, n)| n.to_lowercase());
    let name = pack.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let popper = name.eq_ignore_ascii_case("PinUpMenu");
    let layouts = ::library::pup_pack::layouts_with(pack, st.user.pup_layouts.get(&k).map(String::as_str));
    let facts = facts_of(pack, &layouts);
    let db = st.db.as_deref();
    // its tables' game, where one's match is sure
    let game_of = |t: &library::Table| t.on_vps.sure.then(|| db.and_then(|db| ::library::installed::of(db, &t.on_vps).0)).flatten();
    let own_game = st.tables.iter().filter(|t| tables.iter().any(|(id, _)| *id == t.id)).find_map(game_of);
    // a pack no table plays: an installed table whose script asks for a name its folder's starts
    // with, or the other way round (`hpgof52`, a table asking for `hpgof`), else the game its name
    // names on VPS and that game's tables installed
    let low = name.to_lowercase();
    let unplayed = tables.is_empty() && !popper;
    let asked = unplayed
        .then(|| {
            st.tables.iter().find(|t| {
                t.read.pup_names.iter().any(|n| {
                    let n = n.to_lowercase();
                    n.len() >= 4 && (low.starts_with(&n) || n.starts_with(&low))
                })
            })
        })
        .flatten();
    let named = match asked {
        Some(t) => game_of(t),
        None => unplayed.then(|| db.and_then(|db| game_named(db, &name))).flatten(),
    };
    let row = |t: &library::Table| {
        let asks = t.read.pup_names.first().or(t.read.game_name.as_ref()).cloned();
        let (plays, rename_to) = match (t.holds.pup_pack.as_ref().and_then(|p| p.file_name()), asks) {
            (Some(own), _) => (format!("plays another pack, {}", own.to_string_lossy()), None),
            (None, Some(asks)) => (format!("asks for a pack named {asks}, which isn't here"), Some(asks)),
            (None, None) => ("plays no pack".to_string(), None),
        };
        GameTable { id: t.id.clone(), name: library::view(st, t).name, plays, rename_to }
    };
    let game_tables: Vec<GameTable> = match (asked, named) {
        (Some(t), _) => vec![row(t)],
        (None, Some(g)) => st.tables.iter().filter(|t| t.on_vps.sure && t.on_vps.game.as_deref() == Some(g.id.as_str())).map(row).collect(),
        (None, None) => Vec::new(),
    };
    let drop_table = tables.first().map(|(id, _)| id.clone()).or_else(|| game_tables.first().map(|g| g.id.clone()));
    let archives = ::library::pup_pack::archives(pack).iter().map(|a| a.display().to_string()).collect();
    let renamed_from = st.user.pup_renames.get(&k).cloned();
    Pack { game: own_game.or(named).map(link), guessed: own_game.is_none() && named.is_some(), game_tables, drop_table, archives, renamed_from, popper, facts, layouts, misnamed, tables, path: pack.display().to_string(), name }
}

/// Every pack, as the list shows them.
pub fn all(app: &AppHandle) -> Vec<Pack> {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    folders(&st).iter().map(|p| pack_of(&st, p)).collect()
}

/// One of the packs, by its folder, as the list finds it: never a folder the list doesn't hold.
fn known(st: &State, path: &str) -> Result<PathBuf, String> {
    let k = library::key(Path::new(path));
    folders(st).into_iter().find(|p| library::key(p) == k).ok_or_else(|| "That PinUP pack isn't there any more.".to_string())
}

/// One pack, as the list shows it.
pub fn one(app: &AppHandle, path: &str) -> Result<Pack, String> {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let pack = known(&st, path)?;
    Ok(pack_of(&st, &pack))
}

/// What applying a layout did, in words, for a notice.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Used {
    pub said: String,
    /// done, or problem where something couldn't be done.
    pub look: &'static str,
}

/// A layout of the pack applied: its option file read again and its copies, deletes and lines
/// made by the app, inside the pack's folder; a file of the user's it replaces or deletes put in
/// the Recycle Bin. Then its tables' views told.
pub fn use_layout(app: &AppHandle, path: &str, file: &str) -> Result<Used, String> {
    let pack = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        known(&st, path)?
    };
    let done = ::library::pup_pack::apply(&pack, file, &::library::install::to_recycle_bin)?;
    let after = ::library::pup_pack::layouts_with(&pack, Some(file));
    let name = after.layouts.iter().find(|l| l.file.eq_ignore_ascii_case(file)).map_or_else(|| ::library::pup_pack::name_of(file), |l| l.name.clone());
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        let k = library::key(&pack);
        // the choice kept, for telling apart two layouts whose files are the same
        if let Some(l) = after.layouts.iter().find(|l| l.file.eq_ignore_ascii_case(file)) {
            st.user.pup_layouts.insert(k.clone(), l.file.clone());
            if let Err(e) = st.user.save() {
                eprintln!("the layout chosen wasn't kept: {e}");
            }
        }
        if st.pup_checks.contains_key(&k) {
            st.pup_checks.insert(k, facts_of(&pack, &after));
        }
    }
    let _ = app.emit("library-changed", ());
    let mut said = format!("The pack now uses “{name}”.");
    if done.recycled > 0 {
        said.push_str(&format!(" {} {} it replaced went to the Recycle Bin.", done.recycled, if done.recycled == 1 { "file of yours" } else { "files of yours" }));
    }
    if done.problems.is_empty() {
        return Ok(Used { said, look: "done" });
    }
    said.push_str(&format!(" But {}", done.problems.join("; ")));
    Ok(Used { said, look: "problem" })
}

/// A layout of the pack written down for the player to try without applying it: the pack's folder,
/// then each file the layout would put at its top and where it would take it from
/// (`VPI_PUP_LAYOUT`, read by the player's PinUP). The file's path, in the player's data folder.
pub fn trying(app: &AppHandle, path: &str, file: &str) -> Result<PathBuf, String> {
    let pack = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        known(&st, path)?
    };
    let files = ::library::pup_pack::overlay(&pack, file)?;
    let mut text = format!("{}\n", pack.display());
    for (name, from) in files {
        text.push_str(&format!("{name}\t{}\n", from.display()));
    }
    let dir = library::data_folder().ok_or("no APPDATA folder")?.join("PinUP");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join("Trying a layout.txt");
    std::fs::write(&out, text).map_err(|e| format!("The layout couldn't be written down for the player: {e}"))?;
    Ok(out)
}

/// A pack no table plays renamed to the name a table's script asks for, on the user's word, so the
/// player finds it for that table; then the library read again, the table's pack its own.
pub fn rename(app: &AppHandle, path: &str, to: &str) -> Result<(), String> {
    let pack = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let pack = known(&st, path)?;
        let p = pack_of(&st, &pack);
        if !p.tables.is_empty() || !p.game_tables.iter().any(|g| g.rename_to.as_deref() == Some(to)) {
            return Err("That name isn't one a table here asks for.".into());
        }
        pack
    };
    let now = ::library::install::rename_pack(&pack, to)?;
    let from = pack.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    kept_rename(app, &pack, &now, &from);
    library::start(app.clone());
    Ok(())
}

/// A pack's rename kept, to put back: by its folder now, its name before (a rename of one renamed
/// here keeping the first name).
pub fn kept_rename(app: &AppHandle, before: &Path, now: &Path, name_before: &str) {
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    let first = st.user.pup_renames.remove(&library::key(before)).unwrap_or_else(|| name_before.to_string());
    st.user.pup_renames.insert(library::key(now), first);
    if let Err(e) = st.user.save() {
        eprintln!("the pack's rename wasn't kept: {e}");
    }
}

/// A pack renamed here put back to its name before, on the user's word; then the library read again.
pub fn undo_rename(app: &AppHandle, path: &str) -> Result<(), String> {
    let (pack, before) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let pack = known(&st, path)?;
        let before = st.user.pup_renames.get(&library::key(&pack)).cloned().ok_or("That pack wasn't renamed here.")?;
        (pack, before)
    };
    ::library::install::rename_pack(&pack, &before)?;
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        st.user.pup_renames.remove(&library::key(&pack));
        if let Err(e) = st.user.save() {
            eprintln!("the pack's rename wasn't forgotten: {e}");
        }
    }
    library::start(app.clone());
    Ok(())
}

/// A pack put in the Recycle Bin, on the user's word (the list asks first, saying which tables play
/// it); then the library read again.
pub fn remove(app: &AppHandle, path: &str) -> Result<(), String> {
    let pack = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        known(&st, path)?
    };
    ::library::install::to_recycle_bin(&pack).map_err(|e| format!("The pack couldn't be moved to the Recycle Bin: {e}"))?;
    library::start(app.clone());
    Ok(())
}

/// A pack's folder shown in Explorer, or one of its option files selected there: by the list's
/// folder and the file's own name, never a path the screens make up.
pub fn show(app: &AppHandle, path: &str, file: Option<&str>) -> Result<(), String> {
    let pack = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        known(&st, path)?
    };
    let at = match file {
        Some(f) if f.is_empty() || f.contains(['\\', '/', ':']) || f.starts_with('.') => return Err("That isn't one of the pack's files.".into()),
        Some(f) => pack.join(f),
        None => pack,
    };
    crate::shell::show(&at)
}
