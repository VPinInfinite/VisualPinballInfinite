//! A table's ROM changed ("Change" on a table's ROM row pops up the other ROMs known for the table,
//! saying which are installed; choosing one changes the script as needed,
//! after saying what will change and asking; always a way back; "Select from all ROMs" with a
//! warning that it may not work). The table file itself is never changed: the script beside it
//! (`<table>.vbs`), which VPX 5436 and our player play in its own script's place, is made from the
//! table's script with the game name it hands PinMAME changed — or, when there's one already, it's
//! changed, the one before kept beside it to go back to. Ours.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex;
use serde::{Deserialize, Serialize};

/// The game name a script hands PinMAME, as `table::script_facts` reads it, and where its value is:
/// in the script's bytes, which are kept as they are (VPX reads a script beside a table as UTF-8,
/// else as Latin-1).
static GAME_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b(?:cgamename|gamename)\s*=\s*"([^"\r\n]+)""#).unwrap());

/// A ROM set offered for a table.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub set: String,
    /// PinMAME's name for it ("Attack From Mars (1.1 Bally)").
    pub description: Option<String>,
    /// Its zip is in VPinMAME's `roms`.
    pub have: bool,
    /// VPS lists it for the table's game.
    pub on_vps: bool,
    /// The one its script asks for now.
    pub current: bool,
}

/// A set's family: the name before its version (`afm` of `afm_113b`), PinMAME's sets of one game
/// mostly sharing it.
fn family(set: &str) -> String {
    set.split_once('_').map_or(set, |(f, _)| f).to_lowercase()
}

/// The sets in VPinMAME's `roms`, by name, lower case.
pub fn installed(roms: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(roms)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip"))).then(|| p.file_stem().map(|s| s.to_string_lossy().to_lowercase())).flatten()
        })
        .collect();
    out.sort();
    out
}

/// The ROM sets known for a table whose script asks for `current`: PinMAME's sets of its game (its
/// parent and the parent's clones), VPS's for its game (`on_vps`), those of the same game installed,
/// and the one it asks for now — each said by PinMAME's name for it, and whether it's here.
pub fn candidates(roms: &Path, current: &str, on_vps: &[String]) -> Vec<Candidate> {
    let here = installed(roms);
    let current = current.to_lowercase();
    let pinmame = crate::roms::same_game(&current);
    let mut sets: Vec<String> = vec![current.clone()];
    let fam = family(&current);
    let parent = pinmame.iter().find(|k| k.set.eq_ignore_ascii_case(&current)).and_then(|k| k.clone_of.as_deref().map(str::to_lowercase));
    let same_game = |s: &String| family(s) == fam || parent.as_deref().is_some_and(|p| family(s) == family(p));
    for s in pinmame.iter().map(|k| k.set.to_lowercase()).chain(on_vps.iter().map(|s| s.to_lowercase())).chain(here.iter().filter(|s| same_game(s)).cloned()) {
        if !sets.contains(&s) {
            sets.push(s);
        }
    }
    let mut out: Vec<Candidate> = sets
        .into_iter()
        .map(|set| {
            // PinMAME's name for it from its list, else asked of it alone (a set the list hasn't)
            let description = pinmame
                .iter()
                .find(|k| k.set.eq_ignore_ascii_case(&set))
                .and_then(|k| k.description.clone())
                .or_else(|| crate::roms::set(&set).ok().filter(|s| s.known).and_then(|s| s.description));
            Candidate { have: here.contains(&set), on_vps: on_vps.iter().any(|v| v.eq_ignore_ascii_case(&set)), current: set == current, description, set }
        })
        .collect();
    // the current first, then those here, then by name
    out.sort_by(|a, b| b.current.cmp(&a.current).then(b.have.cmp(&a.have)).then(a.set.cmp(&b.set)));
    out
}

/// What a change did, to go back on it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Changed {
    pub from: String,
    pub to: String,
    /// The script beside the table was made by the change: going back takes it away.
    pub made: bool,
    /// The script beside it before the change, kept here: going back puts it back.
    pub before: Option<PathBuf>,
}

/// The table's script with the game name `from` changed to `to`, where the script hands it; every
/// other byte as it was.
pub fn renamed_in(script: &[u8], from: &str, to: &str) -> Result<Vec<u8>, String> {
    let mut found = false;
    let out = GAME_NAME.replace_all(script, |c: &regex::bytes::Captures| {
        let (whole, name) = (c.get(0).unwrap(), c.get(1).unwrap());
        if name.as_bytes().eq_ignore_ascii_case(from.as_bytes()) {
            found = true;
            let at = name.start() - whole.start();
            [&whole.as_bytes()[..at], to.as_bytes(), &whole.as_bytes()[at + name.len()..]].concat()
        } else {
            whole.as_bytes().to_vec()
        }
    });
    if found { Ok(out.into_owned()) } else { Err(format!("Its script doesn't name {from} where it hands PinMAME its game, so it can't be changed here.")) }
}

/// The ROM set `to` put in `from`'s place for the table at `table`, whose script (the one that plays:
/// the script beside it, else its own) is `script`: the script beside it written with the new name,
/// the one before kept beside it. What was done, to go back on it.
pub fn change(table: &Path, script: &[u8], from: &str, to: &str) -> Result<Changed, String> {
    if to.is_empty() || to.contains(['"', '\r', '\n']) {
        return Err(format!("\"{to}\" can't be a ROM set's name."));
    }
    let text = renamed_in(script, from, to)?;
    let beside = table.with_extension("vbs");
    let mut changed = Changed { from: from.to_string(), to: to.to_string(), ..Default::default() };
    if beside.exists() {
        let keep = table.with_extension("vbs.before-rom-change");
        if !keep.exists() {
            std::fs::copy(&beside, &keep).map_err(|e| format!("{}: {e}", keep.display()))?;
        }
        changed.before = Some(keep);
    } else {
        changed.made = true;
    }
    let part = table.with_extension("vbs.part");
    std::fs::write(&part, text).and_then(|_| std::fs::rename(&part, &beside)).map_err(|e| format!("{}: {e}", beside.display()))?;
    Ok(changed)
}

/// The change gone back on: the script beside the table taken away (`recycle`: to the Recycle Bin)
/// if the change made it, else the one before put back.
pub fn undo(table: &Path, changed: &Changed, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    let beside = table.with_extension("vbs");
    if changed.made {
        if beside.exists() {
            recycle(&beside)?;
        }
        return Ok(());
    }
    let before = changed.before.as_ref().ok_or("What was there before isn't known.")?;
    std::fs::rename(before, &beside).map_err(|e| format!("{}: {e}", beside.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_name_changed_where_the_script_hands_it() {
        // a Latin-1 byte (©) kept as it is
        let script = b"Option Explicit\r\nConst cGameName = \"afm_113b\"  ' \xa9 the ROM\r\nDim x : x = \"afm_113b\"\r\n";
        let out = renamed_in(script, "AFM_113B", "afm_113").unwrap();
        assert_eq!(out, b"Option Explicit\r\nConst cGameName = \"afm_113\"  ' \xa9 the ROM\r\nDim x : x = \"afm_113b\"\r\n");
        assert!(renamed_in(b"Dim x\r\n", "afm_113b", "afm_113").is_err());
    }

    #[test]
    fn changed_and_gone_back_on() {
        let dir = std::env::temp_dir().join(format!("vpi-rom-change-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("t.vpx");
        // no script beside it: one made, and taken away going back
        let made = change(&table, b"Const cGameName=\"afm_113b\"\r\n", "afm_113b", "afm_113").unwrap();
        let written = std::fs::read_to_string(table.with_extension("vbs")).unwrap();
        undo(&table, &made, &|p| std::fs::remove_file(p).map_err(|e| e.to_string())).unwrap();
        let gone = !table.with_extension("vbs").exists();
        // one beside it already: changed, and put back going back
        std::fs::write(table.with_extension("vbs"), "' mine\r\nConst cGameName=\"tz_92\"\r\n").unwrap();
        let edited = change(&table, b"' mine\r\nConst cGameName=\"tz_92\"\r\n", "tz_92", "tz_94h").unwrap();
        let after = std::fs::read_to_string(table.with_extension("vbs")).unwrap();
        undo(&table, &edited, &|_| Ok(())).unwrap();
        let back = std::fs::read_to_string(table.with_extension("vbs")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(made.made && written.contains("\"afm_113\"") && gone);
        assert!(!edited.made && after.contains("\"tz_94h\"") && back.contains("\"tz_92\""));
    }
}
