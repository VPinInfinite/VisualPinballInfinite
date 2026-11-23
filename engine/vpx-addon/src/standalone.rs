//! The add-on on its own, in a VPX with no Visual Pinball Infinite beside it. Where no app hands it
//! a game's packs (`VPI_ACHIEVEMENTS`), it finds them itself: in an `achievements` folder beside the
//! table and in VPX's preferences folder, each matched to the table by the machine's ROM set, else
//! by its game's name (and year) in the table file's name, the latest version of each pack taken.
//! What's earned is kept in VPX's preferences folder. A game is hardcore unless something the
//! add-on can see makes it softcore: a script or a physics file beside the table, the table's own
//! options or difficulty changed in its settings, PinMAME's cheat mode. Its own settings — the
//! sound, the card — are VPX's, set in VPX's menu (`[Plugin.VPIAchievements]`), and its card is
//! drawn under VPX's own priorities for that window.

use std::path::{Path, PathBuf};

use vpi_achievements::pack::{For, Pack};

use crate::follow::Asked;

/// The folder packs are kept in, beside a table and in VPX's preferences folder.
pub const FOLDER: &str = "achievements";

/// How many folders down packs are looked for: `achievements\<pack>\<version>`.
const DEPTH: usize = 3;

/// Folders holding a pack (`achievements.json`) at `dir` or under it, up to `depth` folders down.
pub fn pack_dirs(dir: &Path, depth: usize) -> Vec<PathBuf> {
    if dir.join("achievements.json").is_file() {
        return vec![dir.to_path_buf()];
    }
    if depth == 0 {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut subs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort();
    subs.iter().flat_map(|s| pack_dirs(s, depth - 1)).collect()
}

/// Letters and digits alone, in lower case: names compared whatever their spaces and punctuation.
fn key(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// The years in a name: runs of four digits from 1930 to 2099 standing alone.
fn years_in(s: &str) -> Vec<i32> {
    let mut out = Vec::new();
    let mut run = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() {
            run.push(c);
            continue;
        }
        if run.len() == 4
            && let Ok(y) = run.parse::<i32>()
            && (1930..=2099).contains(&y)
        {
            out.push(y);
        }
        run.clear();
    }
    out
}

/// Whether a pack is for this table: one of its ROM sets is the machine's; else — a pack naming no
/// set, or a table with no machine — its game's name in the table file's name (letters and digits,
/// any case), and its year too where the pack gives one and the file's name has years.
pub fn fits(f: &For, stem: &str, rom: Option<&str>) -> bool {
    if let Some(rom) = rom {
        if f.roms.iter().any(|r| r.eq_ignore_ascii_case(rom)) {
            return true;
        }
        // made for other sets: another machine's, or another version's
        if !f.roms.is_empty() {
            return false;
        }
    }
    let name = key(&f.game.name);
    if name.len() < 3 || !key(stem).contains(&name) {
        return false;
    }
    let years = years_in(stem);
    match f.game.year {
        Some(y) if !years.is_empty() => years.contains(&y),
        _ => true,
    }
}

/// Whether version `a` is later than `b`, by their numbers part by part (1.10 after 1.9).
fn later(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) > parts(b)
}

/// The packs for a table: those in the `achievements` folder beside it and in VPX's preferences
/// folder's, fitting it, each pack once — its latest version where several are kept.
pub fn packs_for(table: &Path, prefs: &Path, rom: Option<&str>) -> Vec<PathBuf> {
    let stem = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut found: Vec<(String, String, PathBuf)> = Vec::new();
    let roots = [table.parent().map(|d| d.join(FOLDER)), Some(prefs.join(FOLDER))];
    for root in roots.into_iter().flatten() {
        for dir in pack_dirs(&root, DEPTH) {
            let Ok(p) = Pack::read(&dir) else { continue };
            if !fits(&p.made_for, &stem, rom) {
                continue;
            }
            match found.iter_mut().find(|(id, ..)| *id == p.pack.id) {
                Some(kept) if later(&p.pack.version, &kept.1) => *kept = (p.pack.id, p.pack.version, dir),
                Some(_) => {}
                None => found.push((p.pack.id, p.pack.version, dir)),
            }
        }
    }
    found.into_iter().map(|(_, _, dir)| dir).collect()
}

/// Why a game of `table` can't be hardcore, as the add-on can tell on its own; none where it plays
/// as its author made it: a script beside it in its own's place, its own physics file, its options
/// or difficulty changed in its own settings (VPX keeps there only what differs), PinMAME's cheat
/// mode on in VPX's settings or the table's.
pub fn softcore_because(table: &Path, vpx_settings: &str) -> Option<String> {
    if table.with_extension("vbs").is_file() {
        return Some("its script is changed: a script beside it plays in its place".into());
    }
    if table.with_extension("vpp").is_file() {
        return Some("its physics are changed: a physics file beside it".into());
    }
    let own = std::fs::read(table.with_extension("ini")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let mine = crate::hardcore::that_matter(&own);
    if mine.keys().any(|(section, _)| section == "tableoption" || section == "tableoverride") {
        return Some("its options or difficulty are changed in its own settings".into());
    }
    let cheat = |set: &std::collections::BTreeMap<(String, String), String>| set.get(&("plugin.pinmame".to_string(), "cheat".to_string())).is_some_and(|v| v != "0" && !v.eq_ignore_ascii_case("false"));
    if cheat(&mine) || cheat(&crate::hardcore::that_matter(vpx_settings)) {
        return Some("PinMAME's cheat mode is on".into());
    }
    None
}

/// VPX's priorities for a window's renderers (`Priority.<id>` in its section of the settings), the
/// table's own over the main settings', as VPX reads them.
pub fn priorities(section: &str, main: &str, own: &str) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = Vec::new();
    for text in [main, own] {
        let mut here = String::new();
        for line in text.lines() {
            let l = line.trim().trim_start_matches('\u{feff}');
            if l.starts_with('[') && l.ends_with(']') {
                here = l[1..l.len() - 1].trim().to_string();
                continue;
            }
            let Some((k, v)) = l.split_once('=') else { continue };
            let (Some(id), true) = (k.trim().strip_prefix("Priority."), here.eq_ignore_ascii_case(section)) else { continue };
            let Ok(p) = v.trim().parse::<i64>() else { continue };
            out.retain(|(i, _)| i != id);
            out.push((id.to_string(), p));
        }
    }
    out
}

/// The card as the add-on's own settings set it: where (none for no card) and how big.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    pub on: Option<&'static str>,
    pub size: &'static str,
}

/// A game's instructions built as the app would hand them, for a table and VPX's preferences
/// folder: the packs that fit, hardcore or softcore and why, where what's earned is kept, the
/// sound and the card. None where no pack fits the table.
pub fn asked(table: &Path, prefs: &Path, rom: Option<&str>, sound: bool, card: Card) -> Option<Asked> {
    let packs = packs_for(table, prefs, rom);
    if packs.is_empty() {
        return None;
    }
    let main_file = prefs.join("VPinballX.ini");
    let main = std::fs::read(&main_file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let own_file = table.with_extension("ini");
    let own = std::fs::read(&own_file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let why = softcore_because(table, &main);
    let popup = card.on.and_then(|on| {
        let section = if on == "scoreview" { "ScoreView" } else { "Backglass" };
        let priorities: serde_json::Map<String, serde_json::Value> = priorities(section, &main, &own).into_iter().map(|(id, p)| (id, p.into())).collect();
        crate::popup::Settings::from_json(&serde_json::json!({ "on": on, "size": card.size, "priorities": priorities }))
    });
    let hardcore = why.is_none();
    Some(Asked {
        mode: if hardcore { "hardcore" } else { "practice" }.into(),
        why,
        table: table.file_name().map(|n| n.to_string_lossy().into_owned()),
        packs,
        earned: prefs.join(FOLDER).join("earned.json"),
        trying: false,
        result: None,
        sound_on: sound,
        gain: 1.0,
        popup,
        hardcore_files: if hardcore { vec![main_file, own_file] } else { Vec::new() },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vpi-addon-standalone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn pack(dir: &Path, id: &str, version: &str, made_for: serde_json::Value) {
        std::fs::create_dir_all(dir).unwrap();
        let json = serde_json::json!({ "format": "vpi-achievements", "formatVersion": 1,
            "pack": { "id": id, "name": "Test", "version": version }, "for": made_for,
            "achievements": [{ "id": "start", "name": "Off We Go", "about": "a", "points": 1, "since": "1.0", "when": { "game": "started" } }] });
        std::fs::write(dir.join("achievements.json"), serde_json::to_vec(&json).unwrap()).unwrap();
    }

    #[test]
    fn a_pack_fits_by_its_rom_else_its_games_name_and_year() {
        let f = |v: serde_json::Value| serde_json::from_value::<For>(v).unwrap();
        let afm = f(serde_json::json!({ "game": { "name": "Attack from Mars", "maker": "Bally", "year": 1995 }, "roms": ["afm_113b", "afm_113"] }));
        assert!(fits(&afm, "Attack from Mars (Bally 1995)", Some("afm_113b")));
        assert!(fits(&afm, "anything at all", Some("AFM_113")));
        assert!(!fits(&afm, "Attack from Mars (Bally 1995)", Some("mm_109c")), "another machine's set");
        assert!(fits(&afm, "Attack from Mars (Bally 1995)", None), "no machine heard: by its name");
        let batman = f(serde_json::json!({ "game": { "name": "Batman", "maker": "Data East", "year": 1991 } }));
        assert!(fits(&batman, "Batman (Data East 1991) VPW v1.1", None));
        assert!(!fits(&batman, "Batman [The Dark Knight] (Stern 2008)", None), "another year");
        assert!(fits(&batman, "batman_mod", None), "no year in the file's name");
        let lk = f(serde_json::json!({ "game": { "name": "Leprechaun King" } }));
        assert!(fits(&lk, "Leprechaun King (Orbital 2020)", Some("ignored")), "a pack naming no set, a machine running");
        assert!(!fits(&lk, "Blood Machines 2.0", None));
        assert_eq!(years_in("TimeLord-4.2 (2021) 19995 1925"), [2021]);
        assert!(later("1.10", "1.9") && !later("1.0", "1.0") && later("2", "1.9.9"));
    }

    #[test]
    fn packs_found_beside_the_table_and_in_vpxs_folder_their_latest_versions() {
        let dir = scratch("found");
        let tables = dir.join("Tables");
        let prefs = dir.join("prefs");
        let table = tables.join("Attack from Mars (Bally 1995).vpx");
        std::fs::create_dir_all(&tables).unwrap();
        std::fs::write(&table, b"").unwrap();
        let afm = serde_json::json!({ "game": { "name": "Attack from Mars", "year": 1995 }, "roms": ["afm_113b"] });
        pack(&tables.join(FOLDER).join("afm"), "a1", "1.0", afm.clone());
        pack(&prefs.join(FOLDER).join("a1").join("1.1"), "a1", "1.1", afm.clone());
        pack(&prefs.join(FOLDER).join("a1").join("1.0"), "a1", "1.0", afm);
        pack(&prefs.join(FOLDER).join("mm"), "m1", "1.0", serde_json::json!({ "game": { "name": "Medieval Madness" }, "roms": ["mm_109c"] }));
        let got = packs_for(&table, &prefs, Some("afm_113b"));
        assert_eq!(got, [prefs.join(FOLDER).join("a1").join("1.1")]);
        assert!(packs_for(&table, &prefs, Some("mm_109c")).iter().all(|p| p.ends_with("mm")));
        // no pack fits: nothing to follow
        assert!(asked(&tables.join("Gorgar (Williams 1979).vpx"), &prefs, Some("grgar_l1"), true, Card { on: None, size: "medium" }).is_none());
        let a = asked(&table, &prefs, Some("afm_113b"), true, Card { on: Some("backglass"), size: "small" }).unwrap();
        assert_eq!((a.mode.as_str(), a.why.as_deref(), a.earned.clone()), ("hardcore", None, prefs.join(FOLDER).join("earned.json")));
        assert_eq!(a.hardcore_files, [prefs.join("VPinballX.ini"), table.with_extension("ini")]);
        assert!(a.popup.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_makes_a_game_softcore_on_its_own() {
        let dir = scratch("softcore");
        let table = dir.join("Medieval Madness (Williams 1997).vpx");
        std::fs::write(&table, b"").unwrap();
        assert_eq!(softcore_because(&table, ""), None);
        assert_eq!(softcore_because(&table, "[Plugin.PinMAME]\nCheat = 0\n"), None);
        assert!(softcore_because(&table, "[Plugin.PinMAME]\nCheat = 1\n").unwrap().contains("cheat"));
        std::fs::write(table.with_extension("ini"), "[TableOption]\nBallColour = 2\n").unwrap();
        assert!(softcore_because(&table, "").unwrap().contains("options"));
        std::fs::write(table.with_extension("ini"), "[Player]\nPlayfieldFullScreen = 1\n").unwrap();
        assert_eq!(softcore_because(&table, ""), None, "a window setting changes nothing");
        std::fs::write(table.with_extension("vpp"), b"").unwrap();
        assert!(softcore_because(&table, "").unwrap().contains("physics"));
        std::fs::write(table.with_extension("vbs"), b"").unwrap();
        assert!(softcore_because(&table, "").unwrap().contains("script"));
        let main = "[Backglass]\nPriority.B2S = 2\nPriority.PUP = 1\n[ScoreView]\nPriority.Nixie = 3\n";
        assert_eq!(priorities("Backglass", main, "[Backglass]\nPriority.PUP = 4\n"), [("B2S".to_string(), 2), ("PUP".to_string(), 4)]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
