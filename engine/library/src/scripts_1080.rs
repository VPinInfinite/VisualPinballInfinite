//! VPX 10.8.0's shared scripts for its games. VPX 10.8.0 and a VPX 10.8.1 build in one folder share
//! its `scripts` folder, and a 10.8.1 build brings shared scripts 10.8.0 can't run: its
//! `VPMKeys.vbs`, which every ROM table's machine script loads, reads each key through
//! `VPXActionKey`, which VPX 10.8.0 doesn't have ("Can't open vpmkeys.vbs", then "Subscript out of
//! range"). So for a game in 10.8.0, the shared scripts it was released with are lent beside the
//! table — the first place VPX 10.8.0 looks, its working folder being the table's as it plays —
//! for that game alone, and taken back after it, or as the app starts where it stopped during
//! one: neither VPX's own files change, and a 10.8.1 game never sees them.
//!
//! Where 10.8.0's come from: what putting a 10.8.1 build in kept of the folder's own (`Before
//! <tag>\scripts`, `vpx::install`), the set whose `VPMKeys.vbs` doesn't use `VPXActionKey`. Ours.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What a script needs of the newer VPX that VPX 10.8.0 doesn't have.
const NEWER_ONLY: &str = "VPXActionKey";

/// A file in `folder` named `name`, whatever the case of either (as VPX finds them).
fn named_in(folder: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(folder).ok()?.flatten().map(|e| e.path()).find(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// Whether `scripts` holds a set of shared scripts VPX 10.8.0 runs: its `core.vbs`, and a
/// `VPMKeys.vbs` that doesn't use what only the newer VPX has.
pub fn runs_in_1080(scripts: &Path) -> bool {
    named_in(scripts, "core.vbs").is_some()
        && named_in(scripts, "VPMKeys.vbs").and_then(|f| std::fs::read(f).ok()).is_some_and(|b| !String::from_utf8_lossy(&b).contains(NEWER_ONLY))
}

/// The build a `Before v10.8.1-<build>-<commit>` folder was kept for, for putting them in order.
fn build_of(folder: &Path) -> u32 {
    let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    name.split(['-', ' ']).find_map(|p| p.parse::<u32>().ok().filter(|n| *n > 100)).unwrap_or(u32::MAX)
}

/// VPX 10.8.0's shared scripts among what putting VPX builds in kept in `kept` (the app's `VPX`
/// folder): the oldest `Before …\scripts` VPX 10.8.0 runs.
pub fn kept_for_1080(kept: &Path) -> Option<PathBuf> {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(kept).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Before "))).collect();
    folders.sort_by_key(|f| build_of(f));
    folders.into_iter().map(|f| f.join("scripts")).find(|s| runs_in_1080(s))
}

/// The shared scripts a table's script loads, of those in `own` (10.8.0's set): each it names,
/// and each those name in turn (a machine's script naming `core.vbs` and `VPMKeys.vbs`), once.
pub fn loaded_by(script: &str, own: &Path) -> Vec<PathBuf> {
    let mut queue = crate::table::helper_scripts(script);
    let (mut seen, mut out): (Vec<String>, Vec<PathBuf>) = (Vec::new(), Vec::new());
    while let Some(name) = queue.pop() {
        if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            continue;
        }
        seen.push(name.clone());
        if let Some(file) = named_in(own, &name) {
            if let Ok(bytes) = std::fs::read(&file) {
                queue.extend(crate::table::helper_scripts(&String::from_utf8_lossy(&bytes)));
            }
            out.push(file);
        }
    }
    out
}

/// One script lent beside a table: where it lies, the file it's a copy of, and the copy's SHA-256,
/// telling it's still as lent whatever becomes of the file it came from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lent {
    pub to: PathBuf,
    pub from: PathBuf,
    pub sha256: String,
}

fn sha256_of(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// `scripts` (10.8.0's) lent into `table_folder` for a game, those of them VPX's own scripts
/// folder (`vpx_scripts`) holds the same left out, and any the table's folder holds of its own left
/// alone; what was lent kept in `record` first, so it's taken back even if the app stops during
/// the game. What's still lent from a game before is taken back first.
pub fn lend(record: &Path, table_folder: &Path, vpx_scripts: &Path, scripts: &[PathBuf]) -> Result<Vec<Lent>, String> {
    take_back(record)?;
    let mut lent = Vec::new();
    for from in scripts {
        let Some(name) = from.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        if named_in(table_folder, &name).is_some() {
            continue;
        }
        let bytes = std::fs::read(from).map_err(|e| format!("{}: {e}", from.display()))?;
        if named_in(vpx_scripts, &name).and_then(|f| std::fs::read(f).ok()).is_some_and(|theirs| theirs == bytes) {
            continue;
        }
        lent.push(Lent { to: table_folder.join(&name), from: from.clone(), sha256: sha256_of(&bytes) });
    }
    if lent.is_empty() {
        return Ok(lent);
    }
    if let Some(dir) = record.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(record, serde_json::to_vec_pretty(&lent).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", record.display()))?;
    for l in &lent {
        std::fs::copy(&l.from, &l.to).map_err(|e| format!("{}: {e}", l.to.display()))?;
    }
    Ok(lent)
}

/// What `record` says was lent taken back: each copy still as it was lent removed (one changed
/// since is left where it is), then the record.
pub fn take_back(record: &Path) -> Result<(), String> {
    let Ok(bytes) = std::fs::read(record) else { return Ok(()) };
    let lent: Vec<Lent> = serde_json::from_slice(&bytes).unwrap_or_default();
    for l in &lent {
        if std::fs::read(&l.to).is_ok_and(|there| sha256_of(&there) == l.sha256) {
            std::fs::remove_file(&l.to).map_err(|e| format!("{}: {e}", l.to.display()))?;
        }
    }
    std::fs::remove_file(record).map_err(|e| format!("{}: {e}", record.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_1080_game_gets_the_scripts_it_was_released_with_for_the_game_alone() {
        let root = std::env::temp_dir().join(format!("vpi-scripts-1080-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (kept, tables, vpx) = (root.join("kept"), root.join("Tables"), root.join("VPX").join("scripts"));
        // what putting builds in kept: 10.8.0's set first, then a newer build's, which 10.8.0 can't run
        let old = kept.join("Before v10.8.1-5436-af26b2d93").join("scripts");
        write(&old.join("alving.vbs"), "ExecuteGlobal GetTextFile(\"core.vbs\")\nExecuteGlobal GetTextFile(\"VPMKeys.vbs\")\n");
        write(&old.join("core.vbs"), "' core 3.61\nExecuteGlobal GetTextFile(\"ledcontrol.vbs\")\n");
        write(&old.join("VPMKeys.vbs"), "Const keyAddBall = 48\n");
        write(&old.join("ledcontrol.vbs"), "' the same in both\n");
        write(&old.join("wpc.vbs"), "' a machine this table doesn't use\n");
        let newer = kept.join("Before v10.8.1-6625-2ad1916b2").join("scripts");
        write(&newer.join("core.vbs"), "' core 3.62\n");
        write(&newer.join("VPMKeys.vbs"), "Dim keyAddBall: keyAddBall = VPXActionKey(22)\n");
        assert_eq!(kept_for_1080(&kept), Some(old.clone()));
        assert!(!runs_in_1080(&newer));
        // VPX's own: the newest build's, ledcontrol.vbs the same as 10.8.0's
        write(&vpx.join("alving.vbs"), "' 3.62\n");
        write(&vpx.join("core.vbs"), "' core 3.62\n");
        write(&vpx.join("VPMKeys.vbs"), "Dim keyAddBall: keyAddBall = VPXActionKey(22)\n");
        write(&vpx.join("ledcontrol.vbs"), "' the same in both\n");
        // the table names its machine's script and one only the newer VPX has; it keeps a script of its own
        let script = "LoadVPM \"02904245\", \"alvinG.VBS\", 3.10\nExecuteGlobal GetTextFile(\"controller.vbs\")\nExecuteGlobal GetTextFile(\"own.vbs\")\n";
        write(&tables.join("own.vbs"), "' the table's own\n");
        let needed = loaded_by(script, &old);
        let mut names: Vec<String> = needed.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["VPMKeys.vbs", "alving.vbs", "core.vbs", "ledcontrol.vbs"]);
        let record = root.join("lent.json");
        let lent = lend(&record, &tables, &vpx, &needed).unwrap();
        let mut lent_names: Vec<String> = lent.iter().map(|l| l.to.file_name().unwrap().to_string_lossy().into_owned()).collect();
        lent_names.sort();
        // ledcontrol.vbs is the same as VPX's own: not lent
        assert_eq!(lent_names, ["VPMKeys.vbs", "alving.vbs", "core.vbs"]);
        assert_eq!(std::fs::read_to_string(tables.join("VPMKeys.vbs")).unwrap(), "Const keyAddBall = 48\n");
        assert!(record.is_file());
        // a lent copy changed during the game stays; the rest go, and the table's own and VPX's are untouched
        write(&tables.join("core.vbs"), "' changed during the game\n");
        take_back(&record).unwrap();
        assert!(!tables.join("VPMKeys.vbs").exists() && !tables.join("alving.vbs").exists());
        assert!(tables.join("core.vbs").exists() && tables.join("own.vbs").exists());
        assert_eq!(std::fs::read_to_string(vpx.join("VPMKeys.vbs")).unwrap(), "Dim keyAddBall: keyAddBall = VPXActionKey(22)\n");
        assert!(!record.exists());
        // nothing lent, nothing recorded; taking back with no record is nothing
        std::fs::remove_file(tables.join("core.vbs")).unwrap();
        let none = lend(&record, &tables, &old, &needed).unwrap();
        assert!(none.is_empty() && !record.exists());
        take_back(&record).unwrap();
        // lent, then the kept set gone (the VPX before put back): the copies still go
        assert_eq!(lend(&record, &tables, &vpx, &needed).unwrap().len(), 3);
        std::fs::remove_dir_all(&kept).unwrap();
        take_back(&record).unwrap();
        assert!(!tables.join("core.vbs").exists() && !tables.join("VPMKeys.vbs").exists() && !tables.join("alving.vbs").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
