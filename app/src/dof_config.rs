//! The cabinet's toys' configuration, where the player reads it (its `dof.rs`): a set of the DOF
//! Config Tool's files — `directoutputconfig*.ini`, one per output board, or DOF's own global
//! configuration — in a `directoutputconfig` folder in our settings folder, else in VPX's, the
//! first holding one, so a cabinet set up for VPX's DOF keeps working; and a set brought in, from
//! the Config Tool's download (its zip) or the folder of a set (DOF's own, or its `Config`
//! folder), copied into ours, a file it replaces going to the Recycle Bin. DOF's own folder, where
//! DOF itself is installed for VPX 10.8.0, is found to offer bringing it in: the player reads only
//! a `directoutputconfig` folder.

use std::io::Read;
use std::path::{Path, PathBuf};

/// Where the configuration is found.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// The folder the player reads it from.
    pub folder: String,
    /// Whose folder that is: ours, or VPX's.
    pub whose: &'static str,
    /// The configuration's files in it.
    pub files: usize,
}

/// What's known of the configuration.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Where {
    /// Where the player finds it, if anywhere.
    pub found: Option<Found>,
    /// DOF's own configuration folder, where DOF itself is installed, when it holds a set: to
    /// bring in.
    pub dof_own: Option<String>,
}

/// Where the configuration is, and DOF's own.
pub fn look() -> Where {
    Where { found: found(), dof_own: dof_own().map(|p| p.display().to_string()) }
}

/// DOF's own configuration folder at the places DOF is installed (`C:\DirectOutput`, or beside
/// VPX in `C:\vPinball`), when it holds a set.
fn dof_own() -> Option<PathBuf> {
    [r"C:\DirectOutput\Config", r"C:\vPinball\DirectOutput\Config"].into_iter().map(PathBuf::from).find(|d| holds_set(d))
}

fn appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// The Config Tool's file for an output board.
fn is_board_file(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("directoutputconfig") && n.ends_with(".ini")
}

/// A file of a set: the Config Tool's, or DOF's own beside them (its global configuration, the
/// cabinet's, the table mappings, an LED matrix's shapes and their picture).
fn is_config(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    is_board_file(&n) || n.ends_with(".xml") || n == "directoutputshapes.png"
}

/// Whether `dir` holds a set, as the player tells one: DOF's global configuration, or a board's file.
fn holds_set(dir: &Path) -> bool {
    dir.join("GlobalConfig_B2SServer.xml").is_file() || std::fs::read_dir(dir).is_ok_and(|entries| entries.flatten().any(|e| is_board_file(&e.file_name().to_string_lossy())))
}

/// Where the player finds the configuration: ours, else VPX's; none when neither holds a set.
pub fn found() -> Option<Found> {
    let appdata = appdata()?;
    [(appdata.join("Visual Pinball Infinite"), "ours"), (appdata.join("VPinballX"), "VPX")].into_iter().find_map(|(base, whose)| {
        let dir = base.join("directoutputconfig");
        holds_set(&dir).then(|| Found {
            files: std::fs::read_dir(&dir).map(|e| e.flatten().filter(|e| is_config(&e.file_name().to_string_lossy())).count()).unwrap_or(0),
            folder: dir.display().to_string(),
            whose,
        })
    })
}

/// A set brought in from `picked` — the Config Tool's download (a zip), or a file of a set, its
/// whole folder then (DOF's own folder holds its set in `Config`) — and copied into ours, which
/// the player reads first. Returns how many files came in.
pub fn import(picked: &Path) -> Result<usize, String> {
    let ours = appdata().ok_or("No APPDATA folder.")?.join("Visual Pinball Infinite").join("directoutputconfig");
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    if picked.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", picked.display());
        let mut zip = zip::ZipArchive::new(std::fs::File::open(picked).map_err(|e| fail(&e))?).map_err(|e| fail(&e))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| fail(&e))?;
            let name = entry.name().replace('\\', "/").rsplit('/').next().unwrap_or_default().to_string();
            if entry.is_dir() || !is_config(&name) {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| fail(&e))?;
            files.push((name, bytes));
        }
    } else {
        let folder = if picked.is_dir() { picked.to_path_buf() } else { picked.parent().ok_or("That file has no folder.")?.to_path_buf() };
        let folder = [folder.join("Config"), folder.join("directoutputconfig")].into_iter().find(|d| holds_set(d)).unwrap_or(folder);
        for entry in std::fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_file() && is_config(&name) {
                files.push((name.clone(), std::fs::read(entry.path()).map_err(|e| format!("{name}: {e}"))?));
            }
        }
    }
    if !files.iter().any(|(n, _)| is_board_file(n) || n.eq_ignore_ascii_case("GlobalConfig_B2SServer.xml")) {
        return Err("There's no DOF configuration in it: the DOF Config Tool's download holds a file for each output board (directoutputconfig30.ini and the like).".into());
    }
    std::fs::create_dir_all(&ours).map_err(|e| format!("{}: {e}", ours.display()))?;
    for (name, bytes) in &files {
        let to = ours.join(name);
        if to.is_file() {
            if std::fs::read(&to).is_ok_and(|old| &old == bytes) {
                continue;
            }
            ::library::install::to_recycle_bin(&to)?;
        }
        std::fs::write(&to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    Ok(files.len())
}
