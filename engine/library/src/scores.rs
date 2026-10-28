//! A table's high scores, where its machine or its script keeps them, for its page — from the
//! machine's memory, VPX's brought over too, and from table scripts that keep their own: a ROM
//! machine's in its memory (`<rom>.nv`), read through
//! the Pinball Memory Maps project's maps of each game's memory (`pinmame-nvram`, MIT; the maps
//! ODbL), as our player keeps it and as VPX's PinMAME keeps it; a table whose script keeps its own,
//! in the values it saves (`SaveValue`: `HighScore1`, `HighScore1Name`, … and `TotalGamesPlayed`,
//! as most scripts name them), as our player keeps them (`VPReg.ini`) and as VPX does
//! (`VPReg.stg`, a compound file: a storage for each name, a stream of UTF-16 text for each value).
//! Ours.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// One place on the machine's or the script's list.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// What the machine calls the place ("Grand Champion", "First Place"); None: its rank.
    pub label: Option<String>,
    pub initials: String,
    pub score: Option<u64>,
    /// What a champion's place counts besides, as the machine shows it ("12 loops", a time).
    pub extra: Option<String>,
}

/// One copy of a table's high scores: our player's, or VPX's.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    /// `ours` or `vpx`.
    pub by: &'static str,
    pub file: PathBuf,
    /// When the file last changed, in ms since 1970.
    pub changed: i64,
    pub scores: Vec<Entry>,
    /// A machine's mode champions.
    pub champions: Vec<Entry>,
    pub games_played: Option<u64>,
}

/// A table's high scores, as each copy keeps them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighScores {
    /// `machine` (its ROM's memory), `script` (the values its script saves), or `none`.
    pub kind: &'static str,
    /// The name they're kept under: the ROM set, or the script's own name.
    pub under: Option<String>,
    pub kept: Vec<Kept>,
    /// Why they can't be read, where they can't: no map for the machine's memory yet.
    pub problem: Option<String>,
}

fn changed(path: &Path) -> i64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64)
}

fn clean(text: &str) -> String {
    text.trim_matches(|c: char| c.is_whitespace() || c == '\0').to_string()
}

/// Initials as the memory maps read them: `pinmame-nvram` writes a byte outside ASCII as the text
/// `\uXXXX`, and a slot the machine never filled holds bytes of 0xFF (Medieval Madness's Kings of
/// the Realm: "\u00FF\u00FF\u00FF"). Only ASCII letters,
/// digits, marks and spaces are kept — machines write their initials in nothing else — so an
/// unfilled slot's are none.
fn initials(text: &str) -> String {
    let mut decoded = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("\\u") {
        decoded.push_str(&rest[..at]);
        match rest.get(at + 2..at + 6).and_then(|h| u32::from_str_radix(h, 16).ok()).and_then(char::from_u32) {
            Some(c) => {
                decoded.push(c);
                rest = &rest[at + 6..];
            }
            None => {
                decoded.push_str("\\u");
                rest = &rest[at + 2..];
            }
        }
    }
    decoded.push_str(rest);
    clean(&decoded.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').collect::<String>())
}

/// Whether an entry holds anything: a slot never filled has no initials and no score.
fn filled(e: &Entry) -> bool {
    !e.initials.is_empty() || e.score.is_some_and(|s| s > 0)
}

/// A machine's memory read: None when there's no map for the game.
fn machine_copy(by: &'static str, file: &Path) -> Result<Option<Kept>, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", file.display());
    let Some(mut nv) = pinmame_nvram::Nvram::open(file).map_err(fail)? else { return Ok(None) };
    let scores = nv
        .read_highscores()
        .map_err(fail)?
        .into_iter()
        .map(|h| Entry { label: h.label.map(|l| clean(&l)), initials: initials(&h.initials), score: Some(h.score), extra: None })
        .filter(filled)
        .collect();
    let champions = nv
        .read_mode_champions()
        .map_err(fail)?
        .unwrap_or_default()
        .into_iter()
        .map(|c| Entry { label: c.label.map(|l| clean(&l)), initials: c.initials.map(|i| initials(&i)).unwrap_or_default(), score: c.score, extra: c.suffix.or(c.timestamp).map(|x| clean(&x)).filter(|x| !x.is_empty()) })
        .filter(filled)
        .collect();
    Ok(Some(Kept { by, file: file.to_path_buf(), changed: changed(file), scores, champions, games_played: None }))
}

/// What a ROM machine's memory map lets the player read of its game as it plays: the names of its
/// game state (`game_over`, `current_ball`, `current_player`, `scores`, `credits`, `extra_balls`,
/// `tilted`, …), none where no map knows the machine. The map is found by the set's name, which
/// the reader takes from a memory file's: a passing empty one, deleted after.
pub fn game_facts(rom: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("vpi-map-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("{}.nv", rom.to_ascii_lowercase()));
    let _ = std::fs::write(&file, []);
    let facts = pinmame_nvram::Nvram::open(&file).ok().flatten().and_then(|nv| nv.map.game_state.as_ref().map(|g| g.keys().cloned().collect::<Vec<_>>())).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_dir(&dir);
    facts
}

/// A ROM machine's high scores: its memory as our player keeps it (`ours`, the player's PinMAME
/// `nvram` folder) and as VPX's PinMAME keeps it (`vpx`, VPinMAME's `nvram` folder), each that's there.
pub fn of_machine(rom: &str, ours: &Path, vpx: &Path) -> HighScores {
    let mut out = HighScores { kind: "machine", under: Some(rom.to_string()), ..Default::default() };
    for (by, dir) in [("ours", ours), ("vpx", vpx)] {
        // named as PinMAME names its sets, whatever case the script wrote: the map is found by it
        let file = dir.join(format!("{}.nv", rom.to_ascii_lowercase()));
        if !file.is_file() {
            continue;
        }
        match machine_copy(by, &file) {
            Ok(Some(k)) => out.kept.push(k),
            Ok(None) => {
                out.problem = Some(format!("Its machine's memory can't be read yet: the Pinball Memory Maps project has no map of {rom}'s."));
                break;
            }
            Err(e) => out.problem = Some(e),
        }
    }
    out
}

/// `HighScore1` … and `HighScore1Name` …, as most scripts name their high scores.
static HIGH_SCORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^high\s*score\s*(\d+)$").unwrap());

/// A saved number as text: VBScript's, a whole number or one with an exponent.
fn number(text: &str) -> Option<u64> {
    let t = text.trim();
    t.parse::<u64>().ok().or_else(|| t.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0 && *v < 1.8e19).map(|v| v.round() as u64))
}

/// A script's high scores from its values (by name, lower case).
fn from_values(by: &'static str, file: &Path, values: &BTreeMap<String, String>) -> Option<Kept> {
    let mut ranked: Vec<(u32, Entry)> = values
        .iter()
        .filter_map(|(name, text)| {
            let n: u32 = HIGH_SCORE.captures(name)?[1].parse().ok()?;
            let initials = values.get(&format!("highscore{n}name")).map(|i| clean(i)).unwrap_or_default();
            Some((n, Entry { label: None, initials, score: number(text), extra: None }))
        })
        .collect();
    let games_played = values.get("totalgamesplayed").and_then(|t| number(t));
    if ranked.is_empty() && games_played.is_none() {
        return None;
    }
    ranked.sort_by_key(|(n, _)| *n);
    Some(Kept { by, file: file.to_path_buf(), changed: changed(file), scores: ranked.into_iter().map(|(_, e)| e).collect(), champions: Vec::new(), games_played })
}

/// The values kept under `name` in our player's file, by value name in lower case.
pub fn values_in_ini(file: &Path, name: &str) -> BTreeMap<String, String> {
    let text = std::fs::read(file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    vpi_settings::parse_ini(&text).into_iter().filter(|(s, _, _)| s.eq_ignore_ascii_case(name)).map(|(_, k, v)| (k.to_lowercase(), v)).collect()
}

/// The values kept under `name` in VPX's `VPReg.stg`, by value name in lower case; None when it
/// keeps none under it.
pub fn values_in_stg(file: &Path, name: &str) -> Result<Option<BTreeMap<String, String>>, String> {
    Ok(stg_values(file, name)?.map(|(_, values)| values.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect()))
}

/// The name values are kept under in VPX's `VPReg.stg`, and each value's name and text.
type StgValues = (String, Vec<(String, String)>);

/// The values kept under `name` in VPX's `VPReg.stg`, with the name and value names as VPX keeps
/// them; None when it keeps none under it.
fn stg_values(file: &Path, name: &str) -> Result<Option<StgValues>, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", file.display());
    let mut stg = cfb::open(file).map_err(fail)?;
    // the storage named as the script names it, in any case, as VPX's storage finds it
    let Some(storage) = stg.read_root_storage().filter(|e| e.is_storage()).map(|e| e.name().to_string()).find(|n| n.eq_ignore_ascii_case(name)) else { return Ok(None) };
    let streams: Vec<String> = stg.read_storage(format!("/{storage}")).map_err(fail)?.filter(|e| e.is_stream()).map(|e| e.name().to_string()).collect();
    let mut out = Vec::new();
    for stream in streams {
        let mut bytes = Vec::new();
        stg.open_stream(format!("/{storage}/{stream}")).map_err(fail)?.read_to_end(&mut bytes).map_err(fail)?;
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        out.push((stream, String::from_utf16_lossy(&units).trim_end_matches('\0').to_string()));
    }
    Ok(Some((storage, out)))
}

/// The names VPX's `VPReg.stg` keeps values under, each with how many it keeps.
pub fn names_in_stg(file: &Path) -> Result<Vec<(String, usize)>, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", file.display());
    let stg = cfb::open(file).map_err(fail)?;
    let names: Vec<String> = stg.read_root_storage().filter(|e| e.is_storage()).map(|e| e.name().to_string()).collect();
    Ok(names.into_iter().map(|n| (n.clone(), stg.read_storage(format!("/{n}")).map_or(0, |s| s.count()))).collect())
}

/// The high scores a table's script keeps among its values, under the first of `names` that holds
/// any: as our player keeps them (`ours`, its `VPReg.ini`) and as VPX does (each of `vpx`, its
/// `VPReg.stg` files, the first that holds them).
pub fn of_values(names: &[String], ours: &Path, vpx: &[PathBuf]) -> HighScores {
    let mut out = HighScores { kind: "script", ..Default::default() };
    for name in names {
        let mut kept = Vec::new();
        if let Some(k) = from_values("ours", ours, &values_in_ini(ours, name)) {
            kept.push(k);
        }
        for file in vpx.iter().filter(|f| f.is_file()) {
            match values_in_stg(file, name) {
                Ok(Some(values)) => {
                    if let Some(k) = from_values("vpx", file, &values) {
                        kept.push(k);
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => out.problem = Some(e),
            }
        }
        if !kept.is_empty() {
            out.under = Some(name.clone());
            out.kept = kept;
            return out;
        }
    }
    out.under = names.first().cloned();
    out
}

/// A table's high scores, where the player and VPX keep them: a table driving PinMAME, `rom_set`
/// (after VPinMAME's aliases), in its machine's memory — our player's in its data folder's
/// `PinMAME\nvram`, VPX's in VPinMAME's `nvram`; else, a table whose script keeps values under
/// `saved_under`, among them — our player's `VPReg.ini` in its data folder, VPX's `VPReg.stg` in
/// the table folder's `user` (VPX 5436 keeping it beside the tables), else VPX's own `User`.
pub fn for_table(layout: &crate::plan::Layout, rom_set: Option<&str>, saved_under: &[String]) -> HighScores {
    if let Some(rom) = rom_set {
        return of_machine(rom, &layout.data.join("PinMAME").join("nvram"), &layout.vpinmame.join("nvram"));
    }
    if !saved_under.is_empty() {
        return of_values(saved_under, &layout.data.join("VPReg.ini"), &vpx_values(layout));
    }
    HighScores { kind: "none", ..Default::default() }
}

/// The last game's scores, each player's, from a machine's memory as our player keeps it, if it
/// changed since `since` (ms since 1970) — a game played since — and its map says where they are.
pub fn last_game(layout: &crate::plan::Layout, rom: &str, since: i64) -> Option<Vec<u64>> {
    let file = layout.data.join("PinMAME").join("nvram").join(format!("{}.nv", rom.to_ascii_lowercase()));
    if changed(&file) < since {
        return None;
    }
    let players = pinmame_nvram::Nvram::open(&file).ok()??.read_last_game().ok()??;
    let scores: Vec<u64> = players.into_iter().map(|p| p.score).collect();
    // a one-player game's other places stay at nothing
    let played = scores.iter().rposition(|&s| s > 0).map_or(1, |n| n + 1);
    Some(scores.into_iter().take(played).collect())
}

/// Where VPX keeps the values scripts save: its `VPReg.stg` in the table folder's `user`, else in
/// its own `User`.
fn vpx_values(layout: &crate::plan::Layout) -> Vec<PathBuf> {
    std::iter::once(layout.tables.join("user").join("VPReg.stg")).chain(layout.tables.parent().map(|v| v.join("User").join("VPReg.stg"))).collect()
}

/// Before a table first plays in our player, VPX's copy of what it keeps taken as ours to start
/// from, so its high scores carry on from VPX's (VPX's pulled in by
/// themselves, no button; a copy from VPX, table by table): a machine's whole memory — its
/// high scores, settings and audits — or the values its script saves under its name, copied into
/// our player's only where ours has none, never in place of ours. VPX's files are only read.
/// Whether anything was taken.
pub fn start_from_vpx(layout: &crate::plan::Layout, rom_set: Option<&str>, saved_under: &[String]) -> Result<bool, String> {
    if let Some(rom) = rom_set {
        let from = layout.vpinmame.join("nvram").join(format!("{rom}.nv"));
        let to = layout.data.join("PinMAME").join("nvram").join(format!("{rom}.nv"));
        if to.exists() || !from.is_file() {
            return Ok(false);
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        return std::fs::copy(&from, &to).map(|_| true).map_err(|e| format!("{}: {e}", to.display()));
    }
    let ours = layout.data.join("VPReg.ini");
    let before = std::fs::read(&ours).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let kept = vpi_settings::parse_ini(&before);
    if saved_under.iter().any(|n| kept.iter().any(|(s, _, _)| s.eq_ignore_ascii_case(n))) {
        return Ok(false);
    }
    for file in vpx_values(layout).into_iter().filter(|f| f.is_file()) {
        for name in saved_under {
            let Some((under, values)) = stg_values(&file, name)? else { continue };
            let changes: Vec<vpi_settings::Change> = values.iter().filter(|(k, _)| !k.contains(['=', '\r', '\n'])).map(|(k, v)| vpi_settings::Change::set(&under, k, v.replace(['\r', '\n'], " "))).collect();
            let after = vpi_settings::edit(&before, &changes);
            if let Some(dir) = ours.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let new = ours.with_file_name("VPReg.ini.new");
            return std::fs::write(&new, after).and_then(|_| std::fs::rename(&new, &ours)).map(|_| true).map_err(|e| format!("{}: {e}", ours.display()));
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_never_filled_has_no_initials() {
        assert_eq!(initials("\\u00FF\\u00FF\\u00FF"), "");
        assert_eq!(initials(" SLL\0"), "SLL");
        assert_eq!(initials("A\\u00C4B"), "AB");
        assert!(!filled(&Entry { label: Some("King of the Realm #1".into()), initials: initials("\\u00FF\\u00FF\\u00FF"), score: None, extra: Some("2025-01-02 22:18".into()) }));
        assert!(filled(&Entry { label: None, initials: String::new(), score: Some(52_000_000), extra: None }));
    }

    #[test]
    fn a_scripts_scores_from_its_values() {
        let values: BTreeMap<String, String> = [
            ("highscore2", "500000000"),
            ("highscore1", "7.5E+08"),
            ("highscore1name", "AAA"),
            ("highscore2name", "BBB "),
            ("credits", "3"),
            ("totalgamesplayed", "12"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let k = from_values("ours", Path::new("VPReg.ini"), &values).unwrap();
        assert_eq!(k.scores.iter().map(|e| (e.initials.as_str(), e.score)).collect::<Vec<_>>(), [("AAA", Some(750_000_000)), ("BBB", Some(500_000_000))]);
        assert_eq!(k.games_played, Some(12));
        assert_eq!(from_values("ours", Path::new("x"), &BTreeMap::new()), None);
    }

    #[test]
    fn vpxs_values_taken_as_ours_to_start_from() {
        use std::io::Write as _;
        let root = std::env::temp_dir().join(format!("vpi-scores-over-{}", std::process::id()));
        let layout = crate::plan::Layout::under(&root);
        std::fs::create_dir_all(root.join("User")).unwrap();
        std::fs::create_dir_all(&layout.data).unwrap();
        // VPX's file as VPX writes it: a storage for the name, a stream of UTF-16 text for each value
        let stg_path = root.join("User").join("VPReg.stg");
        let mut stg = cfb::create(&stg_path).unwrap();
        stg.create_storage("/Leprechaun").unwrap();
        for (name, value) in [("HighScore1", "1300000"), ("HighScore1Name", "SCO"), ("TotalGamesPlayed", "2")] {
            let units: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
            stg.create_stream(format!("/Leprechaun/{name}")).unwrap().write_all(&units).unwrap();
        }
        drop(stg);
        std::fs::write(layout.data.join("VPReg.ini"), "[other]\nA = 1\n").unwrap();
        let names = ["leprechaun".to_string()];
        assert_eq!(for_table(&layout, None, &names).kept.iter().map(|k| k.by).collect::<Vec<_>>(), ["vpx"]);
        let took = start_from_vpx(&layout, None, &names).unwrap();
        let ours = std::fs::read_to_string(layout.data.join("VPReg.ini")).unwrap();
        let s = for_table(&layout, None, &names);
        // ours once there: never taken again
        std::fs::write(layout.data.join("VPReg.ini"), "[leprechaun]\nHighScore1 = 5\n").unwrap();
        let again = start_from_vpx(&layout, None, &names).unwrap();
        let after = std::fs::read_to_string(layout.data.join("VPReg.ini")).unwrap();
        // a machine's memory the same way
        std::fs::create_dir_all(layout.vpinmame.join("nvram")).unwrap();
        std::fs::write(layout.vpinmame.join("nvram").join("afm_113b.nv"), b"vpx's").unwrap();
        let machine = start_from_vpx(&layout, Some("afm_113b"), &[]).unwrap();
        let copied = std::fs::read(layout.data.join("PinMAME").join("nvram").join("afm_113b.nv")).unwrap();
        std::fs::write(layout.vpinmame.join("nvram").join("afm_113b.nv"), b"newer").unwrap();
        let machine_again = start_from_vpx(&layout, Some("afm_113b"), &[]).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(took && ours.contains("[other]\nA = 1") && ours.contains("HighScore1Name = SCO"), "{ours}");
        assert_eq!(s.kept[0].scores[0].score, Some(1_300_000));
        assert_eq!(s.kept[0].games_played, Some(2));
        assert!(!again && after == "[leprechaun]\nHighScore1 = 5\n", "{after}");
        assert!(machine && copied == b"vpx's" && !machine_again);
    }

    #[test]
    fn our_values_file_read_by_name_in_any_case() {
        let dir = std::env::temp_dir().join(format!("vpi-scores-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("VPReg.ini");
        std::fs::write(&file, "[gameofthrones]\nHighScore1 = 750000000\nHighScore1Name = AAA\n[other]\nHighScore1 = 5\n").unwrap();
        let s = of_values(&["GameOfThrones".to_string()], &file, &[]);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((s.kind, s.under.as_deref(), s.kept.len()), ("script", Some("GameOfThrones"), 1));
        assert_eq!(s.kept[0].scores[0].score, Some(750_000_000));
    }
}
