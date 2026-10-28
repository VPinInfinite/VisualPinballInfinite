//! A table's script edited in the app: the script that plays — the one beside the table, else its
//! own — opened, its option lines listed (`library::script_edit`) and the whole of it shown; saved
//! beside the table as `<table>.vbs`, which VPX and our player play in its own's place, the table
//! file untouched; checked by playing it with no window before it's kept (the headless runner, the
//! script handed to it by `VPI_SCRIPT_FILE`); opened in Notepad; or put back, the one beside it to the
//! Recycle Bin. What was changed is remembered (`ScriptEdit`), so a table updated in place can carry
//! its changed options over to its new script.
use ::library::script_edit::{self, ScriptOption};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::os::windows::process::CommandExt;
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

/// What playing the script with no window found.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    problems: Vec<Problem>,
    /// What the engine doesn't answer yet, which plays on without it.
    missing: Vec<String>,
    /// It couldn't be played, and why.
    failed: Option<String>,
}

/// The script played as the player plays it, with no window, for eight seconds in real time — the
/// table's machine with it — its errors read from what the runner says.
pub fn check(table: &Path, text: &str, crlf: bool) -> Result<Checked, String> {
    let runner = ::library::test::runner().ok_or("The headless runner (vpi-headless.exe) isn't beside the app.")?;
    let dir = crate::library::data_folder().ok_or("The app's data folder isn't there.")?.join("Scripts");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("being checked.vbs");
    std::fs::write(&file, if crlf { text.replace('\n', "\r\n") } else { text.to_string() }).map_err(|e| format!("{}: {e}", file.display()))?;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new(runner)
        .arg(table)
        .args(["--script", "--realtime", "--ms", ::library::test::SCRIPT_MS])
        .env("VPI_SCRIPT_FILE", &file)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("The headless runner didn't start: {e}"))?;
    let _ = std::fs::remove_file(&file);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut checked = Checked { problems: Vec::new(), missing: Vec::new(), failed: None };
    for line in stderr.lines() {
        let Some(rest) = line.trim().strip_prefix("[script] ") else { continue };
        if let Some(error) = rest.strip_prefix("error on line ") {
            // `<line>, column <column>: <what>`
            let (line_no, after) = error.split_once(", column ").unwrap_or((error, ""));
            let (column, what) = after.split_once(": ").unwrap_or(("0", after));
            checked.problems.push(Problem { line: line_no.trim().parse().unwrap_or(0), column: column.trim().parse().unwrap_or(0), text: what.trim().to_string() });
        } else if let Some(member) = rest.strip_prefix("not supported yet: ") {
            checked.missing.push(member.split(" (first").next().unwrap_or(member).to_string());
        }
    }
    if !out.status.success() && checked.problems.is_empty() {
        checked.failed = Some(stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("The run stopped before its end.").trim().to_string());
    }
    Ok(checked)
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
