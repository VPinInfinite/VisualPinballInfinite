//! Every table's ROM checked file by file in the background once the library is read (the tables
//! with problems marked in the library), as a table's page checks its own
//! (`library::roms::check_installed`): each set's verdict kept in `Library\rom-checks.json` with its
//! zip's size and date, and a clone's with its parent's zip's too (there or not), so a set is
//! checked again only when one of them changes. Shown as a background task while it runs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{self, Shared};

/// A set's verdict, with the size and date of the zip it's of; a clone's with its parent's zip's,
/// none (0, 0) where it isn't installed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Checked {
    size: u64,
    modified: i64,
    look: String,
    text: String,
    parent: Option<String>,
    parent_size: u64,
    parent_modified: i64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Kept {
    version: u32,
    /// By set, lower case.
    sets: BTreeMap<String, Checked>,
}

/// Raised when the checks are made differently (2: their words, and a clone's parent's zip).
const VERSION: u32 = 2;

fn kept_path() -> Option<PathBuf> {
    Some(library::data_folder()?.join("Library").join("rom-checks.json"))
}

fn load() -> Kept {
    kept_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Kept>(&t).ok())
        .filter(|k| k.version == VERSION)
        .unwrap_or_default()
}

fn save(kept: &Kept) {
    let Some(path) = kept_path() else { return };
    let text = match serde_json::to_string(kept) {
        Ok(t) => t,
        Err(_) => return,
    };
    let part = path.with_extension("json.part");
    if std::fs::write(&part, text).and_then(|_| std::fs::rename(&part, &path)).is_err() {
        eprintln!("the ROM checks weren't kept");
    }
}

fn stamp(zip: &Path) -> Option<(u64, i64)> {
    let meta = std::fs::metadata(zip).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some((meta.len(), modified))
}

static RUNNING: AtomicBool = AtomicBool::new(false);
static AGAIN: AtomicBool = AtomicBool::new(false);

/// The library's sets checked, those whose zips changed since; on a thread of its own, one run at
/// a time (a run asked for meanwhile follows it).
pub fn start(app: &AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        AGAIN.store(true, Ordering::SeqCst);
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("rom checks".into()).spawn(move || {
        loop {
            AGAIN.store(false, Ordering::SeqCst);
            run(&app);
            if !AGAIN.load(Ordering::SeqCst) {
                break;
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

fn run(app: &AppHandle) {
    // the test mode checks only when asked to (`--checks`)
    let testing = app.state::<Option<crate::shot::Shot>>().as_ref().is_some_and(|s| !s.checks);
    let (sets, folders, roms) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let Some(layout) = st.layout.as_ref() else { return };
        // each set in the folder its table keeps it in: its own `pinmame\roms`, else VPinMAME's
        let mut folders: std::collections::HashMap<String, std::path::PathBuf> = std::collections::HashMap::new();
        for t in &st.tables {
            if let (Some(zip), Some(set)) = (&t.holds.rom, &t.holds.rom_set) {
                folders.entry(set.to_lowercase()).or_insert_with(|| zip.parent().map(std::path::Path::to_path_buf).unwrap_or_default());
            }
        }
        let mut sets: Vec<String> = folders.keys().cloned().collect();
        sets.sort();
        (sets, folders, layout.vpinmame.join("roms"))
    };
    let folder_of = |set: &str| folders.get(set).cloned().unwrap_or_else(|| roms.clone());
    let mut kept = load();
    kept.version = VERSION;
    // a parent set beside its clone, else in VPinMAME's folder, as PinMAME looks
    let zip_stamp = |set: &str| stamp(&folder_of(set).join(format!("{set}.zip"))).or_else(|| stamp(&roms.join(format!("{set}.zip")))).unwrap_or_default();
    let fresh = |kept: &Kept, set: &str| {
        kept.sets.get(set).is_some_and(|c| (c.size, c.modified) == zip_stamp(set) && c.parent.as_deref().is_none_or(|p| (c.parent_size, c.parent_modified) == zip_stamp(p)))
    };
    let stale: Vec<String> = sets.iter().filter(|s| !fresh(&kept, s)).cloned().collect();
    // what's known shows at once
    tell(app, &kept, &sets, &stale);
    if stale.is_empty() || testing {
        return;
    }
    let task = crate::tasks::begin(app, "Checking ROMs", stale.len() as u32);
    for (n, set) in stale.iter().enumerate() {
        task.step(n as u32, Some(format!("Checking ROMs: {set}")));
        let (size, modified) = zip_stamp(set);
        let checked = match ::library::roms::installed_check(&folder_of(set), set) {
            Ok(c) => {
                let (parent_size, parent_modified) = c.parent.as_deref().map(zip_stamp).unwrap_or_default();
                Checked { size, modified, look: c.look.to_string(), text: c.text, parent: c.parent, parent_size, parent_modified }
            }
            Err(e) => Checked { size, modified, look: "warn".to_string(), text: e, ..Default::default() },
        };
        kept.sets.insert(set.clone(), checked);
        if (n + 1) % 10 == 0 {
            save(&kept);
            tell(app, &kept, &sets, &stale[n + 1..]);
        }
    }
    save(&kept);
    tell(app, &kept, &sets, &[]);
}

/// The verdicts known, but those still to check again, given to the library and the screens told.
fn tell(app: &AppHandle, kept: &Kept, sets: &[String], stale: &[String]) {
    let checks = sets
        .iter()
        .filter(|s| !stale.contains(s))
        .filter_map(|s| kept.sets.get(s).map(|c| (s.clone(), (c.look.clone(), c.text.clone()))))
        .collect();
    app.state::<Shared>().0.lock().unwrap().rom_checks = checks;
    let _ = app.emit("library-changed", ());
}
