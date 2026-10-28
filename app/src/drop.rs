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
    /// The library's other releases of the machine it brings, by their table files' names: matched
    /// to the same game on VPS.
    same_machine: Vec<String>,
}

/// The library's tables matched to `game` on VPS, but the one named `stem`: their file names.
fn releases_of(st: &State, game: Option<&str>, stem: &str) -> Vec<String> {
    let Some(game) = game else { return Vec::new() };
    st.tables.iter().filter(|t| t.on_vps.sure && t.on_vps.game.as_deref() == Some(game) && !t.stem.eq_ignore_ascii_case(stem)).map(|t| t.stem.clone()).collect()
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
        .map(|d| View {
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
    if let Some(stem) = gone.table.as_ref().map(|t| t.stem.clone())
        && !w.downloads.iter().any(|d| d.table.as_ref().is_some_and(|t| t.stem.eq_ignore_ascii_case(&stem)))
        && !st.tables.iter().any(|t| t.stem.eq_ignore_ascii_case(&stem))
        && let Some(db) = st.db.clone()
    {
        let layout = st.layout.clone().unwrap_or_else(library::plan::Layout::reference);
        let brought_now = brought(&w.downloads);
        let library_known: Vec<Known> = known(&st).into_iter().map(|(_, k)| k).collect();
        let all_known = library::drop::known_with(&brought_now, &library_known);
        for d in w.downloads.iter_mut().filter(|d| d.table.is_none() && d.for_table.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(&stem))) {
            *d = library::drop::again(d, &db, &all_known, &brought_now, &layout);
        }
    }
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
                    for t in &added.tables {
                        let table = t.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let _ = app.emit("drop-progress", Progress { download: i, of, name: name.clone(), done: 0, total: 0, testing: Some(table.clone()) });
                        match crate::library::test_path(t) {
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
                results.push(Result_ { index: i, name: d.name.clone(), tables, added: Some(added), problem: None, tested });
            }
            Err(e) => results.push(Result_ { index: i, name: d.name.clone(), tables: Vec::new(), added: None, problem: Some(e), tested: Vec::new() }),
        }
    }
    // the library read again: the tables added, what they hold now
    crate::library::start(app.clone());
    Ok(results)
}

/// The drop window closed without adding.
pub fn forget(app: &AppHandle) {
    *app.state::<Dropped>().0.lock().unwrap() = Window::default();
}
