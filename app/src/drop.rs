//! Dropping files in: a drop sorted out by the library's engine side (`library::drop`) — its
//! downloads, the tables they bring or are for, each file's place — kept while the drop window shows
//! it; a download sorted again for the table the user chooses; the ticked files copied into place
//! (`library::install`), the screens told how far it's come; the library read again after.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use library::drop::{Download, Known, Tie, places};
use library::install::{Added, Already, install_rows, to_recycle_bin};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{Shared, State};

/// The drop the drop window shows.
#[derive(Default)]
pub struct Dropped(Mutex<Window>);

#[derive(Default)]
struct Window {
    downloads: Vec<Download>,
    /// The library's table it was opened for, dropped on its tile or page: files dropped onto the
    /// window after are for it too.
    for_table: Option<String>,
}

/// One of the drop window's downloads as it shows it, by its place: for the pictures of its media
/// cards (`pictures::handle`'s `d` route).
pub fn download<R: tauri::Runtime>(app: &tauri::AppHandle<R>, n: usize) -> Option<Download> {
    app.state::<Dropped>().0.lock().unwrap().downloads.get(n).cloned()
}

/// The drop's backglasses and front-end media for one table settled across its downloads, so
/// several of a kind are all added (a 2-screen and a 3-screen backglass dropped together, two
/// downloads' wheels): the backglass made for the player's screens the one VPX plays — three where
/// VPX's settings give the score display a window of its own — the rest kept numbered.
/// And a table's missing ROM said again, every download the window shows counting: one dropped onto
/// it after its table takes the note away.
fn settle(downloads: &mut [Download], layout: &library::plan::Layout) {
    let ini = vpi_settings::parse_ini(&vpi_settings::vpx_settings_text());
    let three = vpi_settings::get(&ini, "ScoreView", "ScoreViewOutput") == Some("1");
    library::drop::backglasses_apart(downloads, three);
    // two downloads' media for one screen of one table: each kept, numbered as the front end numbers
    library::drop::media_apart(downloads);
    library::drop::note_missing_roms(downloads, layout);
}

/// How alike, in where its colours lie, a picture is to the table's own backglass to be taken for
/// it: pictures of one backglass's art come out 0.65 to 1, another table's at most 0.5.
const LIKE_ITS_BACKGLASS: f32 = 0.6;

/// The pictures of the library's tables' backglasses made small, by file, as each file was then
/// (its size and when it changed); and a drop's pictures, by download and step and size.
static BACKGLASSES: Mutex<Option<HashMap<PathBuf, (u64, std::sync::Arc<library::media_guess::Small>)>>> = Mutex::new(None);
static CARDS: Mutex<Option<HashMap<(PathBuf, usize, usize), Option<std::sync::Arc<library::media_guess::Small>>>>> = Mutex::new(None);

/// The library's table's backglass picture made small, as its backglass file keeps it.
fn backglass_small(file: &std::path::Path) -> Option<std::sync::Arc<library::media_guess::Small>> {
    let stamp = std::fs::metadata(file).ok().map(|m| m.len() ^ m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()))?;
    if let Some((s, small)) = BACKGLASSES.lock().unwrap().get_or_insert_with(HashMap::new).get(file)
        && *s == stamp
    {
        return Some(small.clone());
    }
    let text = std::fs::read(file).ok()?;
    let picture = library::backglass_dmd::picture_for(&text, library::backglass_dmd::Target::Backglass)?;
    let small = std::sync::Arc::new(library::media_guess::Small::of_bytes(&picture, 0)?);
    BACKGLASSES.lock().unwrap().get_or_insert_with(HashMap::new).insert(file.to_path_buf(), (stamp, small.clone()));
    Some(small)
}

/// A picture of a download nothing in its name places, likened to its table's own backglass, the
/// library's: one that looks like it guessed a backglass, to check (`why` `like`). Only a backglass:
/// a front end's playfield picture and a table's own playfield differ too much in where their
/// colours lie for one to be told by likeness, and a table being added has no backglass read yet.
fn liken(st: &State, downloads: &mut [Download]) {
    use library::media_guess::{BACKGLASS_GRIDS, MediaType, Small, alike, place_for};
    let layout = st.layout.clone().unwrap_or_else(library::plan::Layout::reference);
    for d in downloads.iter_mut() {
        let Some(stem) = d.for_table.as_deref() else { continue };
        let Some(file) = st.tables.iter().find(|t| t.stem.eq_ignore_ascii_case(stem)).and_then(|t| t.holds.backglass.clone()) else { continue };
        let unplaced = |m: &library::drop::MediaRow| m.kind == "picture" && (m.guess.is_none() || m.why == Some("shape"));
        if !d.rows.iter().any(|r| r.media.as_ref().is_some_and(unplaced)) {
            continue;
        }
        let Some(reference) = backglass_small(&file) else { continue };
        let keys: Vec<&'static str> = places(d, &layout).into_iter().map(|(k, _, _)| k).collect();
        let Some(place) = place_for("backglass", MediaType::Picture, &keys) else { continue };
        let (previews, source) = (d.previews.clone(), d.source.clone());
        for m in d.rows.iter_mut().filter_map(|r| r.media.as_mut()).filter(|m| unplaced(m)) {
            let Some(bytes) = previews.0.get(&m.step) else { continue };
            let key = (source.clone(), m.step, bytes.len());
            let small = CARDS.lock().unwrap().get_or_insert_with(HashMap::new).get(&key).cloned();
            let small = match small {
                Some(s) => s,
                None => {
                    let s = Small::of_bytes(bytes, 0).map(std::sync::Arc::new);
                    CARDS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, s.clone());
                    s
                }
            };
            if small.is_some_and(|s| alike(&s, &reference, &BACKGLASS_GRIDS) >= LIKE_ITS_BACKGLASS) {
                m.guess = Some(place);
                m.why = Some("like");
                m.what = Some("backglass");
            }
        }
    }
}

/// The ID the screens give a table being added in the drop, not in the library yet.
const NEW: &str = "new:";

/// The tables the downloads bring, as files are tied to them.
fn brought(downloads: &[Download]) -> Vec<Known> {
    downloads.iter().filter_map(|d| d.table.as_ref()).map(Known::from).collect()
}

/// A download as the drop window shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View<'a> {
    #[serde(flatten)]
    download: &'a Download,
    /// Where its unsorted files can be put: (the screens' key, in words).
    places: Vec<(&'static str, String)>,
    /// The table its files are for, by its ID: the library's, or one being added (`new:<stem>`).
    for_table_id: Option<String>,
    /// The library's table of the same file it brings, if it's installed already.
    installed_id: Option<String>,
    /// The library's other releases of the machine it brings, matched to the same game on VPS: each
    /// one's ID and table file's name, for "Add it as a build" or "Replace it".
    same_machine: Vec<(String, String)>,
    /// Where its media cards' pictures are served, a card's step after it.
    media_base: String,
    /// The download as it is (`media_key`), after a card's address: a later drop's cards, numbered
    /// from the first again, never show the pictures of the drop before.
    media_key: String,
}

/// A download as it is, for its media cards' addresses: its place on the PC, its length and when it
/// last changed (the screens keep a picture for as long as its address stays the same).
fn media_key(d: &Download) -> String {
    let Some(listing) = d.listing.as_ref() else { return String::new() };
    let path = match &listing.source {
        library::archive::Source::Folder(p) | library::archive::Source::File(p) => p.clone(),
        library::archive::Source::Archive(a) => a.path().to_path_buf(),
    };
    crate::media_facts::address_stamp(&path)
}

/// The library's tables matched to `game` on VPS, but the one named `stem`: their IDs and file names.
fn releases_of(st: &State, game: Option<&str>, stem: &str) -> Vec<(String, String)> {
    let Some(game) = game else { return Vec::new() };
    st.tables.iter().filter(|t| t.on_vps.sure && t.on_vps.game.as_deref() == Some(game) && !t.stem.eq_ignore_ascii_case(stem)).map(|t| (t.id.clone(), t.stem.clone())).collect()
}

/// The library's tables as a drop's files are tied to them.
fn known(st: &State) -> Vec<(String, Known)> {
    st.tables
        .iter()
        .map(|t| {
            (
                t.id.clone(),
                Known {
                    stem: t.stem.clone(),
                    game_name: t.read.game_name.clone(),
                    pup_names: t.read.pup_names.clone(),
                    vps_game: t.on_vps.game.clone().filter(|_| t.on_vps.sure),
                    helpers: t.read.helpers.clone(),
                    music: t.read.music.clone(),
                    file: Some(t.path.clone()),
                },
            )
        })
        .collect()
}

fn views(st: &State, downloads: &[Download]) -> serde_json::Value {
    let layout = st.layout.clone().unwrap_or_else(library::plan::Layout::reference);
    let id_of = |stem: &str| st.tables.iter().find(|t| t.stem.eq_ignore_ascii_case(stem)).map(|t| t.id.clone());
    // a table being added: the library's of the same name, else its own
    let new_id = |stem: &str| id_of(stem).or_else(|| downloads.iter().any(|d| d.table.as_ref().is_some_and(|t| t.stem.eq_ignore_ascii_case(stem))).then(|| format!("{NEW}{stem}")));
    let out: Vec<View> = downloads
        .iter()
        .enumerate()
        .map(|(i, d)| View {
            media_base: format!("{}d/{i}/", crate::pictures::base()),
            media_key: media_key(d),
            download: d,
            places: places(d, &layout).into_iter().map(|(k, label, _)| (k, label)).collect(),
            for_table_id: d.for_table.as_deref().and_then(new_id),
            installed_id: d.table.as_ref().and_then(|t| id_of(&t.stem)),
            same_machine: d.table.as_ref().map(|t| releases_of(st, t.game_id.as_deref(), &t.stem)).unwrap_or_default(),
        })
        .collect();
    serde_json::to_value(out).unwrap_or_default()
}

/// A drop sorted out, kept for adding: `table_id`, the library's table it was dropped on; `join`,
/// dropped onto the drop window already open, its downloads joining those there — for the table the window was opened for, if it was, or tied to
/// the tables being added in it; those there tied to none sorted again when a table joins them.
pub fn inspect(app: &AppHandle, paths: Vec<PathBuf>, table_id: Option<String>, join: bool) -> Result<serde_json::Value, String> {
    let (db, layout, known) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let db = st.db.clone().ok_or("VPS's database isn't read yet: try again in a moment.")?;
        (db, st.layout.clone().unwrap_or_else(library::plan::Layout::reference), known(&st))
    };
    let (before, window_table) = {
        let dropped = app.state::<Dropped>();
        let mut w = dropped.0.lock().unwrap();
        if !join {
            *w = Window { downloads: Vec::new(), for_table: table_id.clone() };
        }
        (brought(&w.downloads), w.for_table.clone())
    };
    let find = |id: &str| known.iter().find(|(k, _)| k == id).map(|(_, k)| k.clone());
    let dropped_on = table_id.as_deref().and_then(find);
    let opened_for = if join && dropped_on.is_none() { window_table.as_deref().and_then(find) } else { None };
    let tie = match (&dropped_on, &opened_for) {
        (Some(k), _) => Tie::On(k),
        (None, Some(k)) => Tie::Joining(k),
        _ => Tie::Found,
    };
    let known: Vec<Known> = known.into_iter().map(|(_, k)| k).collect();
    let fresh = library::drop::inspect(&paths, &db, &known, &layout, tie, &before);
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let dropped = app.state::<Dropped>();
    let mut w = dropped.0.lock().unwrap();
    let tables_before = w.downloads.iter().filter(|d| d.table.is_some()).count();
    // a download dropped twice shows once
    for d in fresh {
        if !w.downloads.iter().any(|o| o.source == d.source) {
            w.downloads.push(d);
        }
    }
    // a table joined: what was tied to no table sorted again, to be tied to it
    if join && matches!(tie, Tie::Found) && w.downloads.iter().filter(|d| d.table.is_some()).count() > tables_before {
        let brought_now = brought(&w.downloads);
        let all_known = library::drop::known_with(&brought_now, &known);
        for d in w.downloads.iter_mut().filter(|d| d.table.is_none() && d.for_table.is_none() && d.error.is_none()) {
            *d = library::drop::again(d, &db, &all_known, &brought_now, &layout);
        }
    }
    settle(&mut w.downloads, &layout);
    liken(&st, &mut w.downloads);
    Ok(views(&st, &w.downloads))
}

/// A download of the drop sorted again for the table the user chose: the library's, or one being
/// added in the drop.
pub fn choose_table(app: &AppHandle, index: usize, table_id: String) -> Result<serde_json::Value, String> {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let db = st.db.clone().ok_or("VPS's database isn't read yet.")?;
    let layout = st.layout.clone().unwrap_or_else(library::plan::Layout::reference);
    let dropped = app.state::<Dropped>();
    let mut w = dropped.0.lock().unwrap();
    let table = match table_id.strip_prefix(NEW) {
        Some(stem) => brought(&w.downloads).into_iter().find(|k| k.stem.eq_ignore_ascii_case(stem)).ok_or("That table isn't in the drop any more.")?,
        None => known(&st).into_iter().find(|(id, _)| *id == table_id).map(|(_, k)| k).ok_or("That table isn't in the library any more.")?,
    };
    let d = w.downloads.get(index).ok_or("That download isn't in the drop any more.")?;
    let again = library::drop::with_table(d, &db, &layout, &table);
    w.downloads[index] = again;
    settle(&mut w.downloads, &layout);
    liken(&st, &mut w.downloads);
    Ok(views(&st, &w.downloads))
}

/// A download locked with a password opened with the one typed: the password tried first from then
/// on, the download sorted out again as when it was dropped — for the table the window was opened
/// for, else found by what it is — and the passwords kept once it opens it whole. Where it doesn't
/// open, the password is tried no more (unless it was kept before) and that's said.
pub fn unlock(app: &AppHandle, index: usize, password: String) -> Result<serde_json::Value, String> {
    // a password is as typed, spaces and all, but for the line's end a paste may bring
    let password = password.trim_end_matches(['\r', '\n']).to_string();
    if password.is_empty() {
        return Err("Type its password first.".into());
    }
    let (db, layout, known) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let db = st.db.clone().ok_or("VPS's database isn't read yet: try again in a moment.")?;
        (db, st.layout.clone().unwrap_or_else(library::plan::Layout::reference), known(&st))
    };
    let (source, before, window_table) = {
        let dropped = app.state::<Dropped>();
        let w = dropped.0.lock().unwrap();
        let d = w.downloads.get(index).ok_or("That download isn't in the drop any more.")?;
        (d.source.clone(), brought(&w.downloads), w.for_table.clone())
    };
    let kept_before = library::archive::passwords().contains(&password);
    library::archive::remember_password(&password);
    let table = window_table.as_deref().and_then(|id| known.iter().find(|(k, _)| k == id).map(|(_, k)| k.clone()));
    let tie = match &table {
        Some(k) => Tie::On(k),
        None => Tie::Found,
    };
    let known: Vec<Known> = known.into_iter().map(|(_, k)| k).collect();
    let fresh = library::drop::inspect(std::slice::from_ref(&source), &db, &known, &layout, tie, &before);
    let opened = !fresh.is_empty() && fresh.iter().all(|d| d.error.is_none());
    if !opened && !kept_before {
        library::archive::forget_password(&password);
    }
    if fresh.is_empty() || fresh.iter().any(|d| d.locked) {
        return Err("That password didn't open it.".into());
    }
    if opened && let Err(e) = crate::archive_passwords::save() {
        eprintln!("the passwords of locked downloads couldn't be kept: {e}");
    }
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let dropped = app.state::<Dropped>();
    let mut w = dropped.0.lock().unwrap();
    let at = w.downloads.iter().position(|d| d.source == source).ok_or("That download isn't in the drop any more.")?;
    w.downloads.splice(at..=at, fresh);
    settle(&mut w.downloads, &layout);
    liken(&st, &mut w.downloads);
    Ok(views(&st, &w.downloads))
}

/// A download taken out of the drop, its files left where they are: those tied to the table it
/// brought — when nothing else brings that table and the library has none of its name — sorted
/// again without it.
pub fn take_out(app: &AppHandle, index: usize) -> Result<serde_json::Value, String> {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let dropped = app.state::<Dropped>();
    let mut w = dropped.0.lock().unwrap();
    if index >= w.downloads.len() {
        return Err("That download isn't in the drop any more.".into());
    }
    let gone = w.downloads.remove(index);
    let layout = st.layout.clone().unwrap_or_else(library::plan::Layout::reference);
    if let Some(stem) = gone.table.as_ref().map(|t| t.stem.clone())
        && !w.downloads.iter().any(|d| d.table.as_ref().is_some_and(|t| t.stem.eq_ignore_ascii_case(&stem)))
        && !st.tables.iter().any(|t| t.stem.eq_ignore_ascii_case(&stem))
        && let Some(db) = st.db.clone()
    {
        let brought_now = brought(&w.downloads);
        let library_known: Vec<Known> = known(&st).into_iter().map(|(_, k)| k).collect();
        let all_known = library::drop::known_with(&brought_now, &library_known);
        for d in w.downloads.iter_mut().filter(|d| d.table.is_none() && d.for_table.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(&stem))) {
            *d = library::drop::again(d, &db, &all_known, &brought_now, &layout);
        }
    }
    settle(&mut w.downloads, &layout);
    liken(&st, &mut w.downloads);
    Ok(views(&st, &w.downloads))
}

/// What the user chose for one download: each row ticked or not, and where unsorted rows go, by row,
/// as the screens name the places.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub include: Vec<bool>,
    #[serde(default)]
    pub put: HashMap<usize, String>,
    /// What's done with what's there already, row by row (asked each time).
    #[serde(default)]
    pub already: HashMap<usize, Already>,
    /// A table's own settings file's windows kept as it sets them, row by row; else, as by default,
    /// its window settings left out as it's copied in, so the user's screens apply.
    #[serde(default)]
    pub keep_windows: HashMap<usize, bool>,
    /// The library's release of the same machine the table it brings replaces, by ID: that table
    /// file, and its own files beside it, to the Recycle Bin once the new one's in, the user's own
    /// for it carried over (else the new one is another build of the table).
    #[serde(default)]
    pub replaces: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    download: usize,
    of: usize,
    name: String,
    done: u64,
    total: u64,
    /// The table being tested, once its files are in (the full test of new tables).
    testing: Option<String>,
}

/// What was done with one download.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Result_ {
    /// Which of the drop's downloads it was.
    pub index: usize,
    pub name: String,
    /// The tables it put in place: for the library to show them when it's read again, scrolled to them.
    pub tables: Vec<TableAdded>,
    pub added: Option<Added>,
    pub problem: Option<String>,
    /// The tables it brought, tested (when the full test of new tables is on): each one's name and
    /// what its test came to.
    pub tested: Vec<(String, String, String)>,
    /// Its PinUP packs' layouts set to fit the screens, in words.
    pub layouts: Vec<String>,
}

/// A table a download put in place: its ID as the library will have it, its name, whether it's new
/// to the library, and whether VPS's match for it was sure.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableAdded {
    pub id: String,
    pub name: String,
    pub new: bool,
    pub sure: bool,
}

/// The ticked files of the drop copied into place, download by download, the screens told how far
/// it's come (`drop-progress`); each table brought tested when `test` asks (the full test of new
/// tables, a setting, off by default); the library read again after.
pub fn add(app: &AppHandle, choices: Vec<Choice>, already: Already, test: bool) -> Result<Vec<Result_>, String> {
    let downloads = std::mem::take(&mut *app.state::<Dropped>().0.lock().unwrap()).downloads;
    let (layout, had) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let had: std::collections::HashSet<String> = st.tables.iter().map(|t| crate::library::key(&t.path)).collect();
        (st.layout.clone().unwrap_or_else(library::plan::Layout::reference), had)
    };
    let of = downloads.len();
    let mut results = Vec::new();
    for (i, (d, choice)) in downloads.iter().zip(choices).enumerate() {
        if d.error.is_some() {
            continue;
        }
        let spots = places(d, &layout);
        let put: HashMap<usize, PathBuf> = choice.put.iter().filter_map(|(row, key)| spots.iter().find(|(k, _, _)| k == key).map(|(_, _, p)| (*row, p.clone()))).collect();
        let name = d.name.clone();
        let mut tell = |done: u64, total: u64| {
            let _ = app.emit("drop-progress", Progress { download: i, of, name: name.clone(), done, total, testing: None });
        };
        match install_rows(d, &choice.include, &put, already, &choice.already, &to_recycle_bin, &mut tell) {
            Ok(added) => {
                // a table's own settings file copied into its place: its window settings left out
                // unless the user kept them, so the user's screens apply
                for (k, row) in d.rows.iter().enumerate() {
                    let copied = row.state == "new" || choice.already.get(&k).copied().unwrap_or(already) == Already::Replace;
                    if row.windows.is_none() || !choice.include.get(k).copied().unwrap_or(false) || choice.keep_windows.get(&k).copied().unwrap_or(false) || !copied {
                        continue;
                    }
                    if let Some(file) = &row.to
                        && let Ok(bytes) = std::fs::read(file)
                    {
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        let left = vpi_settings::without_window_settings(&text);
                        if left != text
                            && let Err(e) = std::fs::write(file, left)
                        {
                            eprintln!("{}: its window settings couldn't be left out: {e}", file.display());
                        }
                    }
                }
                // a release it replaces: out once the new one's in, the user's own carried over
                let mut problem = None;
                if let (Some(old), Some(new)) = (choice.replaces.as_deref(), added.tables.first()) {
                    let shared = app.state::<Shared>();
                    let mut st = shared.0.lock().unwrap();
                    if let Err(e) = crate::library::replace_release(&mut st, old, new) {
                        problem = Some(format!("The release it replaces is still there: {e}"));
                    }
                }
                // what was put in place remembered for its table: the one the download brought, else
                // the one its files are named for (a table already there, the library's it was for)
                let table = added.tables.first().cloned().or_else(|| d.stem.as_ref().map(|stem| layout.tables.join(format!("{stem}.vpx"))));
                if let Some(table) = table
                    && let Err(e) = crate::added::remember(&table, &added.placed)
                {
                    eprintln!("what was added wasn't remembered: {e}");
                }
                let mut tested = Vec::new();
                if test {
                    // a Future Pinball table plays in Future Pinball: tests run VPX's
                    for t in added.tables.iter().filter(|t| !::library::future_pinball::is_table(t)) {
                        let table = t.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let _ = app.emit("drop-progress", Progress { download: i, of, name: name.clone(), done: 0, total: 0, testing: Some(table.clone()) });
                        match crate::library::test_path(Some(&layout), t) {
                            Ok(found) => {
                                crate::library::remember_test(app, t, &found);
                                tested.push((table, found.report.look.clone(), found.report.text.clone()));
                            }
                            Err(e) => tested.push((table, "warn".into(), e)),
                        }
                    }
                }
                let tables = added
                    .tables
                    .iter()
                    .map(|t| TableAdded {
                        id: crate::library::id_of(t),
                        name: d.table.as_ref().and_then(|f| f.game.clone()).unwrap_or_else(|| t.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                        new: !had.contains(&crate::library::key(t)),
                        sure: d.table.as_ref().is_some_and(|f| f.sure),
                    })
                    .collect();
                // a PinUP pack put in place: its layout set to fit the screens, as the preferences say
                let mut packs: Vec<PathBuf> = Vec::new();
                for (file, kind) in &added.placed {
                    let pack = file.ancestors().find(|a| a.parent().and_then(|p| p.file_name()).is_some_and(|n| n.eq_ignore_ascii_case("PUPVideos")));
                    if *kind == "pupPack"
                        && let Some(pack) = pack
                        && !packs.iter().any(|p| p == pack)
                    {
                        packs.push(pack.to_path_buf());
                    }
                }
                let layouts = packs.iter().filter_map(|p| crate::pup_packs::fit_new(app, p)).collect();
                results.push(Result_ { index: i, name: d.name.clone(), tables, added: Some(added), problem, tested, layouts });
                // a download from the panel's waiting folder: out of it, now it's in place
                crate::web_view::imported(app, &d.source);
            }
            Err(e) => results.push(Result_ { index: i, name: d.name.clone(), tables: Vec::new(), added: None, problem: Some(e), tested: Vec::new(), layouts: Vec::new() }),
        }
    }
    // the tables added, and those files were put in place for: each set to play a ROM of its game it
    // has, where the one its script asks for isn't here, once the library's read again
    let mut tables: Vec<PathBuf> = results.iter().flat_map(|r| r.added.iter().flat_map(|a| a.tables.iter().cloned())).collect();
    tables.extend(downloads.iter().filter_map(|d| d.stem.as_ref().map(|stem| d.table_dir.clone().unwrap_or_else(|| layout.tables.clone()).join(format!("{stem}.vpx")))));
    crate::library::fit_roms_after(tables);
    // the library read again: the tables added, what they hold now
    crate::library::start(app.clone());
    Ok(results)
}

/// The drop window closed without adding.
pub fn forget(app: &AppHandle) {
    *app.state::<Dropped>().0.lock().unwrap() = Window::default();
}
