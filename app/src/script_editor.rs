//! A table's script edited in the app: the script that plays — the one beside the table, else its
//! own — opened, its option lines listed (`library::script_edit`) and the whole of it shown; saved
//! beside the table as `<table>.vbs`, which VPX plays in its own's place, the table file
//! untouched; checked in VPX before it's kept (put beside the table for a run unseen and unheard,
//! what was there before put back after); opened in Notepad; or put back, the one beside it to the
//! Recycle Bin. What was changed is remembered (`ScriptEdit`), so a table updated in place can carry
//! its changed options over to its new script.
use ::library::script_edit::{self, ScriptOption};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A script as the editor shows it: its lines ending in `\n`, and whether they ended in `\r\n`.
fn editor_text(bytes: &[u8]) -> (String, bool) {
    // read as UTF-8, else as Latin-1, as VPX and the player read scripts
    let text = String::from_utf8(bytes.to_vec()).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| char::from(b)).collect());
    let text = text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text);
    let crlf = text.contains("\r\n");
    (text.replace("\r\n", "\n"), crlf)
}

/// The script beside a table, which plays in its own's place.
pub fn beside(table: &Path) -> PathBuf {
    table.with_extension("vbs")
}

/// What the editor opens: the script that plays, and what's needed beside it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    /// The script that plays, its lines ending in `\n`.
    text: String,
    /// Its lines ended in `\r\n`: they're written so again.
    crlf: bool,
    /// A script beside the table plays in its own's place.
    beside: bool,
    /// The file name it's kept under beside the table.
    file: String,
    /// The table's own script's option values, by name, to tell what's been changed.
    own_values: BTreeMap<String, String>,
    /// The game name its script hands PinMAME, where it names one.
    rom: Option<String>,
    /// Its script has its own options menu (`Table1.Option`), in its settings.
    own_menu: bool,
}

pub fn open(table: &Path, rom: Option<String>) -> Result<Doc, String> {
    let (text, crlf) = editor_text(&::library::table::script_that_plays(table)?);
    let (own, _) = editor_text(&::library::table::own_script(table)?);
    let own_values = script_edit::options(&own).into_iter().map(|o| (o.name, o.value)).collect();
    let own_menu = own.to_lowercase().contains(".option(");
    Ok(Doc { text, crlf, beside: beside(table).is_file(), file: beside(table).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(), own_values, rom, own_menu })
}

/// The option lines of a script as the editor has it.
pub fn options(text: &str) -> Vec<ScriptOption> {
    script_edit::options(text)
}

/// The script with its option lines changed.
pub fn with_options(text: &str, changes: &[(String, String)]) -> Result<String, String> {
    script_edit::with_options(text, changes)
}

/// What was changed in a table's script, as it was saved: to carry its options over when the table's
/// own script changes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptEdit {
    /// The table's own script's fingerprint when the change was saved.
    pub base: String,
    /// The table file's size and date then: either changed, its own script may have.
    pub size: u64,
    pub modified: u64,
    /// The option lines changed from the table's own, by name: their values.
    pub options: BTreeMap<String, String>,
    /// Lines changed beyond the option lines (and a ROM changed on its page).
    pub other: bool,
}

/// A table file's size and date, in seconds.
pub fn stamp(table: &Path) -> (u64, u64) {
    std::fs::metadata(table).map(|m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()))).unwrap_or_default()
}

/// The script written beside the table (its line endings as they were), and what was changed from
/// the table's own: the option lines, and whether anything else (`rom`: a ROM changed on its page,
/// its game name from and to, which isn't counted).
pub fn save(table: &Path, text: &str, crlf: bool, rom: Option<(&str, &str)>) -> Result<ScriptEdit, String> {
    let written = if crlf { text.replace('\n', "\r\n") } else { text.to_string() };
    let file = beside(table);
    let part = table.with_extension("vbs.part");
    std::fs::write(&part, written.as_bytes()).and_then(|_| std::fs::rename(&part, &file)).map_err(|e| format!("{}: {e}", file.display()))?;
    let own_bytes = ::library::table::own_script(table)?;
    let (mut own, _) = editor_text(&own_bytes);
    if let Some((from, to)) = rom
        && let Ok(renamed) = ::library::rom_change::renamed_in(own.as_bytes(), from, to)
    {
        own = String::from_utf8_lossy(&renamed).into_owned();
    }
    let was: BTreeMap<String, String> = script_edit::options(&own).into_iter().map(|o| (o.name, o.value)).collect();
    let options: BTreeMap<String, String> = script_edit::options(text).into_iter().filter(|o| was.get(&o.name).is_some_and(|v| *v != o.value)).map(|o| (o.name, o.value)).collect();
    let changes: Vec<(String, String)> = options.iter().map(|(n, v)| (n.clone(), unquote(v))).collect();
    let other = script_edit::with_options(&own, &changes).map_or(true, |remade| remade != text);
    let (size, modified) = stamp(table);
    Ok(ScriptEdit { base: script_edit::digest(&own_bytes), size, modified, options, other })
}

/// One error the check found.
#[derive(Serialize)]
pub struct Problem {
    line: u32,
    column: i32,
    text: String,
}

/// What playing the script in VPX, unseen and unheard, found.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    problems: Vec<Problem>,
    /// It couldn't be played, and why.
    failed: Option<String>,
}

/// Where a script being checked keeps what was beside the table before it, and the list to put it
/// back by (`put back.txt`: the script's path beside the table, then whether one was there, `kept`,
/// or not, `new`).
fn checking_folder() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Scripts").join("Being checked"))
}

/// What was beside a table before a script was checked put back: the one kept aside copied back,
/// or the one checked taken out where none was there — after the check, or the next time the app
/// starts if it didn't see the check end.
pub fn put_back_checked() -> Result<(), String> {
    let Some(dir) = checking_folder() else { return Ok(()) };
    let Ok(list) = std::fs::read_to_string(dir.join("put back.txt")) else { return Ok(()) };
    let mut lines = list.lines();
    let (Some(beside), Some(how)) = (lines.next().map(PathBuf::from), lines.next().map(str::trim)) else { return Ok(()) };
    if how == "kept" {
        std::fs::copy(dir.join("before.vbs"), &beside).map_err(|e| format!("The script beside the table couldn't be put back: {e}"))?;
    } else if beside.is_file() {
        std::fs::remove_file(&beside).map_err(|e| format!("The script checked couldn't be taken out: {e}"))?;
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))
}

/// The script checked in VPX, as VPX plays it: put beside the table, where VPX reads it in its own's
/// place, what was there before kept aside first; the table run unseen and unheard
/// (`vpx_test::check`), its errors read from VPX's log; then what was beside it put back.
pub fn check(vpx: &::library::vpx::Vpx, table: &Path, text: &str, crlf: bool) -> Result<Checked, String> {
    put_back_checked()?;
    let dir = checking_folder().ok_or("The app's data folder isn't there.")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = beside(table);
    let had = file.is_file();
    if had {
        std::fs::copy(&file, dir.join("before.vbs")).map_err(|e| format!("The script beside the table couldn't be kept aside: {e}"))?;
    }
    std::fs::write(dir.join("put back.txt"), format!("{}\n{}\n", file.display(), if had { "kept" } else { "new" })).map_err(|e| e.to_string())?;
    let found = std::fs::write(&file, if crlf { text.replace('\n', "\r\n") } else { text.to_string() })
        .map_err(|e| format!("{}: {e}", file.display()))
        .and_then(|_| ::library::vpx_test::check(vpx, table, &vpi_settings::vpx_settings_text()));
    put_back_checked()?;
    let (errors, failed) = found?;
    Ok(Checked { problems: errors.into_iter().map(|e| Problem { line: e.line, column: e.column, text: e.text }).collect(), failed })
}

/// The script beside the table opened in Notepad (never run: Windows runs a `.vbs` it's asked to
/// open), written out first from the script that plays where there's none.
pub fn notepad(table: &Path) -> Result<(), String> {
    let file = beside(table);
    if !file.is_file() {
        let script = ::library::table::script_that_plays(table)?;
        std::fs::write(&file, script).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Command::new("notepad.exe").arg(&file).spawn().map(|_| ()).map_err(|e| format!("Notepad didn't open: {e}"))
}

/// What a table updated in place takes from the change made to its script before.
pub struct Carried {
    pub script: String,
    /// The option lines its new script no longer has.
    pub dropped: Vec<String>,
}

/// The table's new own script with the options changed before put in again, where it still has
/// them, and a ROM changed on its page, where its script still names the ROM it had.
pub fn carry(table: &Path, edit: &ScriptEdit, rom: Option<(&str, &str)>) -> Result<Carried, String> {
    let (mut own, crlf) = editor_text(&::library::table::own_script(table)?);
    let mut dropped = Vec::new();
    if let Some((from, to)) = rom {
        match ::library::rom_change::renamed_in(own.as_bytes(), from, to) {
            Ok(renamed) => own = String::from_utf8_lossy(&renamed).into_owned(),
            Err(_) => dropped.push(format!("its ROM ({to})")),
        }
    }
    let has: Vec<String> = script_edit::options(&own).into_iter().map(|o| o.name.to_lowercase()).collect();
    let mut changes = Vec::new();
    for (name, value) in &edit.options {
        if has.contains(&name.to_lowercase()) {
            changes.push((name.clone(), unquote(value)));
        } else {
            dropped.push(name.clone());
        }
    }
    let script = script_edit::with_options(&own, &changes)?;
    Ok(Carried { script: if crlf { script.replace('\n', "\r\n") } else { script }, dropped })
}

/// Whether a table's own script changed since its script was edited: its file changed, and its own
/// script with it.
pub fn updated(table: &Path, edit: &ScriptEdit) -> bool {
    if stamp(table) == (edit.size, edit.modified) {
        return false;
    }
    ::library::table::own_script(table).is_ok_and(|own| script_edit::digest(&own) != edit.base)
}

/// A value as the script writes it, as the editor gives it: a string's words without their quotes.
fn unquote(value: &str) -> String {
    match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a copy of a table (`VPI_TABLE`) whose script has option lines: one changed and saved beside
    /// it, what was changed remembered; then, as if the table had been updated, carried over to its
    /// own script. `cargo test -p app --release script_editor -- --ignored`.
    #[test]
    #[ignore]
    fn an_option_saved_and_carried_over() {
        let source = PathBuf::from(std::env::var_os("VPI_TABLE").expect("VPI_TABLE: a table with option lines"));
        let scratch = std::env::temp_dir().join(format!("vpi-script-editor-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let table = scratch.join(source.file_name().unwrap());
        std::fs::copy(&source, &table).unwrap();
        let doc = open(&table, None).unwrap();
        assert!(!doc.beside);
        let first = options(&doc.text).into_iter().find(|o| o.kind == "number").expect("a number option");
        let to = if first.value == "0" { "1" } else { "0" };
        let text = with_options(&doc.text, &[(first.name.clone(), to.to_string())]).unwrap();
        let edit = save(&table, &text, doc.crlf, None).unwrap();
        assert_eq!(edit.options.get(&first.name).map(String::as_str), Some(to));
        assert!(!edit.other);
        assert!(beside(&table).is_file() && !updated(&table, &edit));
        // as if updated: another base, another stamp; carried over to its own script again
        let old = ScriptEdit { base: "something else".into(), size: 1, ..edit.clone() };
        assert!(updated(&table, &old));
        let carried = carry(&table, &old, None).unwrap();
        assert!(carried.dropped.is_empty());
        assert_eq!(options(&carried.script.replace("\r\n", "\n")).iter().find(|o| o.name == first.name).unwrap().value, to);
        // the whole script changed too: counted as more than the options
        let more = format!("{text}' a line of my own\n");
        assert!(save(&table, &more, doc.crlf, None).unwrap().other);
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
