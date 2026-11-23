//! Play Hardcore in VPX: one game of a table as its author made it, whatever the user changed of
//! it — what makes it play softcore (`achievements::practice_because`) left out for that game:
//! its edited script (`<table>.vbs`) and its physics file (`<table>.vpp`), which VPX reads beside
//! it, set aside; its own settings file handed to VPX without its options and difficulty
//! (`-TableIni`, a copy, so nothing VPX keeps of the game reaches the table's own); VPX's physics
//! sets as VPX has them and the PinMAME add-on's cheat mode off (in the game's own copy of VPX's
//! settings, `play::run_changes`); the classic VPinMAME's cheat mode off for the machine. All of
//! it put back once the game's over — or, should the app have stopped meanwhile, when it next
//! starts (`put_back`, from `Hardcore\set aside.json` in the data folder).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vpi_settings::Change;

/// What a game played hardcore set aside, to put back.
#[derive(Default, Serialize, Deserialize)]
struct SetAside {
    /// Files moved aside: where each was, where it is.
    files: Vec<(PathBuf, PathBuf)>,
    /// The classic VPinMAME's cheat mode turned off for a machine: its ROM set, and its own value
    /// before (None: it had none of its own).
    cheat: Option<(String, Option<u32>)>,
}

fn record_file() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Hardcore").join("set aside.json"))
}

/// The name a file set aside takes beside where it was.
fn aside_of(file: &Path) -> PathBuf {
    let mut name = file.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".vpi-hardcore");
    file.with_file_name(name)
}

/// What playing `table` hardcore needs set aside, done: its table settings file for VPX
/// (`-TableIni`) where it has its own, and the changes to VPX's settings for the game — `settings`
/// being those of the VPX that plays. A script beside it that only changes its ROM
/// (`achievements::only_its_rom_changed`) stays: the table as its author made it, on the ROM chosen.
pub fn set_aside(table: &Path, rom: Option<&str>, keep_script: bool, settings: &[(String, String, String)]) -> Result<(Option<PathBuf>, Vec<Change>), String> {
    set_aside_in(&record_file().ok_or("no APPDATA folder")?, table, rom, keep_script, settings)
}

fn set_aside_in(record: &Path, table: &Path, rom: Option<&str>, keep_script: bool, settings: &[(String, String, String)]) -> Result<(Option<PathBuf>, Vec<Change>), String> {
    // anything a game before left aside put back first
    put_back_from(record)?;
    let mut aside = SetAside::default();
    let keep = |aside: &SetAside| -> Result<(), String> {
        if let Some(dir) = record.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(record, serde_json::to_vec_pretty(aside).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", record.display()))
    };
    // its edited script and its physics file, which VPX reads beside it
    for ext in ["vbs", "vpp"] {
        if ext == "vbs" && keep_script {
            continue;
        }
        let file = table.with_extension(ext);
        if file.is_file() {
            let to = aside_of(&file);
            aside.files.push((file.clone(), to.clone()));
            keep(&aside)?;
            if let Err(e) = std::fs::rename(&file, &to) {
                aside.files.pop();
                keep(&aside)?;
                return Err(format!("{} couldn't be set aside for the game: {e}", file.display()));
            }
        }
    }
    // the classic VPinMAME's cheat mode, off for the machine
    if let Some(rom) = rom
        && crate::machines::value(rom, "cheat") == Some(1)
    {
        aside.cheat = Some((rom.to_string(), crate::machines::own(rom, "cheat")));
        keep(&aside)?;
        crate::machines::apply(&[Change::set(rom, "cheat", 0)])?;
    }
    if aside.files.is_empty() && aside.cheat.is_none() {
        let _ = std::fs::remove_file(record);
    }
    // its own settings without its options and difficulty, a copy for the game
    let own = table.with_extension("ini");
    let table_ini = match std::fs::read(&own) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let entries = vpi_settings::parse_ini(&text);
            let out: Vec<Change> = entries
                .iter()
                .filter(|(s, k, _)| s.eq_ignore_ascii_case("TableOption") || (s.eq_ignore_ascii_case("TableOverride") && k.eq_ignore_ascii_case("Difficulty")))
                .map(|(s, k, _)| Change::remove(s, k))
                .collect();
            let dir = std::env::temp_dir().join("vpi-hardcore");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let copy = dir.join(own.file_name().ok_or("the table has no name")?);
            std::fs::write(&copy, vpi_settings::edit(&text, &out)).map_err(|e| format!("{}: {e}", copy.display()))?;
            Some(copy)
        }
        Err(_) => None,
    };
    // VPX's physics sets as VPX has them, and its PinMAME add-on's cheat mode off
    let mut changes: Vec<Change> = settings
        .iter()
        .filter(|(s, k, _)| s.eq_ignore_ascii_case("Player") && (k.starts_with("TablePhysics") || k.starts_with("FlipperPhysics")))
        .map(|(s, k, _)| Change::remove(s, k))
        .collect();
    if vpi_settings::get(settings, "Plugin.PinMAME", "Cheat").and_then(vpi_settings::vpx_bool) == Some(true) {
        changes.push(Change::set("Plugin.PinMAME", "Cheat", 0));
    }
    Ok((table_ini, changes))
}

/// What a game played hardcore set aside, put back: its files where they were (one made since in
/// its place kept, the one set aside then beside it), the machine's cheat mode as it was.
pub fn put_back() -> Result<(), String> {
    match record_file() {
        Some(record) => put_back_from(&record),
        None => Ok(()),
    }
}

fn put_back_from(record: &Path) -> Result<(), String> {
    let Ok(bytes) = std::fs::read(record) else { return Ok(()) };
    let aside: SetAside = serde_json::from_slice(&bytes).unwrap_or_default();
    let mut problems = Vec::new();
    for (was, now) in &aside.files {
        if !now.exists() {
            continue;
        }
        let back = if was.exists() { was.with_extension(format!("{}.before-hardcore", was.extension().and_then(|e| e.to_str()).unwrap_or(""))) } else { was.clone() };
        if let Err(e) = std::fs::rename(now, &back) {
            problems.push(format!("{} couldn't be put back: {e}", was.display()));
        }
    }
    if let Some((rom, before)) = &aside.cheat {
        let change = match before {
            Some(v) => Change::set(rom, "cheat", *v),
            None => Change::remove(rom, "cheat"),
        };
        if let Err(e) = crate::machines::apply(&[change]) {
            problems.push(e);
        }
    }
    if problems.is_empty() {
        let _ = std::fs::remove_file(record);
        Ok(())
    } else {
        Err(problems.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_set_aside_keeps_its_name_beside() {
        assert_eq!(aside_of(Path::new(r"C:\Tables\afm.vbs")), PathBuf::from(r"C:\Tables\afm.vbs.vpi-hardcore"));
    }

    #[test]
    fn a_tables_changes_set_aside_for_a_game_and_put_back() {
        let dir = std::env::temp_dir().join(format!("vpi-hardcore-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("Table.vpx");
        std::fs::write(&table, "table").unwrap();
        std::fs::write(table.with_extension("vbs"), "edited").unwrap();
        std::fs::write(table.with_extension("vpp"), "physics").unwrap();
        std::fs::write(dir.join("autovpp.vpp"), "every table's").unwrap();
        std::fs::write(table.with_extension("ini"), "[TableOption]\nBallColor = 2\n\n[TableOverride]\nDifficulty = 40\nExposure = 1.2\n\n[Player]\nBGSet = 1\n").unwrap();
        let record = dir.join("set aside.json");
        // a script that only changes its ROM stays
        let settings = vpi_settings::parse_ini("[Player]\nTablePhysicsGravityConstant0 = 2\n[Plugin.PinMAME]\nCheat = 1\n");
        let (_, changes) = set_aside_in(&record, &table, None, true, &settings).unwrap();
        assert!(table.with_extension("vbs").exists() && !table.with_extension("vpp").exists());
        // the physics set as VPX has it, the cheat mode off, in the settings of the VPX that plays
        assert_eq!(changes.len(), 2);
        put_back_from(&record).unwrap();
        assert!(table.with_extension("vpp").exists());
        let (table_ini, _) = set_aside_in(&record, &table, None, false, &[]).unwrap();
        // its script and physics file set aside; every table's physics file left
        assert!(!table.with_extension("vbs").exists() && !table.with_extension("vpp").exists());
        assert!(dir.join("autovpp.vpp").exists() && record.exists());
        // its settings for the game without its options and difficulty, its own file as it was
        let copy = std::fs::read_to_string(table_ini.unwrap()).unwrap();
        let e = vpi_settings::parse_ini(&copy);
        assert_eq!(vpi_settings::get(&e, "TableOption", "BallColor"), None);
        assert_eq!(vpi_settings::get(&e, "TableOverride", "Difficulty"), None);
        assert_eq!(vpi_settings::get(&e, "TableOverride", "Exposure"), Some("1.2"));
        assert_eq!(vpi_settings::get(&e, "Player", "BGSet"), Some("1"));
        assert!(std::fs::read_to_string(table.with_extension("ini")).unwrap().contains("Difficulty = 40"));
        // put back after the game; a script made meanwhile kept, the one set aside beside it
        std::fs::write(table.with_extension("vbs"), "new").unwrap();
        put_back_from(&record).unwrap();
        assert_eq!(std::fs::read_to_string(table.with_extension("vpp")).unwrap(), "physics");
        assert_eq!(std::fs::read_to_string(table.with_extension("vbs")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(dir.join("Table.vbs.before-hardcore")).unwrap(), "edited");
        assert!(!record.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
