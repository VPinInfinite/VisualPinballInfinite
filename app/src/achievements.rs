//! Achievements, the app's side: the packs kept in the data folder (`Achievements\packs\<pack
//! ID>\<version>\`, each a pack's `achievements.json` and its pictures), added from a file the user
//! chooses (a pack's zip or its `achievements.json`, checked first) and removed to the Recycle Bin;
//! which packs fit a table (its VPS game, and the tables and ROM sets a pack names); whether a table
//! plays hardcore or softcore (softcore — `practice` in the records — where its physics, script,
//! options or difficulty aren't its author's, or PinMAME's cheat mode is on); what's earned
//! (`Achievements\earned.json`, which our add-on in VPX writes as the game plays, else the app once
//! it's over); the file handed to the game at PLAY (`VPI_ACHIEVEMENTS`); and which achievements a
//! table's game can be heard for in VPX as its settings stand (`heard_for`). A pack's pictures are
//! served by the pictures protocol (`h/<pack ID>/<achievement ID or "pack">`).

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};
use vpi_achievements::earned::Earned;
use vpi_achievements::pack::{Pack, version_key};

use crate::library::{self, Shared, Table};

fn folder() -> Option<PathBuf> {
    Some(library::data_folder()?.join("Achievements"))
}

pub fn earned_file() -> Option<PathBuf> {
    Some(folder()?.join("earned.json"))
}

/// Every pack kept, its newest version of each: its folder and what it says.
pub fn installed() -> Vec<(PathBuf, Pack)> {
    let Some(root) = folder().map(|f| f.join("packs")) else { return Vec::new() };
    let mut out: Vec<(PathBuf, Pack)> = Vec::new();
    for pack_dir in std::fs::read_dir(&root).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let newest = std::fs::read_dir(&pack_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter_map(|dir| Pack::read(&dir).ok().map(|p| (dir, p)))
            .max_by_key(|(_, p)| version_key(&p.pack.version));
        out.extend(newest);
    }
    out.sort_by(|a, b| a.1.pack.name.to_lowercase().cmp(&b.1.pack.name.to_lowercase()));
    out
}

/// Whether a pack is made for a table: its VPS game (else its name, maker and year), the VPS
/// table files it names if it names any, and the ROM sets it names if it names any.
pub fn fits(st: &library::State, t: &Table, pack: &Pack) -> bool {
    let f = &pack.made_for;
    let game = match (&f.game.vps, &t.on_vps.game) {
        (Some(want), Some(have)) => want == have,
        _ => {
            let view = library::view(st, t);
            let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
            same(&f.game.name, &view.name) && f.game.year.is_none_or(|y| view.year == Some(y)) && f.game.maker.as_deref().is_none_or(|m| view.maker.as_deref().is_some_and(|v| same(m, v)))
        }
    };
    let tables = f.tables.iter().filter_map(|x| x.vps.as_deref()).collect::<Vec<_>>();
    let table = tables.is_empty() || t.on_vps.table_file.as_deref().is_some_and(|id| tables.contains(&id));
    let rom = f.roms.is_empty() || t.holds.rom_set.as_deref().is_some_and(|r| f.roms.iter().any(|x| x.eq_ignore_ascii_case(r)));
    game && table && rom
}

/// Why a table plays practice rather than hardcore, if it does: its physics or script not its
/// author's, or PinMAME's cheat mode on for its game.
pub fn practice_because(st: &library::State, t: &Table) -> Vec<String> {
    let mut why = Vec::new();
    if let Some(p) = &t.holds.physics
        && !p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("autovpp.vpp"))
    {
        why.push("its physics are changed (its own physics file)".to_string());
    }
    if let Some(set) = t.read.physics_set
        && physics_set_changed(set)
    {
        why.push(format!("it plays VPX's physics set {set}, which you've changed"));
    }
    if (t.holds.script.is_some() && !only_its_rom_changed_in(st, t)) || st.user.script_edits.contains_key(&library::key(&t.path)) {
        why.push("its script is edited".to_string());
    }
    if let Some(rom) = &t.holds.rom_set
        && cheat_on(rom)
    {
        why.push("PinMAME's cheat mode is on for its game".to_string());
    }
    // its own settings: its script's options, or its difficulty, changed from its author's
    if let Ok(bytes) = std::fs::read(t.path.with_extension("ini")) {
        let entries = vpi_settings::parse_ini(&String::from_utf8_lossy(&bytes));
        if entries.iter().any(|(s, _, v)| s.eq_ignore_ascii_case("TableOption") && !v.trim().is_empty()) {
            why.push("its options are changed in its table settings".to_string());
        }
        if entries.iter().any(|(s, k, v)| s.eq_ignore_ascii_case("TableOverride") && k.eq_ignore_ascii_case("Difficulty") && !v.trim().is_empty()) {
            why.push("its difficulty is changed".to_string());
        }
    }
    why
}

/// Whether the script beside a table is only its ROM changed on its page (`library::change_rom`):
/// its own script with the game name it hands PinMAME changed, every other byte as it was — the
/// table as its author made it, on another of its machine's ROMs, which the packs say they fit or
/// not (`fits`).
pub fn only_its_rom_changed(table: &Path, change: Option<&::library::rom_change::Changed>, edited: bool) -> bool {
    let Some(c) = change.filter(|c| c.made && !edited) else { return false };
    let (Ok(beside), Ok(own)) = (std::fs::read(table.with_extension("vbs")), ::library::table::own_script(table)) else { return false };
    ::library::rom_change::renamed_in(&own, &c.from, &c.to).is_ok_and(|r| r == beside)
}

/// `only_its_rom_changed` as the library remembers the table's changes.
pub fn only_its_rom_changed_in(st: &library::State, t: &Table) -> bool {
    let key = library::key(&t.path);
    only_its_rom_changed(&t.path, st.user.rom_changes.get(&key), st.user.script_edits.contains_key(&key))
}

/// Whether VPX's physics set `set` (1 to 8, as a table names it) has values of the user's in the
/// settings (`[Player]` `TablePhysics…<n>`, `FlipperPhysics…<n>`, numbered from 0), rather than
/// VPX's own.
fn physics_set_changed(set: u32) -> bool {
    let n = set.saturating_sub(1).to_string();
    vpi_settings::read_all().iter().any(|(s, k, v)| {
        s.eq_ignore_ascii_case("Player") && (k.starts_with("TablePhysics") || k.starts_with("FlipperPhysics")) && k.ends_with(&n) && !k[..k.len() - n.len()].ends_with(|c: char| c.is_ascii_digit()) && !v.trim().is_empty()
    })
}

/// PinMAME's cheat mode for a game, as the PinMAME that plays it has it: VPX's own add-on's, for
/// every game (`[Plugin.PinMAME]` `Cheat`), where it's on; else the classic VPinMAME's for the game,
/// or its default (`machines::value`).
fn cheat_on(rom: &str) -> bool {
    let settings = vpi_settings::read_all();
    let plugin = |key: &str| vpi_settings::get(&settings, "Plugin.PinMAME", key).and_then(vpi_settings::vpx_bool);
    if plugin("Enable").unwrap_or(false) {
        plugin("Cheat").unwrap_or(false)
    } else {
        crate::machines::value(rom, "cheat").is_some_and(|v| v == 1)
    }
}

/// How PLAY was asked to play a table for achievements: as it stands, Play Hardcore (what makes it
/// softcore left out for this game), or Play Softcore (the player's choice: a guest, say).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    AsItStands,
    Hardcore,
    Softcore,
}

/// The file handed to the player at PLAY for a table with packs, in the data folder; None for one
/// without.
pub fn for_play(st: &library::State, t: &Table, asked: Asked) -> Option<PathBuf> {
    let packs: Vec<PathBuf> = installed().into_iter().filter(|(_, p)| fits(st, t, p)).map(|(d, _)| d).collect();
    if packs.is_empty() {
        return None;
    }
    let why = match asked {
        Asked::Hardcore => Vec::new(),
        Asked::Softcore => vec!["you chose to play it softcore".to_string()],
        Asked::AsItStands => practice_because(st, t),
    };
    let mode = if why.is_empty() { "hardcore" } else { "practice" };
    let file = folder()?.join("playing.json");
    let asked = serde_json::json!({
        "mode": mode,
        "why": (!why.is_empty()).then(|| why.join("; ")),
        "table": t.path.file_name().map(|n| n.to_string_lossy().into_owned()),
        "packs": packs,
        "earned": earned_file()?,
    });
    std::fs::create_dir_all(file.parent()?).ok()?;
    std::fs::write(&file, serde_json::to_vec_pretty(&asked).ok()?).ok()?;
    Some(file)
}

/// What a game played in VPX left for its achievements, as the app reads it once VPX has quit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameLeft {
    /// The games played.
    pub games: u32,
    /// Each player's score in the last of them, where it's known.
    pub scores: Vec<u64>,
}

/// The achievements a game played in VPX earned, counted once VPX has quit by the packs the file
/// handed to the game names (`for_play`, or the achievements editor's pack being tried), as our
/// player counts them while a game plays — from what the game left: each game begun and over, each
/// player's score in the last one — and recorded in the file it names, hardcore or practice as it
/// says. What was earned, by name.
pub fn count_after(asked: &Path, left: &GameLeft) -> Result<Vec<String>, String> {
    use vpi_achievements::cond::Signal;
    let bytes = std::fs::read(asked).map_err(|e| format!("{}: {e}", asked.display()))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", asked.display()))?;
    let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_string);
    let file = text("earned").map(PathBuf::from).ok_or("it names no file for what's earned")?;
    let packs: Vec<Pack> = v.get("packs").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(|d| d.as_str()).filter_map(|d| Pack::read(Path::new(d)).ok()).collect();
    let mut earned = Earned::load(&file);
    let mut watcher = vpi_achievements::watch::Watcher::new(packs, &text("mode").unwrap_or_else(|| "practice".into()), text("table"), &earned);
    let (mut names, mut changed, mut now) = (Vec::new(), false, 0u64);
    for game in 1..=left.games {
        let mut signals = vec![Signal::GameStarted];
        if game == left.games {
            for (i, &score) in left.scores.iter().enumerate() {
                signals.push(Signal::Player(i as u32 + 1));
                signals.push(Signal::Score { player: i as u32 + 1, score });
            }
        }
        signals.push(Signal::GameOver);
        for signal in signals {
            now += 1000;
            let earns = watcher.signal(signal, now, &mut earned);
            changed |= watcher.take_progress_moved() || !earns.is_empty();
            names.extend(earns.into_iter().map(|e| e.achievement.name));
        }
    }
    if changed {
        earned.save(&file)?;
    }
    Ok(names)
}

/// One achievement as the screens show it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementView {
    pub id: String,
    pub name: String,
    pub about: String,
    pub points: u32,
    pub secret: bool,
    /// Its picture, through the pictures protocol, where the pack has one.
    pub icon: Option<String>,
    /// When it was earned, and how: the hardcore record where there's one, else the practice one.
    pub earned: Option<(String, String)>,
    /// When it was earned in hardcore, and in softcore (`practice` in the records), each if it was.
    pub hardcore: Option<String>,
    pub softcore: Option<String>,
    /// Why it can't be earned as the table plays in VPX now, where it can't: what it counts that the
    /// game isn't heard for.
    pub unheard: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub about: Option<String>,
    pub icon: Option<String>,
    pub game: String,
    pub achievements: Vec<AchievementView>,
    /// Each version mastered, and when.
    pub mastered: Vec<(String, String)>,
    /// The tables of the library it's made for, by ID and name.
    pub tables: Vec<(String, String)>,
}

/// The kinds of signal (`Cond::needs`' names) a table's game is heard for as it plays in VPX, as
/// VPX's settings stand: through our add-on, with VPX's own PinMAME add-on, a ROM machine's game
/// state where its memory map is known, and its lamps, switches and solenoids; with VPX's own B2S
/// add-ons, the game a script tells its backglass; always, what a game leaves once it's over — its
/// games and scores. With the add-on of VPX's own that would hear more, where a classic one plays it.
/// A machine whose controller is the backglass server (`controller.vbs`'s, unless VPX's settings
/// turn it off) plays inside it: the classic B2S Server runs the classic VPinMAME, never VPX's own
/// PinMAME — VPX's own backglass add-on runs VPX's own. VPX 10.8.0 (`older`) takes no add-ons:
/// only what a game leaves. `balls` and `lights`: the VPX that plays shares the balls in play
/// (`BALLS_FROM`) and the table's lights (`LIGHTS_FROM`) with our add-on.
pub fn heard_for(t: &Table, older: bool, balls: bool, lights: bool) -> Heard {
    use vpi_achievements::cond::{FROM_A_BACKGLASS, FROM_A_MACHINE, FROM_THE_BALLS, FROM_THE_LIGHTS, LEFT_BY_A_GAME};
    if older {
        let mut heard: Vec<&'static str> = LEFT_BY_A_GAME.to_vec();
        heard.sort_unstable();
        heard.dedup();
        return Heard { heard, via: Some(OLDER) };
    }
    let settings = vpi_settings::read_all();
    let on = |id: &str| vpi_settings::get(&settings, &format!("Plugin.{id}"), "Enable").and_then(vpi_settings::vpx_bool).unwrap_or(false);
    let mut heard: Vec<&'static str> = LEFT_BY_A_GAME.to_vec();
    let addon = ::library::vpx_addon::built().is_some();
    let mut via = None;
    let b2s_off = vpi_settings::get(&settings, "Controller", "ForceDisableB2S").and_then(vpi_settings::vpx_bool).unwrap_or(false);
    let in_classic_b2s = t.read.b2s_controller && !b2s_off && !(on("B2S") || on("B2SLegacy")) && crate::addons::registered("B2S.Server");
    match &t.holds.rom_set {
        Some(rom) if addon && on("PinMAME") && !in_classic_b2s => {
            if let Some((map, _, _)) = ::library::vpx_addon::map_of(rom) {
                heard.extend(FROM_A_MACHINE);
                if map["game_state"].get("tilted").is_some() {
                    heard.push("tilt");
                }
            }
        }
        Some(_) if on("PinMAME") && in_classic_b2s => via = Some("B2S"),
        Some(_) if in_classic_b2s => via = Some("PinMAME and B2S"),
        Some(_) => via = Some("PinMAME"),
        None if addon && (on("B2S") || on("B2SLegacy")) => heard.extend(FROM_A_BACKGLASS),
        None => via = Some("B2S"),
    }
    // any table's balls in play, through a line of its own script our add-on runs
    if addon && balls {
        heard.extend(FROM_THE_BALLS);
    }
    // any table's lights, which VPX shares with add-ons by their names
    if addon && lights {
        heard.extend(FROM_THE_LIGHTS);
    }
    heard.sort_unstable();
    heard.dedup();
    Heard { heard, via }
}

/// The first VPX build whose add-ons can run a line of a table's own script, so our add-on hears the
/// balls in play: the newer add-on interface (VPX's 595d1fd5e3; `RunScript` since its 1075904b4b).
pub const BALLS_FROM: u16 = 5575;

/// The first VPX build that shares a table's lights with add-ons, each by its name in the table:
/// VPX's "Lights" state group (its 513018dc79).
pub const LIGHTS_FROM: u16 = 6148;

/// What a table's game is heard for in VPX (`heard_for`), and the add-on of VPX's own that would
/// hear more, where a classic one plays it.
pub struct Heard {
    pub heard: Vec<&'static str>,
    pub via: Option<&'static str>,
}

/// Kinds VPX doesn't share with add-ons at all: the table's parts, its balls, its script's events.
const NEVER_SHARED: [&str; 3] = ["part", "ballsInPlay", "script"];

/// `Heard::via` where VPX 10.8.0 plays: VPX 10.8.1 would hear more.
const OLDER: &str = "VPX 10.8.1";

/// Why an achievement whose condition is `when` can't be earned as `heard` hears the game, in
/// words; None where it can.
fn unheard(when: &serde_json::Value, heard: &Heard) -> Option<String> {
    let cond = vpi_achievements::cond::Cond::parse(when).ok()?;
    let mut needs = Vec::new();
    cond.needs(&mut needs);
    let missing: Vec<&str> = needs.into_iter().filter(|n| !heard.heard.contains(n)).collect();
    if missing.is_empty() {
        return None;
    }
    let mut words: Vec<&str> = Vec::new();
    let mut say = |kinds: &[&str], w: &'static str| {
        if missing.iter().any(|m| kinds.contains(m)) && !words.contains(&w) {
            words.push(w);
        }
    };
    say(&["part"], "hits on the table's parts");
    say(&["ballsInPlay"], "how many balls are in play");
    say(&["script"], "events the table's script tells");
    say(&["light"], "the table's lights");
    say(&["lamp", "switch", "solenoid"], "the machine's lamps, switches and coils");
    say(&["ball", "credits", "extraBall", "tilt"], "the ball in play, credits, extra balls and tilts");
    let list = match words.as_slice() {
        [] => return None,
        [one] => (*one).to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    if missing.iter().any(|m| NEVER_SHARED.contains(m)) {
        Some(format!("Can't be earned in VPX yet: it counts {list}, which VPX doesn't share with add-ons."))
    } else if heard.via == Some(OLDER) {
        Some(format!("Earned only when VPX 10.8.1 plays the table (Settings › Library): it counts {list}, which VPX 10.8.0 shares with nothing."))
    } else if missing.contains(&"light") {
        Some(format!("Earned with a newer VPX 10.8.1, from build {LIGHTS_FROM}, which shares a table's lights with add-ons: it counts {list}."))
    } else {
        let via = heard.via.unwrap_or("PinMAME");
        let add_ons = if via.contains(" and ") { "add-ons" } else { "add-on" };
        Some(format!("Earned with VPX's own {via} {add_ons}, chosen in Settings › Library: it counts {list}."))
    }
}

fn view_of(dir: &Path, pack: &Pack, earned: &Earned, tables: Vec<(String, String)>) -> PackView {
    view_heard(dir, pack, earned, tables, None)
}

fn view_heard(dir: &Path, pack: &Pack, earned: &Earned, tables: Vec<(String, String)>, heard: Option<&Heard>) -> PackView {
    let base = crate::pictures::base();
    let mine = earned.packs.get(&pack.pack.id);
    let icon = |which: &str, has: bool| has.then(|| format!("{base}h/{}/{which}", pack.pack.id));
    PackView {
        id: pack.pack.id.clone(),
        name: pack.pack.name.clone(),
        version: pack.pack.version.clone(),
        authors: pack.pack.authors.clone(),
        about: pack.pack.about.clone(),
        icon: icon("pack", pack.pack.icon.as_ref().is_some_and(|i| dir.join(i).is_file())),
        game: pack.made_for.game.name.clone(),
        achievements: pack
            .achievements
            .iter()
            .map(|a| {
                let records = mine.and_then(|m| m.achievements.get(&a.id));
                let earned = records.and_then(|rs| rs.iter().find(|r| r.mode == "hardcore").or_else(|| rs.first())).map(|r| (r.at.clone(), r.mode.clone()));
                let when = |mode: &str| records.and_then(|rs| rs.iter().find(|r| r.mode == mode)).map(|r| r.at.clone());
                AchievementView {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    about: a.about.clone(),
                    points: a.points,
                    secret: a.secret,
                    icon: icon(&a.id, pack.icon_of(dir, a).is_some()),
                    earned,
                    hardcore: when("hardcore"),
                    softcore: when("practice"),
                    unheard: heard.and_then(|h| unheard(&a.when, h)),
                }
            })
            .collect(),
        mastered: mine.map(|m| m.mastered.iter().map(|x| (x.pack_version.clone(), x.at.clone())).collect()).unwrap_or_default(),
        tables,
    }
}

/// A table's packs, with what's earned, and how it plays: practice, and why, or hardcore.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableAchievements {
    pub packs: Vec<PackView>,
    pub practice: Vec<String>,
}

/// How PLAY plays a table: how many packs it has, and what makes it softcore (none: hardcore).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayMode {
    pub packs: usize,
    pub softcore: Vec<String>,
}

#[tauri::command]
pub fn play_mode(shared: State<'_, Shared>, id: String) -> Result<PlayMode, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let packs = installed().into_iter().filter(|(_, p)| fits(&st, t, p)).count();
    Ok(PlayMode { packs, softcore: if packs > 0 { practice_because(&st, t) } else { Vec::new() } })
}

#[tauri::command]
pub fn achievements_for(shared: State<'_, Shared>, id: String) -> Result<TableAchievements, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let earned = earned_file().map(|f| Earned::load(&f)).unwrap_or_default();
    // the VPX this table plays with: Settings › Library's, or the one chosen for it
    let layout = library::layout_for(&st, &t.path);
    let older = layout.as_ref().is_some_and(::library::vpx::plays_older);
    let plays = layout.as_ref().and_then(::library::vpx::to_play);
    let balls = plays.as_ref().is_some_and(|v| v.version >= [10, 8, 1, BALLS_FROM]);
    let lights = plays.as_ref().is_some_and(|v| v.version >= [10, 8, 1, LIGHTS_FROM]);
    let heard = heard_for(t, older, balls, lights);
    let packs = installed().into_iter().filter(|(_, p)| fits(&st, t, p)).map(|(d, p)| view_heard(&d, &p, &earned, Vec::new(), Some(&heard))).collect();
    Ok(TableAchievements { packs, practice: practice_because(&st, t) })
}

/// Every pack kept, with the library's tables each is made for.
#[tauri::command]
pub fn achievements_all(shared: State<'_, Shared>) -> Vec<PackView> {
    let st = shared.0.lock().unwrap();
    let earned = earned_file().map(|f| Earned::load(&f)).unwrap_or_default();
    installed()
        .into_iter()
        .map(|(d, p)| {
            let tables = st.tables.iter().filter(|t| fits(&st, t, &p)).map(|t| (t.id.clone(), library::view(&st, t).name)).collect();
            view_of(&d, &p, &earned, tables)
        })
        .collect()
}

/// A pack's picture: the pack's own (`pack`), or an achievement's (its ID), the pack's where it
/// has none of its own.
pub fn icon(pack_id: &str, which: &str) -> Option<PathBuf> {
    let (dir, pack) = installed().into_iter().find(|(_, p)| p.pack.id == pack_id)?;
    if which == "pack" {
        return pack.pack.icon.as_ref().map(|i| dir.join(i)).filter(|p| p.is_file() && p.starts_with(&dir));
    }
    let a = pack.achievements.iter().find(|a| a.id == which)?;
    pack.icon_of(&dir, a).filter(|p| p.starts_with(&dir))
}

/// A pack added from a file the user chooses in Windows' picker — a pack's zip, or its
/// `achievements.json` alone — checked first, kept as its ID and version say; its name.
#[tauri::command]
pub async fn achievements_add(app: AppHandle, window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    let name = tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        let kinds = [("An achievement pack", "*.zip;achievements.json;*.json")];
        let Some(picked) = crate::pick::file(owner, "The achievement pack to add", &kinds)? else { return Ok(None) };
        add_from(&picked).map(Some)
    })
    .await
    .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(name)
}

/// A pack's zip, or its `achievements.json`, checked and kept.
pub fn add_from(picked: &Path) -> Result<String, String> {
    let root = folder().ok_or("no APPDATA folder")?.join("packs");
    let passing = root.join(format!(".adding-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&passing);
    std::fs::create_dir_all(&passing).map_err(|e| format!("{}: {e}", passing.display()))?;
    let unpacked = (|| -> Result<Pack, String> {
        if picked.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
            let file = std::fs::File::open(picked).map_err(|e| format!("{}: {e}", picked.display()))?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            // the pack's own files, at its top or in the one folder it holds
            let names: Vec<String> = zip.file_names().map(str::to_string).collect();
            let top = names.iter().find(|n| n.rsplit('/').next() == Some("achievements.json")).ok_or("It holds no achievements.json.")?;
            let prefix = top.strip_suffix("achievements.json").unwrap_or("").to_string();
            for i in 0..zip.len() {
                let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
                let Some(rel) = f.enclosed_name().and_then(|p| p.to_str().map(|s| s.replace('\\', "/"))) else { continue };
                let Some(rel) = rel.strip_prefix(&prefix).map(str::to_string) else { continue };
                if f.is_dir() || rel.is_empty() {
                    continue;
                }
                let to = passing.join(&rel);
                std::fs::create_dir_all(to.parent().unwrap_or(&passing)).map_err(|e| e.to_string())?;
                let mut out = std::fs::File::create(&to).map_err(|e| format!("{}: {e}", to.display()))?;
                std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
            }
        } else {
            std::fs::copy(picked, passing.join("achievements.json")).map_err(|e| format!("{}: {e}", picked.display()))?;
            // the pictures and sounds it names, from beside it
            let beside = picked.parent().unwrap_or(Path::new("."));
            let pack = Pack::read(&passing)?;
            let named = pack
                .pack
                .icon
                .iter()
                .chain(pack.achievements.iter().filter_map(|a| a.icon.as_ref()))
                .chain(pack.pack.sound.iter())
                .chain(pack.achievements.iter().filter_map(|a| a.sound.as_ref()));
            for icon in named {
                let (from, to) = (beside.join(icon), passing.join(icon));
                if !icon.contains("..") && from.is_file() {
                    std::fs::create_dir_all(to.parent().unwrap_or(&passing)).map_err(|e| e.to_string())?;
                    std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
                }
            }
        }
        let pack = Pack::read(&passing)?;
        let problems = pack.problems(Some(&passing));
        if !problems.is_empty() {
            return Err(format!("That pack isn't right: {}.", problems.join("; ")));
        }
        Ok(pack)
    })();
    let pack = match unpacked {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&passing);
            return Err(e);
        }
    };
    let to = root.join(&pack.pack.id).join(&pack.pack.version);
    if to.exists() {
        let _ = std::fs::remove_dir_all(&passing);
        return Err(format!("{} {} is kept already.", pack.pack.name, pack.pack.version));
    }
    std::fs::create_dir_all(to.parent().unwrap_or(&root)).map_err(|e| e.to_string())?;
    std::fs::rename(&passing, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(format!("{} {}", pack.pack.name, pack.pack.version))
}

/// What `add_from` takes for a dropped file or folder that's an achievement pack — its
/// `achievements.json`, a folder holding one, or a zip holding one — told by what it says it is;
/// None for anything else.
pub fn pack_in(path: &Path) -> Option<PathBuf> {
    let ours = |text: &str| Pack::parse(text).is_ok();
    if path.is_dir() {
        let json = path.join("achievements.json");
        return std::fs::read_to_string(&json).ok().filter(|t| ours(t)).map(|_| json);
    }
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name == "achievements.json" {
        return std::fs::read_to_string(path).ok().filter(|t| ours(t)).map(|_| path.to_path_buf());
    }
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
        let entry = zip.file_names().find(|n| n.rsplit('/').next() == Some("achievements.json"))?.to_string();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(&entry).ok()?, &mut text).ok()?;
        return ours(&text).then(|| path.to_path_buf());
    }
    None
}

/// Files dropped onto the app: the achievement packs among them added at once, the rest handed
/// back for the drop window. What was added, and the rest.
#[tauri::command]
pub async fn achievements_drop(app: AppHandle, paths: Vec<String>) -> Result<(Vec<String>, Vec<String>), String> {
    let (added, rest) = tauri::async_runtime::spawn_blocking(move || {
        let mut added = Vec::new();
        let mut rest = Vec::new();
        for p in paths {
            match pack_in(Path::new(&p)) {
                Some(from) => added.push(add_from(&from).unwrap_or_else(|e| format!("not added: {e}"))),
                None => rest.push(p),
            }
        }
        (added, rest)
    })
    .await
    .map_err(|e| e.to_string())?;
    if !added.is_empty() {
        library::start(app);
    }
    Ok((added, rest))
}

/// A pack deleted, every version of it, for good: the screens warn first that it can't be
/// recovered. What's earned of it stays.
#[tauri::command]
pub fn achievements_remove(app: AppHandle, id: String) -> Result<(), String> {
    let dir = folder().ok_or("no APPDATA folder")?.join("packs").join(&id);
    if id.contains(['/', '\\', '.']) || !dir.is_dir() {
        return Err("That pack isn't kept.".into());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("It couldn't be deleted: {e}"))?;
    library::start(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_vpx_doesnt_share_said_in_words() {
        let light = serde_json::json!({ "light": "rain1", "is": "on" });
        let newest = Heard { heard: vec!["game", "score", "ballsInPlay", "light"], via: None };
        assert_eq!(unheard(&light, &newest), None);
        // a 10.8.1 from before VPX shared a table's lights
        let before = Heard { heard: vec!["game", "score", "ballsInPlay"], via: None };
        let said = unheard(&light, &before).unwrap();
        assert!(said.starts_with("Earned with a newer VPX 10.8.1, from build 6148") && said.ends_with("it counts the table's lights."), "{said}");
        // VPX 10.8.0, and what no VPX shares
        let older = Heard { heard: vec!["game", "score"], via: Some(OLDER) };
        assert!(unheard(&light, &older).unwrap().starts_with("Earned only when VPX 10.8.1 plays the table"));
        let part = serde_json::json!({ "part": "Bumper1", "does": "hit" });
        assert!(unheard(&part, &newest).unwrap().starts_with("Can't be earned in VPX yet: it counts hits on the table's parts"));
    }

    /// Run alone, as it sets `APPDATA` for the process: `cargo test -p app --release achievements -- --ignored`.
    #[test]
    #[ignore]
    fn a_zipped_pack_added_once_and_a_wrong_one_refused() {
        let data = std::env::temp_dir().join(format!("vpi-ach-add-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).unwrap();
        let pack = r#"{"format":"vpi-achievements","formatVersion":1,"pack":{"id":"8d6c2f0e-4c9a-4b8e-9f51-3a7d2b1c9e40","name":"AFM","version":"1.0","authors":["x"],"icon":"icons/pack.png"},"for":{"game":{"name":"Attack from Mars"}},"achievements":[{"id":"a","name":"A","about":"a","points":5,"since":"1.0","icon":"icons/a.png","when":{"game":"started"}}]}"#;
        let zipped = data.join("pack.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zipped).unwrap());
        for (name, bytes) in [("AFM pack/achievements.json", pack.as_bytes()), ("AFM pack/icons/pack.png", b"png".as_slice()), ("AFM pack/icons/a.png", b"png".as_slice())] {
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, bytes).unwrap();
        }
        w.finish().unwrap();
        let wrong = data.join("wrong.json");
        std::fs::write(&wrong, pack.replace("\"points\":5", "\"points\":500")).unwrap();
        // a pack as a folder, with a sound of its own beside its pictures
        let folder = data.join("folder pack");
        std::fs::create_dir_all(folder.join("icons")).unwrap();
        std::fs::create_dir_all(folder.join("sounds")).unwrap();
        let with_sound = pack.replace("8d6c2f0e-4c9a-4b8e-9f51-3a7d2b1c9e40", "1b2c3d4e-5f60-4a1b-8c2d-3e4f5a6b7c8d").replace("\"icon\":\"icons/pack.png\"", "\"icon\":\"icons/pack.png\",\"sound\":\"sounds/ding.wav\"");
        std::fs::write(folder.join("achievements.json"), with_sound).unwrap();
        std::fs::write(folder.join("icons").join("pack.png"), b"png").unwrap();
        std::fs::write(folder.join("icons").join("a.png"), b"png").unwrap();
        let mut wav = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x00\x00\x00\x00".to_vec();
        wav.extend_from_slice(&[0, 0, 0, 0]);
        std::fs::write(folder.join("sounds").join("ding.wav"), &wav).unwrap();
        // the data folder where the app keeps them, for this check alone
        unsafe { std::env::set_var("APPDATA", &data) };
        let added = add_from(&zipped);
        let again = add_from(&zipped);
        let refused = add_from(&wrong);
        let from_folder = add_from(&folder.join("achievements.json"));
        let kept = installed();
        let sound_kept = folder_of_kept(&data, "1b2c3d4e-5f60-4a1b-8c2d-3e4f5a6b7c8d").map(|d| d.join("1.0").join("sounds").join("ding.wav").is_file());
        let _ = std::fs::remove_dir_all(&data);
        assert_eq!(added.unwrap(), "AFM 1.0");
        assert!(again.unwrap_err().contains("kept already"));
        assert!(refused.unwrap_err().contains("1 to 100"));
        assert_eq!(from_folder.unwrap(), "AFM 1.0");
        assert_eq!(sound_kept, Some(true));
        assert_eq!(kept.len(), 2);
    }

    /// Where a pack added under `data` (the `APPDATA` of the check) is kept, by its ID.
    fn folder_of_kept(data: &Path, id: &str) -> Option<PathBuf> {
        let d = data.join("Visual Pinball Infinite").join("Achievements").join("packs").join(id);
        d.is_dir().then_some(d)
    }
}
