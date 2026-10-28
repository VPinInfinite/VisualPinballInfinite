//! A table's own settings, on its page — the options its script offers (as in VPX's in-game options
//! menu), its view, the machine's settings, its physics settings: kept in the table's own `<table>.ini` beside it, as VPX 5436 keeps them and our player
//! reads them over the main settings — the options its script offers (`[TableOption]`, under build
//! 5436's names; the options themselves asked of the script by a brief run with no window,
//! `vpi-headless --options`), and for this table alone its view (`[Player]` `BGSet`), volumes
//! (`SoundVolume`, `MusicVolume`), VPX 5436's general table settings — its tone mapper, exposure,
//! scene lighting and difficulty (`[TableOverride]` `ToneMapper`, `Exposure`, `Difficulty`;
//! `[Player]` `OverrideTableEmissionScale`, `DynamicDayNight`, `EmissionScale`; its in-game menu's
//! `TableMiscPage`), each starting from the table's own — and sound and colour packs on or off
//! (`[Plugin.AltSound]`, `[Plugin.Serum]`, `[Plugin.VNI]` `Enable`), and its PinUP pack on or off
//! (`[Plugin.PUP]` `Enable`, its page's switch). Saved only where changed; a
//! setting back to the user's usual is taken out of the file. Beside them, for a table that drives
//! PinMAME, the machine's own settings for its game, as VPinMAME keeps them for each game
//! (`Machine settings.ini` beside the settings file, the player's `machine_settings.rs`),
//! shared by every table playing that game; and the physics file VPX reads for it, if it has one.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use ::library::table::OwnLook;
use serde::{Deserialize, Serialize};
use vpi_settings::Change;

/// One option the table's script offers, and its value in the table's settings (None: its default).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableOption {
    pub id: String,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub default: f32,
    /// 1: a share, shown as a percentage.
    pub unit: i32,
    pub names: Option<Vec<String>>,
    #[serde(default)]
    pub value: Option<f32>,
}

/// This table's own choices over the user's usual; None: the usual.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ForTable {
    /// 0 desktop, 1 cabinet, 2 full single screen.
    pub view: Option<i32>,
    pub playfield_volume: Option<i32>,
    pub backglass_volume: Option<i32>,
    /// 0 to 100 (VPX's 0 to 1).
    pub difficulty: Option<i32>,
    /// VPX's tone mappers, 0 to 4 (`OwnLook`).
    pub tone_mapper: Option<i32>,
    /// 0 to 200 (VPX's 0 to 2).
    pub exposure: Option<i32>,
    /// The scene's light, as VPX 5436's menu offers it: 0 the table's own, 1 a level of the user's
    /// own (`light_level`), 2 the sun's where the user lives, at the hour played.
    pub lighting: Option<i32>,
    /// 0 to 100 (VPX's 0 to 1).
    pub light_level: Option<i32>,
    pub altsound: Option<bool>,
    pub colour: Option<bool>,
    /// Its PinUP pack played (`[Plugin.PUP]` `Enable`).
    pub pup: Option<bool>,
    /// A screenshot when a game here makes the high scores (`[VPI]`
    /// `ScreenshotAtHighScore`).
    pub screenshot: Option<bool>,
    /// The windows a screenshot takes (`[VPI]` `ScreenshotScreens`): `playfield`, `backglass`,
    /// `dmd`, `topper`.
    pub screenshot_screens: Option<Vec<String>>,
    /// The screen preset it plays with (`[VPI]` `ScreenPreset`).
    pub screen_preset: Option<String>,
}

/// What holds where the table's own file says nothing: the user's usual settings, else, for VPX's
/// general table settings, the table's own, as its file keeps them.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usual {
    pub view: i32,
    pub playfield_volume: i32,
    pub backglass_volume: i32,
    pub difficulty: i32,
    pub tone_mapper: i32,
    pub exposure: i32,
    pub lighting: i32,
    pub light_level: i32,
    pub altsound: bool,
    pub colour: bool,
    pub pup: bool,
    pub screenshot: bool,
    pub screenshot_screens: Vec<String>,
    /// Those of VPX's general table settings the user's usual settings set; the others are the
    /// table's own.
    pub yours: Vec<&'static str>,
}

/// PinMAME's settings for one game, as VPinMAME keeps them for each game; None: the usual.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MachineChoices {
    pub sound: Option<bool>,
    /// Decibels, -32 to 32.
    pub volume: Option<i32>,
    /// Older machines' recorded sounds played (`samples`).
    pub samples: Option<bool>,
    /// PinMAME's cheat mode, in which some games skip their start-up tests (`cheat`).
    pub cheat: Option<bool>,
    /// The real DMD shows the game (`showpindmd`).
    pub real_dmd: Option<bool>,
    /// The game's colour on a real DMD, `#rrggbb` (`dmd_red`, `dmd_green`, `dmd_blue`).
    pub dmd_colour: Option<String>,
}

/// What holds for a game where its own section says nothing: the file's `[default]` section's,
/// else the player's own (`[Plugin.PinMAME]` in the settings, VPinMAME's defaults).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineUsual {
    pub sound: bool,
    pub volume: i32,
    pub samples: bool,
    pub cheat: bool,
    pub real_dmd: bool,
    pub dmd_colour: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    /// The game its script plays, by the name VPinMAME keys its settings with.
    pub game: String,
    pub file: String,
    pub own: MachineChoices,
    pub usual: MachineUsual,
    /// Whether a real DMD is set up as VPinMAME's display device (`[VPI]` `DmdDevice`), the only
    /// one the DMD's settings here reach.
    pub dmd_device: bool,
    /// What its script sets for the game each time it starts, over these, by VPinMAME's names.
    pub script_sets: BTreeMap<String, i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub options: Vec<TableOption>,
    /// Why the options couldn't be asked of the script, if they couldn't.
    pub options_problem: Option<String>,
    pub for_table: ForTable,
    pub usual: Usual,
    /// For a table that drives PinMAME.
    pub machine: Option<Machine>,
    /// The physics file VPX reads for it before play, if it has one: `<table>.vpp`, else
    /// `autovpp.vpp` beside it (`table_physics::vpp::find`).
    pub physics: Option<String>,
    /// Whether this table may reach outside its own files, its own line in the main settings'
    /// `[ScriptsReachOutside]` (never in its own file, which a download could bring); None: as
    /// every table, `reach_outside_usual` (`[VPI]` `ScriptsReachOutside`).
    pub reach_outside: Option<bool>,
    pub reach_outside_usual: bool,
    pub file: String,
    pub exists: bool,
}

/// What a brief run of the table's script tells.
#[derive(Clone, Debug, Default)]
struct Script {
    options: Vec<TableOption>,
    machine: Option<ScriptMachine>,
}

/// The game the script plays, and what it sets for it.
#[derive(Clone, Debug, Default, Deserialize)]
struct ScriptMachine {
    game: String,
    #[serde(default)]
    set: BTreeMap<String, i32>,
}

/// A table file's size and date, and what its script told.
type AskedOnce = (u64, i64, Script);

/// What each table's script tells, asked once per table file (its size and date).
#[derive(Default)]
pub struct Asked(Mutex<HashMap<PathBuf, AskedOnce>>);

fn stamp(path: &Path) -> (u64, i64) {
    let m = std::fs::metadata(path).ok();
    let modified = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
    (m.map_or(0, |m| m.len()), modified)
}

/// The options the table's script offers, and the game it plays: a brief run with no window, its
/// script started and its options asked (VPX calls a script's option event as the table starts).
fn ask_script(table: &Path) -> Result<Script, String> {
    let runner = ::library::test::runner().ok_or("The headless runner isn't beside the app, so the table's options can't be asked.")?;
    let mut command = Command::new(runner);
    command.arg(table).args(["--script", "--ms", "300", "--options"]).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window
        command.creation_flags(0x0800_0000);
    }
    let out = command.output().map_err(|e| format!("The table couldn't be run to ask its options: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find_map(|l| l.strip_prefix("options: ")).ok_or("The table's script couldn't be run to ask its options.")?;
    let options = serde_json::from_str(line).map_err(|e| format!("The table's options couldn't be read: {e}"))?;
    let machine = text.lines().find_map(|l| l.strip_prefix("machine: ")).and_then(|l| serde_json::from_str(l).ok());
    Ok(Script { options, machine })
}

/// The table's settings file: `<table>.ini` beside it.
fn file_of(table: &Path) -> PathBuf {
    table.with_extension("ini")
}

/// VPinMAME's settings for each game, beside the settings file, as the player reads them.
fn machine_file() -> Option<PathBuf> {
    Some(vpi_settings::settings_path()?.parent()?.join("Machine settings.ini"))
}

/// The physics file VPX reads for `table`, as `table_physics::vpp::find` finds it.
fn physics_of(table: &Path) -> Option<PathBuf> {
    let own = table.with_extension("vpp");
    if own.is_file() {
        return Some(own);
    }
    let auto = table.parent()?.join("autovpp.vpp");
    auto.is_file().then_some(auto)
}

fn read(path: &Path) -> String {
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// The text written in place of the file: beside it first, then moved over it.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".new");
    let new = path.with_file_name(name);
    std::fs::write(&new, text).map_err(|e| format!("{} couldn't be written: {e}", path.display()))?;
    std::fs::rename(&new, path).map_err(|e| format!("{} couldn't be written: {e}", path.display()))
}

fn percent(v: f32) -> i32 {
    (v * 100.0).round() as i32
}

/// A settings file's choices of the kind a table's own file makes; the main settings file reads
/// the same way, under the same names.
fn for_table_of(entries: &[(String, String, String)]) -> ForTable {
    let get = |section: &str, name: &str| vpi_settings::get(entries, section, name).map(|v| vpi_settings::vpx_trim(v).to_string()).filter(|v| !v.is_empty());
    let int = |section: &str, name: &str, min: i32, max: i32| get(section, name).and_then(|v| vpi_settings::vpx_int(&v)).filter(|v| (min..=max).contains(v));
    let share = |section: &str, name: &str, max: f32| get(section, name).and_then(|v| vpi_settings::vpx_float(&v)).filter(|v| (0.0..=max).contains(v)).map(percent);
    let flag = |section: &str, name: &str| get(section, name).as_deref().and_then(vpi_settings::vpx_bool);
    ForTable {
        view: int("Player", "BGSet", 0, 2),
        playfield_volume: int("Player", "SoundVolume", 0, 100),
        backglass_volume: int("Player", "MusicVolume", 0, 100),
        difficulty: share("TableOverride", "Difficulty", 1.0),
        tone_mapper: int("TableOverride", "ToneMapper", 0, 4),
        exposure: share("TableOverride", "Exposure", 2.0),
        // as VPX 5436's menu reads the two switches: the table's own unless overridden, the sun's
        // when the day and night follow
        lighting: flag("Player", "OverrideTableEmissionScale").map(|over| if !over { 0 } else if flag("Player", "DynamicDayNight") == Some(true) { 2 } else { 1 }),
        light_level: share("Player", "EmissionScale", 1.0),
        altsound: flag("Plugin.AltSound", "Enable"),
        colour: flag("Plugin.Serum", "Enable").or_else(|| flag("Plugin.VNI", "Enable")),
        pup: flag("Plugin.PUP", "Enable"),
        screenshot: flag("VPI", "ScreenshotAtHighScore"),
        screenshot_screens: get("VPI", "ScreenshotScreens").map(|v| v.split([',', ' ', ';']).map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect()),
        screen_preset: get("VPI", "ScreenPreset"),
    }
}

/// What holds for the table where its own file says nothing: the user's usual, `main`, else the
/// table's own, `own`, else the player's defaults.
fn usual_of(main: &ForTable, own: Option<OwnLook>) -> Usual {
    // a table that couldn't be read: VPX's defaults for a new table
    let own = own.unwrap_or(OwnLook { tone_mapper: 0, exposure: 1.0, light_level: 1.0, difficulty: 0.2 });
    let yours = [("difficulty", main.difficulty.is_some()), ("toneMapper", main.tone_mapper.is_some()), ("exposure", main.exposure.is_some()), ("lighting", main.lighting.is_some())]
        .into_iter()
        .filter_map(|(name, set)| set.then_some(name))
        .collect();
    Usual {
        view: main.view.unwrap_or(0),
        playfield_volume: main.playfield_volume.unwrap_or(100),
        backglass_volume: main.backglass_volume.unwrap_or(100),
        difficulty: main.difficulty.unwrap_or(percent(own.difficulty)),
        tone_mapper: main.tone_mapper.unwrap_or(own.tone_mapper.min(4) as i32),
        exposure: main.exposure.unwrap_or(percent(own.exposure)),
        lighting: main.lighting.unwrap_or(0),
        light_level: main.light_level.unwrap_or(percent(own.light_level)),
        altsound: main.altsound.unwrap_or(true),
        colour: main.colour.unwrap_or(true),
        pup: main.pup.unwrap_or(true),
        // off unless turned on; every window showing
        screenshot: main.screenshot.unwrap_or(false),
        screenshot_screens: main.screenshot_screens.clone().unwrap_or_else(|| ["playfield", "backglass", "dmd", "topper"].map(String::from).to_vec()),
        yours,
    }
}

/// VPinMAME's default DMD colour, the orange of its own window.
const DMD_COLOUR: [i32; 3] = [255, 88, 32];
const DMD_KEYS: [&str; 3] = ["dmd_red", "dmd_green", "dmd_blue"];

fn colour_text(rgb: [i32; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0].clamp(0, 255), rgb[1].clamp(0, 255), rgb[2].clamp(0, 255))
}

fn colour_of(text: &str) -> Option<[i32; 3]> {
    let hex = text.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii())?;
    let part = |i: usize| i32::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([part(0)?, part(2)?, part(4)?])
}

/// A game's section of the machine settings, `section`; for its DMD colour, a part it leaves out
/// is `fallback`'s, as the player takes each part from the game's, else the default's.
fn machine_of(entries: &[(String, String, String)], section: &str, fallback: [i32; 3]) -> MachineChoices {
    let int = |key: &str| vpi_settings::get(entries, section, key).and_then(vpi_settings::vpx_int);
    let parts = DMD_KEYS.map(int);
    MachineChoices {
        sound: int("sound").map(|v| v != 0),
        volume: int("volume").map(|v| v.clamp(-32, 32)),
        samples: int("samples").map(|v| v != 0),
        cheat: int("cheat").map(|v| v != 0),
        real_dmd: int("showpindmd").map(|v| v != 0),
        dmd_colour: parts.iter().any(Option::is_some).then(|| colour_text([0, 1, 2].map(|i| parts[i].unwrap_or(fallback[i])))),
    }
}

/// What holds for a game where its section says nothing, as the player decides it
/// (`Controller::setting`, `Controller::run`): the default section's, else the plugin's
/// settings, else VPinMAME's defaults — but the samples off unless asked.
fn machine_usual(entries: &[(String, String, String)], main: &[(String, String, String)]) -> MachineUsual {
    let d = machine_of(entries, "default", DMD_COLOUR);
    let plugin = |key: &str| vpi_settings::get(main, "Plugin.PinMAME", key).and_then(vpi_settings::vpx_bool);
    MachineUsual {
        sound: d.sound.unwrap_or(true) && plugin("Sound").unwrap_or(true),
        volume: d.volume.unwrap_or(0),
        samples: d.samples.unwrap_or(false),
        cheat: d.cheat.unwrap_or_else(|| plugin("Cheat").unwrap_or(false)),
        // a display device is used unless the game's settings say 0 (`dmd_device.rs`)
        real_dmd: d.real_dmd.unwrap_or(true),
        dmd_colour: d.dmd_colour.unwrap_or_else(|| colour_text(DMD_COLOUR)),
    }
}

/// The default section's DMD colour, the parts a game's section leaves out.
fn default_colour(entries: &[(String, String, String)]) -> [i32; 3] {
    machine_of(entries, "default", DMD_COLOUR).dmd_colour.as_deref().and_then(colour_of).unwrap_or(DMD_COLOUR)
}

/// The table's settings, and the machine's for `game` (the library's word on it, for when its
/// script can't be run) when it drives PinMAME.
pub fn load(asked: &Asked, table: &Path, game: Option<String>) -> View {
    let file = file_of(table);
    let entries = vpi_settings::parse_ini(&read(&file));
    let main = vpi_settings::settings_path().map(|p| vpi_settings::parse_ini(&read(&p))).unwrap_or_default();
    let (size, modified) = stamp(table);
    let known = asked.0.lock().unwrap().get(table).filter(|(s, m, _)| *s == size && *m == modified).map(|(_, _, o)| o.clone());
    let (script, options_problem) = match known {
        Some(s) => (s, None),
        None => match ask_script(table) {
            Ok(s) => {
                asked.0.lock().unwrap().insert(table.to_path_buf(), (size, modified, s.clone()));
                (s, None)
            }
            Err(e) => (Script::default(), Some(e)),
        },
    };
    let Script { mut options, machine } = script;
    for o in &mut options {
        o.value = vpi_settings::get(&entries, "TableOption", &o.id).and_then(|v| stored(o, v));
    }
    let machine = machine.map(|m| (m.game, m.set)).or_else(|| game.map(|g| (g, BTreeMap::new()))).filter(|(g, _)| !g.trim().is_empty()).map(|(game, script_sets)| {
        let path = machine_file().unwrap_or_default();
        let m = vpi_settings::parse_ini(&read(&path));
        Machine {
            own: machine_of(&m, &game, default_colour(&m)),
            usual: machine_usual(&m, &main),
            dmd_device: vpi_settings::get(&main, "VPI", "DmdDevice").is_some_and(|v| !vpi_settings::vpx_trim(v).is_empty()),
            file: path.display().to_string(),
            game,
            script_sets,
        }
    });
    View {
        options,
        options_problem,
        for_table: for_table_of(&entries),
        usual: usual_of(&for_table_of(&main), ::library::table::own_look(table).ok()),
        machine,
        physics: physics_of(table).and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
        reach_outside: reach_outside_of(&main, table),
        reach_outside_usual: vpi_settings::get(&main, "VPI", "ScriptsReachOutside").and_then(vpi_settings::vpx_bool).unwrap_or(false),
        file: file.display().to_string(),
        exists: file.exists(),
    }
}

/// The section of the main settings file holding each table's own word on reaching outside its
/// files, by its file's name, as the player reads it (`fence::open_for`).
const OUTSIDE: &str = "ScriptsReachOutside";

fn reach_outside_of(main: &[(String, String, String)], table: &Path) -> Option<bool> {
    let name = table.file_name()?.to_string_lossy().into_owned();
    main.iter().rev().find(|(s, n, _)| s.eq_ignore_ascii_case(OUTSIDE) && n.eq_ignore_ascii_case(&name)).and_then(|(_, _, v)| vpi_settings::vpx_bool(v))
}

/// This table's own word on reaching outside its files saved in the main settings file — set, or
/// taken out for the usual — the file before kept, as every settings page keeps it.
pub fn save_reach_outside(table: &Path, value: Option<bool>) -> Result<(), String> {
    let name = table.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // a name the settings file can hold as a key
    if name.is_empty() || name.contains(['=', '[', ']', '\r', '\n']) || name.starts_with([';', '#']) {
        return Err("This table's file name can't be kept in the settings file; use the setting for every table instead.".into());
    }
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let before = read(&path);
    if reach_outside_of(&vpi_settings::parse_ini(&before), table) == value {
        return Ok(());
    }
    let change = match value {
        Some(v) => Change::set(OUTSIDE, &name, i32::from(v)),
        None => Change::remove(OUTSIDE, &name),
    };
    crate::screens::write_settings(&path, &before, &vpi_settings::edit(&before, &[change]))
}

/// An option's value as the file holds it: a number, or one of its names.
fn stored(o: &TableOption, text: &str) -> Option<f32> {
    let t = vpi_settings::vpx_trim(text);
    if let Some(names) = &o.names
        && let Some(k) = names.iter().position(|n| n == t)
    {
        return Some(o.min + k as f32);
    }
    vpi_settings::vpx_float(t).filter(|v| (o.min..=o.max).contains(v))
}

/// The changes saving `now` makes to a file holding `was`: each set or taken out where they differ.
fn table_changes(was: &ForTable, now: &ForTable) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut put = |section: &str, name: &str, before: Option<String>, after: Option<String>| {
        if before != after {
            changes.push(match after {
                Some(v) => Change::set(section, name, v),
                None => Change::remove(section, name),
            });
        }
    };
    let int = |v: Option<i32>, min: i32, max: i32| v.map(|v| v.clamp(min, max).to_string());
    // a share as VPX writes it, 0 to 1 (or 2)
    let share = |v: Option<i32>, max: i32| v.map(|v| format!("{:.6}", f64::from(v.clamp(0, max)) / 100.0));
    let flag = |v: Option<bool>| v.map(|b| i32::from(b).to_string());
    put("Player", "BGSet", int(was.view, 0, 2), int(now.view, 0, 2));
    put("Player", "SoundVolume", int(was.playfield_volume, 0, 100), int(now.playfield_volume, 0, 100));
    put("Player", "MusicVolume", int(was.backglass_volume, 0, 100), int(now.backglass_volume, 0, 100));
    put("TableOverride", "Difficulty", share(was.difficulty, 100), share(now.difficulty, 100));
    put("TableOverride", "ToneMapper", int(was.tone_mapper, 0, 4), int(now.tone_mapper, 0, 4));
    put("TableOverride", "Exposure", share(was.exposure, 200), share(now.exposure, 200));
    // the scene's light as VPX 5436's menu saves it: both switches, or neither
    let switches = |l: Option<i32>| (l.map(|l| i32::from(l != 0).to_string()), l.map(|l| i32::from(l == 2).to_string()));
    let (was_over, was_sun) = switches(was.lighting);
    let (now_over, now_sun) = switches(now.lighting);
    put("Player", "OverrideTableEmissionScale", was_over, now_over);
    put("Player", "DynamicDayNight", was_sun, now_sun);
    put("Player", "EmissionScale", share(was.light_level, 100), share(now.light_level, 100));
    put("Plugin.AltSound", "Enable", flag(was.altsound), flag(now.altsound));
    put("Plugin.Serum", "Enable", flag(was.colour), flag(now.colour));
    put("Plugin.VNI", "Enable", flag(was.colour), flag(now.colour));
    put("Plugin.PUP", "Enable", flag(was.pup), flag(now.pup));
    put("VPI", "ScreenshotAtHighScore", flag(was.screenshot), flag(now.screenshot));
    let screens = |v: &Option<Vec<String>>| v.as_ref().map(|l| l.join(","));
    put("VPI", "ScreenshotScreens", screens(&was.screenshot_screens), screens(&now.screenshot_screens));
    put("VPI", "ScreenPreset", was.screen_preset.clone(), now.screen_preset.clone().filter(|p| !p.contains(['\r', '\n'])));
    changes
}

/// The changes saving `now` for the game in `section` makes to a file holding `was`.
fn machine_changes(section: &str, was: &MachineChoices, now: &MachineChoices) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut put = |key: &str, before: Option<i32>, after: Option<i32>| {
        if before != after {
            changes.push(match after {
                Some(v) => Change::set(section, key, v),
                None => Change::remove(section, key),
            });
        }
    };
    let flag = |v: Option<bool>| v.map(i32::from);
    put("sound", flag(was.sound), flag(now.sound));
    put("volume", was.volume, now.volume.map(|v| v.clamp(-32, 32)));
    put("samples", flag(was.samples), flag(now.samples));
    put("cheat", flag(was.cheat), flag(now.cheat));
    put("showpindmd", flag(was.real_dmd), flag(now.real_dmd));
    let parts = |c: &Option<String>| c.as_deref().and_then(colour_of).map_or([None; 3], |rgb| rgb.map(Some));
    let (before, after) = (parts(&was.dmd_colour), parts(&now.dmd_colour));
    for (i, key) in DMD_KEYS.iter().enumerate() {
        put(key, before[i], after[i]);
    }
    changes
}

/// The table's settings saved: each option set or taken back to its default, this table's own
/// choices set or taken back to the usual — only what differs from the file; and the machine's
/// for its game, `machine`, the same way in the machine settings file.
pub fn save(table: &Path, options: &BTreeMap<String, Option<f32>>, for_table: &ForTable, machine: Option<(&str, &MachineChoices)>) -> Result<(), String> {
    let file = file_of(table);
    let before = read(&file);
    let entries = vpi_settings::parse_ini(&before);
    let mut changes = Vec::new();
    for (id, value) in options {
        // an option's name as build 5436 makes them: letters, digits and `_`
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let now = vpi_settings::get(&entries, "TableOption", id).map(|v| vpi_settings::vpx_trim(v).to_string());
        match value {
            Some(v) => {
                let text = if v.fract() == 0.0 { format!("{}", *v as i64) } else { format!("{v:.6}") };
                if now.as_deref().and_then(vpi_settings::vpx_float) != Some(*v) {
                    changes.push(Change::set("TableOption", id, text));
                }
            }
            None if now.is_some() => changes.push(Change::remove("TableOption", id)),
            None => {}
        }
    }
    changes.extend(table_changes(&for_table_of(&entries), for_table));
    if !changes.is_empty() {
        // a file made for the first time takes in the POV file's views, which it ends the reading of
        changes.splice(0..0, vpi_settings::first_table_settings(table));
        write(&file, &vpi_settings::edit(&before, &changes)).map_err(|e| format!("The table's settings couldn't be saved: {e}"))?;
    }
    if let Some((game, now)) = machine {
        // a game's name as PinMAME names its sets: no brackets to break the file's sections
        if game.is_empty() || game.contains(['[', ']', '\r', '\n']) {
            return Err("The machine's settings couldn't be saved: its game's name can't be a section's.".into());
        }
        let path = machine_file().ok_or("no APPDATA folder")?;
        let before = read(&path);
        let entries = vpi_settings::parse_ini(&before);
        let changes = machine_changes(game, &machine_of(&entries, game, default_colour(&entries)), now);
        if !changes.is_empty() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("The machine's settings couldn't be saved: {e}"))?;
            }
            write(&path, &vpi_settings::edit(&before, &changes)).map_err(|e| format!("The machine's settings couldn't be saved: {e}"))?;
        }
    }
    Ok(())
}

/// Whether the table's PinUP pack plays for it, and whether it does for every table (the main
/// settings' `[Plugin.PUP]` `Enable`), which its own file's word counts over.
pub fn pup_of(table: &Path) -> (bool, bool) {
    let main = vpi_settings::settings_path().map(|p| vpi_settings::parse_ini(&read(&p))).unwrap_or_default();
    let usual = for_table_of(&main).pup.unwrap_or(true);
    (for_table_of(&vpi_settings::parse_ini(&read(&file_of(table)))).pup.unwrap_or(usual), usual)
}

/// The table's PinUP pack on or off for it alone: set in its own file, or taken out of it where
/// that's every table's.
pub fn set_pup(table: &Path, on: bool) -> Result<(), String> {
    let (_, usual) = pup_of(table);
    let mut now = for_table_of(&vpi_settings::parse_ini(&read(&file_of(table))));
    now.pup = (on != usual).then_some(on);
    save(table, &BTreeMap::new(), &now, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(id: &str, names: Option<&[&str]>) -> TableOption {
        TableOption { id: id.into(), name: id.into(), min: 0.0, max: 2.0, step: 1.0, default: 1.0, unit: 0, names: names.map(|n| n.iter().map(|s| s.to_string()).collect()), value: None }
    }

    #[test]
    fn a_tables_own_settings_saved_where_they_changed() {
        let dir = std::env::temp_dir().join(format!("vpi-table-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("Some Table.vpx");
        std::fs::write(table.with_extension("ini"), "[TableOption]\nBall_Image = Old Arse\n; mine\n[Player]\nSoundVolume = 70\nOverrideTableEmissionScale = 1\nEmissionScale = 0.4\n").unwrap();
        let entries = vpi_settings::parse_ini(&read(&table.with_extension("ini")));
        let o = option("Ball_Image", Some(&["Bright", "Old Arse", "Old Arse Warm"]));
        assert_eq!(stored(&o, vpi_settings::get(&entries, "TableOption", "Ball_Image").unwrap()), Some(1.0));
        let was = for_table_of(&entries);
        assert_eq!((was.playfield_volume, was.lighting, was.light_level), (Some(70), Some(1), Some(40)));
        let options: BTreeMap<String, Option<f32>> = [("Ball_Image".to_string(), Some(2.0)), ("Dragon_Flames".to_string(), Some(0.0))].into();
        let mine = ForTable { playfield_volume: None, view: Some(1), difficulty: Some(50), tone_mapper: Some(1), exposure: Some(120), lighting: Some(2), light_level: None, pup: Some(false), ..was };
        save(&table, &options, &mine, None).unwrap();
        let text = read(&table.with_extension("ini"));
        assert!(text.contains("[Plugin.PUP]") && text.contains("Enable = 0"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.contains("Ball_Image = 2") && text.contains("Dragon_Flames = 0") && text.contains("; mine"), "{text}");
        assert!(text.contains("BGSet = 1") && text.contains("Difficulty = 0.500000") && !text.contains("SoundVolume"), "{text}");
        assert!(text.contains("ToneMapper = 1") && text.contains("Exposure = 1.200000"), "{text}");
        assert!(text.contains("OverrideTableEmissionScale = 1") && text.contains("DynamicDayNight = 1") && !text.contains("EmissionScale = 0.4"), "{text}");
        assert_eq!(for_table_of(&vpi_settings::parse_ini(&text)), mine);
    }

    #[test]
    fn the_usual_is_yours_else_the_tables_own() {
        let own = OwnLook { tone_mapper: 1, exposure: 0.8, light_level: 0.9, difficulty: 0.35 };
        let main = for_table_of(&vpi_settings::parse_ini("[TableOverride]\nExposure = 1.5\n[Player]\nBGSet = 2\n"));
        let u = usual_of(&main, Some(own));
        assert_eq!((u.view, u.exposure, u.tone_mapper, u.difficulty, u.light_level, u.lighting), (2, 150, 1, 35, 90, 0));
        assert_eq!(u.yours, vec!["exposure"]);
        assert!(u.altsound && u.colour);
    }

    #[test]
    fn a_games_machine_settings() {
        let text = "[default]\ndmd_red = 200\nsamples = 1\n\n[AFM_113B]\nsound = 0\nVolume = -6\ndmd_green = 10\n";
        let m = vpi_settings::parse_ini(text);
        let own = machine_of(&m, "afm_113b", default_colour(&m));
        assert_eq!((own.sound, own.volume, own.samples, own.cheat), (Some(false), Some(-6), None, None));
        // the parts the game leaves out the default section's
        assert_eq!(own.dmd_colour.as_deref(), Some("#c80a20"));
        let main = vpi_settings::parse_ini("[Plugin.PinMAME]\nCheat = 1\n");
        let usual = machine_usual(&m, &main);
        assert_eq!((usual.sound, usual.volume, usual.samples, usual.cheat, usual.real_dmd), (true, 0, true, true, true));
        assert_eq!(usual.dmd_colour, "#c85820");
        // saved: the sound back to the usual, samples off, a colour of its own
        let now = MachineChoices { sound: None, samples: Some(false), dmd_colour: Some("#00ff00".into()), ..own.clone() };
        let out = vpi_settings::edit(text, &machine_changes("afm_113b", &own, &now));
        let after = vpi_settings::parse_ini(&out);
        assert_eq!(machine_of(&after, "afm_113b", default_colour(&after)), now);
        assert!(!out.contains("sound = 0") && out.contains("Volume = -6") && out.contains("[default]\ndmd_red = 200"), "{out}");
        assert_eq!(colour_of("#c85820"), Some([200, 88, 32]));
        assert_eq!(colour_of("c85820"), None);
    }
}
