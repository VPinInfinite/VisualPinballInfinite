//! The values a table's script keeps between plays (`SaveValue`, `LoadValue`): high scores,
//! credits, its own settings. Kept as VPX 5436 keeps them (`ScriptGlobalTable::SaveValue`): under
//! the table name and value name the script gives, matched in any case as VPX's storage names
//! are, as text — VPX's `BstrFromVariant`: `True` or `False` for a yes or no, else the value as
//! text — and handed back as text, an empty one for a value never saved. VPX's Windows build keeps
//! them in `VPReg.stg` in its User folder; ours in `VPReg.ini` beside the settings file, as VPX's
//! own build for other systems keeps them (a section for each table name, a line for each value),
//! written as each is saved, as VPX writes its file. Ours.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use vpi_settings::Change;

#[derive(Debug, Default)]
pub struct SavedValues {
    /// The file they're kept in; None: kept for the session only (test mode, the headless runner).
    file: Option<PathBuf>,
    /// By table name and value name, lower case.
    values: HashMap<(String, String), String>,
}

/// The file beside the settings file in use.
pub fn file_for(settings: Option<&Path>) -> Option<PathBuf> {
    let dir = match settings.and_then(Path::parent) {
        Some(d) => d.to_path_buf(),
        None => PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite"),
    };
    Some(dir.join("VPReg.ini"))
}

fn key(table: &str, name: &str) -> (String, String) {
    (table.to_lowercase(), name.to_lowercase())
}

impl SavedValues {
    /// The values kept in `file`, if any.
    pub fn load(file: Option<PathBuf>) -> SavedValues {
        let text = file.as_deref().and_then(|f| std::fs::read(f).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let mut values = HashMap::new();
        for (table, name, value) in vpi_settings::parse_ini(&text) {
            values.insert(key(&table, &name), value);
        }
        SavedValues { file, values }
    }

    /// A value as the script saved it; None when never saved.
    pub fn get(&self, table: &str, name: &str) -> Option<&str> {
        self.values.get(&key(table, name)).map(String::as_str)
    }

    /// A value saved, and written to the file at once; a problem writing it said, the value kept
    /// for the session all the same.
    pub fn set(&mut self, table: &str, name: &str, text: String) -> Result<(), String> {
        let text: String = text.chars().map(|c| if c == '\r' || c == '\n' { ' ' } else { c }).collect();
        if self.values.get(&key(table, name)) == Some(&text) {
            return Ok(());
        }
        self.values.insert(key(table, name), text.clone());
        let Some(file) = &self.file else { return Ok(()) };
        // a name that would break the file's lines or sections is kept for the session only
        if table.contains(['[', ']', '\r', '\n']) || name.contains(['=', '\r', '\n']) || name.trim().is_empty() {
            return Err(format!("`{table}`, `{name}`: kept for this session only, as the names can't be written in the file"));
        }
        let before = std::fs::read(file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let after = vpi_settings::edit(&before, &[Change::set(table, name, &text)]);
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut new_name = file.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        new_name.push(".new");
        let new = file.with_file_name(new_name);
        std::fs::write(&new, after).and_then(|_| std::fs::rename(&new, file)).map_err(|e| format!("{}: {e}", file.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_between_plays_in_any_case() {
        let dir = std::env::temp_dir().join(format!("vpi-saved-values-{}", std::process::id()));
        let file = dir.join("VPReg.ini");
        let mut v = SavedValues::load(Some(file.clone()));
        assert_eq!(v.get("BlackKnight", "HighScore1"), None);
        v.set("BlackKnight", "HighScore1", "1234500".into()).unwrap();
        v.set("BlackKnight", "HighScore1Name", "ABC".into()).unwrap();
        v.set("Bad Cats", "Credits", "3".into()).unwrap();
        // played again: read back, the names in any case
        let again = SavedValues::load(Some(file.clone()));
        let text = std::fs::read_to_string(&file).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(again.get("blackknight", "highscore1"), Some("1234500"));
        assert_eq!(again.get("BlackKnight", "HighScore1Name"), Some("ABC"));
        assert_eq!(again.get("Bad Cats", "Credits"), Some("3"));
        assert!(text.contains("[BlackKnight]") && text.contains("HighScore1 = 1234500"), "{text}");
        // the session alone: nothing written
        let mut session = SavedValues::load(None);
        session.set("T", "V", "1".into()).unwrap();
        assert_eq!(session.get("t", "v"), Some("1"));
    }
}
