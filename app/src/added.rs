//! What the drop window put in place for each table, remembered (`Library\added.json` in the
//! player's data folder; `added.json` in the scratch layout's folder under `VPI_LAYOUT`), so a
//! table's page can show what was added for it that fits none of its other rows (a catch-all
//! "Other" section at the bottom of "What it holds").

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Added {
    /// Each table's files, by its table file's path (lower case).
    pub tables: BTreeMap<String, Vec<File>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub path: PathBuf,
    /// Its kind, as the drop window named it (`library::drop::kind_key`).
    pub kind: String,
    /// When it was added, in ms since 1970.
    pub at: i64,
}

fn path() -> Option<PathBuf> {
    match std::env::var_os("VPI_LAYOUT") {
        Some(root) => Some(PathBuf::from(root).join("added.json")),
        None => Some(crate::library::data_folder()?.join("Library").join("added.json")),
    }
}

/// What's remembered; nothing when it's missing or unreadable.
pub fn load() -> Added {
    path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// What was added for the table at `from` remembered for it at `to`, as a rename moves it.
pub fn moved(from: &Path, to: &Path) {
    let Some(path) = path() else { return };
    let mut added = load();
    let Some(files) = added.tables.remove(&crate::library::key(from)) else { return };
    added.tables.insert(crate::library::key(to), files);
    if let Ok(text) = serde_json::to_string_pretty(&added) {
        let part = path.with_extension("json.part");
        if std::fs::write(&part, text).is_ok() {
            let _ = std::fs::rename(&part, &path);
        }
    }
}

/// The files `placed` for the table at `table` remembered with what was added for it before, a file
/// added again taking its old entry's place.
pub fn remember(table: &Path, placed: &[(PathBuf, &str)]) -> Result<(), String> {
    if placed.is_empty() {
        return Ok(());
    }
    let path = path().ok_or("no APPDATA folder")?;
    let mut added = load();
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let files = added.tables.entry(crate::library::key(table)).or_default();
    for (p, kind) in placed {
        files.retain(|f| f.path != *p);
        files.push(File { path: p.clone(), kind: (*kind).to_string(), at });
    }
    // written beside its place first, then moved there, so a failed write leaves the old copy
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&added).map_err(|e| e.to_string())?;
    let part = path.with_extension("json.part");
    std::fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
}
