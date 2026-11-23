//! A table's ROM changed ("Change" on a table's ROM row pops up the other ROMs known for the table,
//! saying which are installed; choosing one changes the script as needed,
//! after saying what will change and asking; always a way back; "Select from all ROMs" with a
//! warning that it may not work). The table file itself is never changed: the script beside it
//! (`<table>.vbs`), which VPX 5436 and our player play in its own script's place, is made from the
//! table's script with the game name it hands PinMAME changed — or, when there's one already, it's
//! changed, the one before kept beside it to go back to. The packs kept under the set's name — its
//! PinUP pack where the script hands PinUP the ROM's name, its AltSound pack, its coloring — go with
//! it to a set they play on (`applies`): their folders renamed for it, or copied where another table
//! still plays the set they were for, put back going back. Ours.

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
    let sets = sets_of(&here, &current, &pinmame, on_vps);
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

/// The sets of the game of a table whose script asks for `current`, as `candidates` offers them, by
/// name alone, lower case: the sets a pack made for one of them plays on.
pub fn game_sets(roms: &Path, current: &str, on_vps: &[String]) -> Vec<String> {
    let current = current.to_lowercase();
    sets_of(&installed(roms), &current, &crate::roms::same_game(&current), on_vps)
}

/// `current`, then PinMAME's sets of its game, VPS's for it (`on_vps`) and those of its family `here`.
fn sets_of(here: &[String], current: &str, pinmame: &[crate::roms::KnownSet], on_vps: &[String]) -> Vec<String> {
    let mut sets: Vec<String> = vec![current.to_string()];
    let fam = family(current);
    let parent = pinmame.iter().find(|k| k.set.eq_ignore_ascii_case(current)).and_then(|k| k.clone_of.as_deref().map(str::to_lowercase));
    let same_game = |s: &String| family(s) == fam || parent.as_deref().is_some_and(|p| family(s) == family(p));
    for s in pinmame.iter().map(|k| k.set.to_lowercase()).chain(on_vps.iter().map(|s| s.to_lowercase())).chain(here.iter().filter(|s| same_game(s)).cloned()) {
        if !sets.contains(&s) {
            sets.push(s);
        }
    }
    sets
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
    /// The packs carried to the set it plays now, in the order they were: going back puts each back,
    /// the last first.
    pub carried: Vec<Carried>,
}

/// A table's files kept under its ROM set's name, which follow it to another set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pack {
    /// Its PinUP pack, looked for by the name its script hands PinUP: the ROM's.
    PinUp,
    /// Its AltSound pack, in a folder named for the set.
    AltSound,
    /// Its coloring, in a folder named for the set; a Serum file in it named for the set too.
    Coloring,
}

impl Pack {
    pub fn words(self) -> &'static str {
        match self {
            Pack::PinUp => "PinUP pack",
            Pack::AltSound => "AltSound pack",
            Pack::Coloring => "coloring",
        }
    }
}

/// A pack carried to the set a table plays now: its folder renamed for the set, or copied where
/// another table still plays the set it was for.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Carried {
    pub pack: Pack,
    /// Its folder before.
    pub from: PathBuf,
    /// Its folder now, named for the set the table plays.
    pub to: PathBuf,
    /// Copied: the folder before is kept, for the other tables playing its set.
    pub copied: bool,
    /// Files in it renamed for the set, each by its name before and after: a Serum file, which
    /// libserum looks for as `<set>\<set>.cRZ` (or `.cROMc`, `.cROM`).
    #[serde(default)]
    pub renamed: Vec<(String, String)>,
}

/// Whether a pack made for one of a game's sets plays on `to`: a PinUP pack or an AltSound pack on
/// any set of its game (`game_sets`: PinMAME's sets of it, VPS's for it, those of its family here),
/// as they play from the machine's sound and score display, alike across its versions; a coloring,
/// made for one version's pictures, only on a set VPS lists a coloring made for (`coloring_sets`,
/// the folders VPS gives its game's color files), else — VPS listing none — on any set of its game.
pub fn applies(pack: Pack, to: &str, game_sets: &[String], coloring_sets: &[String]) -> bool {
    let among = |list: &[String]| list.iter().any(|s| s.eq_ignore_ascii_case(to));
    match pack {
        Pack::Coloring if !coloring_sets.is_empty() => among(coloring_sets),
        _ => among(game_sets),
    }
}

/// A pack's `folder`, named for `from_set`, carried to `to_set`'s beside it: renamed, or copied
/// (`copy`), and a Serum file in it named for `from_set` renamed for `to_set`. Nothing is done where
/// `to_set` has a folder already.
pub fn carry(pack: Pack, folder: &Path, from_set: &str, to_set: &str, copy: bool) -> Result<Carried, String> {
    let to = folder.with_file_name(to_set);
    if to.exists() {
        return Err(format!("{} is there already.", to.display()));
    }
    if copy {
        if let Err(e) = copy_dir(folder, &to) {
            // a copy cut short taken away: it was made here
            let _ = std::fs::remove_dir_all(&to);
            return Err(e);
        }
    } else {
        std::fs::rename(folder, &to).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let mut renamed = Vec::new();
    if pack == Pack::Coloring {
        for entry in std::fs::read_dir(&to).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((stem, ext)) = name.rsplit_once('.') else { continue };
            if entry.path().is_file() && stem.eq_ignore_ascii_case(from_set) && ["crz", "cromc", "crom"].contains(&ext.to_lowercase().as_str()) {
                let new = format!("{to_set}.{ext}");
                std::fs::rename(entry.path(), to.join(&new)).map_err(|e| format!("{}: {e}", entry.path().display()))?;
                renamed.push((name, new));
            }
        }
    }
    Ok(Carried { pack, from: folder.to_path_buf(), to, copied: copy, renamed })
}

/// A carried pack put back as it was: a copy to the Recycle Bin (`recycle`); a folder renamed back,
/// its files' names first. One taken away since is left so.
pub fn uncarry(c: &Carried, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    if !c.to.exists() {
        return Ok(());
    }
    if c.copied {
        return recycle(&c.to);
    }
    if c.from.exists() {
        return Err(format!("{} is there again, so {} is left as it is.", c.from.display(), c.to.display()));
    }
    for (before, after) in &c.renamed {
        let now = c.to.join(after);
        if now.exists() {
            std::fs::rename(&now, c.to.join(before)).map_err(|e| format!("{}: {e}", now.display()))?;
        }
    }
    std::fs::rename(&c.to, &c.from).map_err(|e| format!("{}: {e}", c.to.display()))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().map_err(|e| format!("{}: {e}", src.display()))?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
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

    /// Whether the script beside a real table is only its own with its ROM changed, as the app tells
    /// a ROM change from an edited script (reading only): `VPI_TEST_TABLE=<table.vpx>
    /// VPI_TEST_FROM=<its set> VPI_TEST_TO=<the set chosen> cargo test --release -p library --lib
    /// a_rom_change_beside_a_real_table -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_rom_change_beside_a_real_table() {
        let table = PathBuf::from(std::env::var_os("VPI_TEST_TABLE").expect("VPI_TEST_TABLE"));
        let (from, to) = (std::env::var("VPI_TEST_FROM").expect("VPI_TEST_FROM"), std::env::var("VPI_TEST_TO").expect("VPI_TEST_TO"));
        let own = crate::table::own_script(&table).unwrap();
        let beside = std::fs::read(table.with_extension("vbs")).unwrap();
        let renamed = renamed_in(&own, &from, &to).unwrap();
        println!("own {} bytes, beside {} bytes, the same once renamed: {}", own.len(), beside.len(), renamed == beside);
        assert_eq!(renamed, beside);
    }

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

    #[test]
    fn packs_follow_their_games_and_colorings_their_versions() {
        let game: Vec<String> = ["mm_109c", "mm_109b", "mm_10"].map(String::from).to_vec();
        let colorings: Vec<String> = vec!["mm_109c".into()];
        assert!(applies(Pack::PinUp, "MM_109C", &game, &colorings));
        assert!(applies(Pack::AltSound, "mm_10", &game, &colorings));
        assert!(!applies(Pack::PinUp, "afm_113b", &game, &colorings));
        // a coloring only where VPS lists one made for the set, else on any set of its game
        assert!(applies(Pack::Coloring, "mm_109c", &game, &colorings));
        assert!(!applies(Pack::Coloring, "mm_10", &game, &colorings));
        assert!(applies(Pack::Coloring, "mm_10", &game, &[]));
    }

    #[test]
    fn packs_carried_and_put_back() {
        let dir = std::env::temp_dir().join(format!("vpi-rom-carry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (pups, colors) = (dir.join("PUPVideos"), dir.join("altcolor"));
        std::fs::create_dir_all(pups.join("mm_109b").join("1")).unwrap();
        std::fs::write(pups.join("mm_109b").join("1").join("intro.mp4"), b"video").unwrap();
        std::fs::create_dir_all(colors.join("mm_109b").join("backups")).unwrap();
        std::fs::write(colors.join("mm_109b").join("mm_109b.cRZ"), b"serum").unwrap();
        std::fs::write(colors.join("mm_109b").join("backups").join("mm_109b[1].cRZ"), b"old").unwrap();
        // renamed: the folder, and the Serum file in it, the backup left as it was
        let pup = carry(Pack::PinUp, &pups.join("mm_109b"), "mm_109b", "mm_109c", false).unwrap();
        let col = carry(Pack::Coloring, &colors.join("mm_109b"), "mm_109b", "mm_109c", false).unwrap();
        let moved = pups.join("mm_109c").join("1").join("intro.mp4").is_file() && !pups.join("mm_109b").exists();
        let named = colors.join("mm_109c").join("mm_109c.cRZ").is_file() && colors.join("mm_109c").join("backups").join("mm_109b[1].cRZ").is_file();
        // a second carry onto a set with a folder of its own does nothing
        std::fs::create_dir_all(pups.join("mm_10")).unwrap();
        let refused = carry(Pack::PinUp, &pups.join("mm_109c"), "mm_109c", "mm_10", false).is_err() && pups.join("mm_109c").exists();
        // copied, for another table still playing the set: the copy taken away going back
        let copy = carry(Pack::PinUp, &pups.join("mm_109c"), "mm_109c", "mm_109x", true).unwrap();
        let both = pups.join("mm_109c").join("1").join("intro.mp4").is_file() && pups.join("mm_109x").join("1").join("intro.mp4").is_file();
        uncarry(&copy, &|p| std::fs::remove_dir_all(p).map_err(|e| e.to_string())).unwrap();
        uncarry(&col, &|_| Ok(())).unwrap();
        uncarry(&pup, &|_| Ok(())).unwrap();
        let back = pups.join("mm_109b").join("1").join("intro.mp4").is_file()
            && colors.join("mm_109b").join("mm_109b.cRZ").is_file()
            && !pups.join("mm_109x").exists()
            && !colors.join("mm_109c").exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(moved && named && pup.renamed.is_empty() && col.renamed == vec![("mm_109b.cRZ".to_string(), "mm_109c.cRZ".to_string())]);
        assert!(refused && copy.copied && both && back);
    }
}
