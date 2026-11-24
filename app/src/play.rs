//! PLAY: a table played in VPX — the library's, 10.8.1 or, where Settings › Library chooses it,
//! 10.8.0 beside it (`library::vpx::to_play`) — as a front end starts it (`-Play <table>`), which
//! quits when the game ends; never in our player, which the app keeps as a tool. VPX 10.8.0 takes
//! no add-ons, so a game there is counted for achievements once it's over, from what it left; its
//! own settings file is the one a game's changes are made on, the game controller 10.8.1's settings
//! map among them where its own map none. VPX's own settings play it, but where a screen preset is chosen (the table's
//! own, `[VPI]` `ScreenPreset`, or cabinet mode's) or its score display is recorded for colouring:
//! then a copy of VPX's settings with those over them, for that game alone (`-Ini`), and a
//! recording made by VPX's DMDUtil add-on in a folder of its own, VPX started there. A PinUP pack's
//! layout tried is put in the pack's own place for the game (`pup_packs::put_in_place`). A ROM
//! machine VPX's PinMAME has no memory of yet starts first unseen (`vpx_test::first_start`), so it
//! restores its factory settings then, not in the game; VPX is started, and waited for, on a thread
//! of its own. Once VPX has quit: the pack's own files put back, the game's scores read from its machine's memory where
//! VPX's PinMAME keeps it (or, for a table whose script keeps its own, a new best score), a
//! recording collected. A table's achievements are followed while it plays by our add-on in VPX
//! (`library::vpx_addon`, `engine/vpx-addon`), which hears the game from VPX's own PinMAME and B2S
//! add-ons and says how it went; where it heard nothing (the classic add-ons play the table), they're
//! counted from what the game left once VPX has quit (`achievements::count_after`).
//! The app is minimised once VPX's window shows (so the screen never stands empty between them) —
//! but for cabinet mode, which stays behind it — and back, its page focused, when the game ends.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use vpi_settings::Change;

use crate::achievements::Asked;
use crate::library::{self, Shared};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ended {
    id: String,
    name: String,
    /// VPX's exit code; None when it was stopped.
    code: Option<i32>,
    /// VPX's log, where it keeps one.
    log: Option<PathBuf>,
    /// The achievements the game earned, by name.
    earned: Vec<String>,
    /// Why VPX didn't play it, where it didn't start.
    problem: Option<String>,
    /// What VPX's log says of the game that the player can do something about.
    notes: Vec<crate::game_notes::GameNote>,
}

/// How a table's played.
enum Run {
    /// PLAY, as it stands or softcore.
    Play(Asked),
    /// A PinUP pack's layout tried: the file listing its files (`pup_packs::trying`).
    TryLayout(PathBuf),
    /// Its score display recorded for colouring, into this file.
    Record(PathBuf),
    /// An achievement pack being made, tried: the file naming it.
    TryPack(PathBuf),
}

/// PLAY: `asked`, as it stands, Play Hardcore — what makes it softcore set aside for the game
/// (`hardcore`) — or Play Softcore, its achievements kept as softcore whatever it plays as.
pub fn play(app: &AppHandle, id: &str, preset: Option<&str>, stay: bool, asked: Asked) -> Result<(), String> {
    start(app, id, preset, stay, Run::Play(asked))
}

/// The table played to try a layout of its PinUP pack, written down in `trying`: not counted as a
/// play.
pub fn try_layout(app: &AppHandle, id: &str, trying: PathBuf) -> Result<(), String> {
    start(app, id, None, false, Run::TryLayout(trying))
}

/// The table played with its score display's frames recorded for colouring it, into `dump`:
/// counted as a play.
pub fn play_recording(app: &AppHandle, id: &str, dump: PathBuf) -> Result<(), String> {
    start(app, id, None, false, Run::Record(dump))
}

/// The table played to try an achievement pack being made, the file naming it in `asked` in place
/// of its own packs': not counted as a play.
pub fn try_achievements(app: &AppHandle, id: &str, asked: PathBuf) -> Result<(), String> {
    start(app, id, None, false, Run::TryPack(asked))
}

/// The library's VPX to play a table in, as Settings › Library chooses (`vpx::to_play`): 10.8.0
/// or newer.
fn vpx_to_play(layout: Option<&::library::plan::Layout>) -> Result<::library::vpx::Vpx, String> {
    match layout.and_then(::library::vpx::to_play) {
        Some(v) if v.plays() => Ok(v),
        Some(v) => Err(format!(
            "Tables play in VPX 10.8.0 or newer, and the VPX found for this library is {}.{}.{} ({}). Settings › Library gets VPX 10.8.1.",
            v.version[0],
            v.version[1],
            v.version[2],
            v.program.parent().map(|d| d.display().to_string()).unwrap_or_default()
        )),
        None => Err("VPX wasn't found for this library. Settings › Library says where it is, or gets VPX 10.8.1.".into()),
    }
}

/// The library's VPX for what needs 10.8.1 whichever plays — its quiet capture mode, its DMDUtil
/// add-on's recording: the one whose settings the app keeps (`library::vpx::settings_vpx`), the one
/// that plays where it's 10.8.1 or newer, else the newest that is.
fn vpx_with_addons(layout: Option<&::library::plan::Layout>, for_what: &str) -> Result<::library::vpx::Vpx, String> {
    layout.and_then(::library::vpx::settings_vpx).ok_or_else(|| format!("{for_what} needs VPX 10.8.1, which wasn't found for this library: Settings › Library gets it."))
}

/// Whether the process `pid` shows a window yet: a visible top-level window of its own.
#[cfg(windows)]
fn shows_window(pid: u32) -> bool {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(each: unsafe extern "system" fn(*mut c_void, isize) -> i32, data: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
    }
    unsafe extern "system" fn each(hwnd: *mut c_void, data: isize) -> i32 {
        // SAFETY: `data` is the `(pid, found)` pair below, alive through EnumWindows
        let seek = unsafe { &mut *(data as *mut (u32, bool)) };
        let mut pid = 0;
        // SAFETY: a window Windows handed over
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == seek.0 && unsafe { IsWindowVisible(hwnd) } != 0 {
            seek.1 = true;
            return 0;
        }
        1
    }
    let mut seek = (pid, false);
    // SAFETY: the callback reads only the pair, which outlives the call
    unsafe { EnumWindows(each, &mut seek as *mut (u32, bool) as isize) };
    seek.1
}

#[cfg(not(windows))]
fn shows_window(_pid: u32) -> bool {
    true
}

/// Our add-on put in VPX's add-ons, or brought up to date, and switched on: whether it's there.
/// VPX 10.8.0 takes no add-ons.
fn addon_ready(vpx: &::library::vpx::Vpx) -> bool {
    if !vpx.has_addons() {
        return false;
    }
    if let Err(e) = ::library::vpx_addon::put_in_for(vpx, ::library::vpx::prefs_override().as_deref()) {
        eprintln!("{e}");
        return false;
    }
    let entries = vpi_settings::read_all();
    let section = format!("Plugin.{}", ::library::vpx_addon::ID);
    if !vpi_settings::get(&entries, &section, "Enable").and_then(vpi_settings::vpx_bool).unwrap_or(false)
        && let Err(e) = vpi_settings::save(&[Change::set(&section, "Enable", 1)])
    {
        eprintln!("the add-on couldn't be switched on: {e}");
        return false;
    }
    true
}

/// The keys a screenshot can be taken with while VPX plays, as `[VPI]` `ScreenshotKey` names them:
/// those VPX 10.8.1 leaves free, by Windows' key codes. F9 unless set.
pub const SCREENSHOT_KEYS: [(&str, u32); 8] = [("F1", 0x70), ("F2", 0x71), ("F4", 0x73), ("F5", 0x74), ("F6", 0x75), ("F7", 0x76), ("F8", 0x77), ("F9", 0x78)];

/// The key screenshots are taken with, as Windows codes it; None where they're off.
pub(crate) fn screenshot_key() -> Option<u32> {
    let entries = vpi_settings::read_all();
    match vpi_settings::get(&entries, "VPI", "ScreenshotKey").map(str::trim) {
        Some(k) if k.eq_ignore_ascii_case("off") => None,
        Some(k) => SCREENSHOT_KEYS.iter().find(|(name, _)| name.eq_ignore_ascii_case(k)).map(|(_, code)| *code).or(Some(0x78)),
        None => Some(0x78),
    }
}

/// Whether a screenshot is taken when a game here makes the machine's high scores: the table's own
/// settings (`<table>.ini`), else the main ones (`[VPI]` `ScreenshotAtHighScore`).
fn screenshot_at_high_score(table: &Path) -> bool {
    let own = std::fs::read(table.with_extension("ini")).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
    vpi_settings::get(&own, "VPI", "ScreenshotAtHighScore")
        .or(vpi_settings::get(&vpi_settings::read_all(), "VPI", "ScreenshotAtHighScore"))
        .and_then(vpi_settings::vpx_bool)
        .unwrap_or(false)
}

/// A ROM machine's memory map put where VPX's PinMAME add-on reads it: it shares a machine's game
/// state and high scores with our add-on only where it finds one. A VPX whose PinMAME add-on carries
/// the community's maps (from build 6625) reads its own where no `memmaps` folder is beside the
/// table or in PinMAME's folder: for a ROM its maps know, none of ours is put in, and a folder
/// holding only ours, put in for a VPX that carried none, goes to the Recycle Bin, as it would hide
/// VPX's own maps from every other machine.
fn put_memory_map(table: &Path, rom: &str, vpx: &::library::vpx::Vpx) {
    use ::library::vpx_addon::{maps_know, memory_maps_folder, only_ours, own_maps};
    let entries = vpi_settings::read_all();
    let pinmame = vpi_settings::get(&entries, "Plugin.PinMAME", "PinMAMEPath");
    if let Some(own) = own_maps(vpx)
        && maps_know(&own, rom)
    {
        if let Some(folder) = memory_maps_folder(table, pinmame).filter(|f| f.is_dir() && only_ours(f))
            && let Err(e) = ::library::install::to_recycle_bin(&folder)
        {
            eprintln!("our memory maps couldn't be taken out: {e}");
        }
        if memory_maps_folder(table, pinmame).is_none_or(|f| !f.is_dir()) {
            return;
        }
    }
    if let Err(e) = ::library::vpx_addon::put_memory_map(table, rom, pinmame) {
        eprintln!("the machine's memory map couldn't be put in: {e}");
    }
}

/// A table's achievements followed while VPX plays it, as far as VPX's own add-ons share its game
/// (our add-on in place, `addon_ready`, and the machine's memory map, `put_memory_map`): the file
/// handed to the game (`asked`) told where the add-on says how it went — that file, beside it,
/// returned.
fn follow_while_playing(asked: &Path, table: &Path) -> Option<PathBuf> {
    let result = asked.with_file_name("played.json");
    let _ = std::fs::remove_file(&result);
    let entries = vpi_settings::read_all();
    let mut v: serde_json::Value = std::fs::read(asked).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
    v["result"] = serde_json::Value::String(result.display().to_string());
    // the sound each one earned plays, as Settings › Sound has it
    let on = vpi_settings::get(&entries, "VPI", "AchievementSound").and_then(vpi_settings::vpx_bool).unwrap_or(true);
    let gain = vpi_settings::get(&entries, "VPI", "AchievementGain").and_then(vpi_settings::vpx_float).filter(|g| (0.0..=2.0).contains(g)).unwrap_or(1.0);
    v["sound"] = serde_json::json!({ "on": on, "gain": gain });
    v["popup"] = popup_asked(table, &entries);
    std::fs::write(asked, serde_json::to_vec_pretty(&v).ok()?).ok()?;
    Some(result)
}

/// The settings files a hardcore game plays with, put in the file handed to the add-on
/// (`hardcoreFiles`), for it to watch for what makes a game softcore being changed during it.
fn note_hardcore_files(asked: &Path, files: Vec<PathBuf>) {
    let Some(mut v) = std::fs::read(asked).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { return };
    v["hardcoreFiles"] = serde_json::Value::Array(files.iter().map(|f| serde_json::Value::String(f.display().to_string())).collect());
    if let Ok(bytes) = serde_json::to_vec_pretty(&v) {
        let _ = std::fs::write(asked, bytes);
    }
}

/// The card each achievement earned shows, as Settings › Screens has it: where and how big, and
/// VPX's priorities for that window's renderers, by which the add-on draws the window's own
/// picture under a card as VPX would.
fn popup_asked(table: &Path, main: &[(String, String, String)]) -> serde_json::Value {
    let (on, size) = crate::screens::popup_of(main);
    let Some(section) = popup_section(&on) else { return serde_json::json!({ "on": on, "size": size }) };
    let priorities: serde_json::Map<String, serde_json::Value> = renderer_priorities(table, main, section).into_iter().map(|(id, p)| (id, p.into())).collect();
    serde_json::json!({ "on": on, "size": size, "priorities": priorities })
}

/// The section of VPX's settings for the window the popup shows in; none for the playfield.
fn popup_section(on: &str) -> Option<&'static str> {
    match on {
        "backglass" => Some("Backglass"),
        "scoreview" => Some("ScoreView"),
        _ => None,
    }
}

/// VPX's priorities for a window's renderers, by their ids (`Priority.<id>` in the window's
/// section), as VPX reads them: the main settings', the table's own over them.
fn renderer_priorities(table: &Path, main: &[(String, String, String)], section: &str) -> Vec<(String, i64)> {
    let own = std::fs::read(table.with_extension("ini")).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
    let mut out: Vec<(String, i64)> = Vec::new();
    for entries in [main, &own[..]] {
        for (s, k, v) in entries {
            let (Some(id), true) = (k.strip_prefix("Priority."), s.eq_ignore_ascii_case(section)) else { continue };
            let Ok(p) = v.trim().parse::<i64>() else { continue };
            out.retain(|(i, _)| i != id);
            out.push((id.to_string(), p));
        }
    }
    out
}

/// What the add-on said of a game it followed, once VPX has quit: what was earned, by name; None
/// where it heard nothing of the game (or didn't run).
fn followed(result: Option<&Path>) -> Option<Vec<String>> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(result?).ok()?).ok()?;
    if v.get("followed").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    Some(v.get("earned").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// What VPX's settings are to be changed by for one game: the screens a preset gives — cabinet
/// mode's, else the one the table's own settings choose — and, recording for colouring, VPX's
/// DMDUtil add-on writing the display's frames down (its displays left unlooked for, where it
/// isn't on already). Presets hold VPX 10.8.1's windows, which 10.8.0 has none of: for 10.8.0, the
/// game controller 10.8.1's settings map, where its own settings map none
/// (`library::controller_1080`).
fn run_changes(vpx: &::library::vpx::Vpx, table: &Path, preset: Option<&str>, recording: bool) -> Vec<Change> {
    let main = vpi_settings::read_all();
    let mut changes = Vec::new();
    if !vpx.has_addons() {
        return ::library::controller_1080::changes(&main, &vpi_settings::parse_ini(&::library::vpx::settings_text(vpx)));
    }
    let preset_text = match preset.filter(|p| !p.trim().is_empty()) {
        Some(name) => vpi_settings::preset_path(name).and_then(|p| std::fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()),
        None => {
            let own = std::fs::read(table.with_extension("ini")).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
            vpi_settings::preset_named(&own, &main).map(|(_, text)| text)
        }
    };
    if let Some(text) = preset_text {
        changes.extend(vpi_settings::parse_ini(&text).into_iter().filter(|(s, _, _)| !vpi_settings::own_section(s)).map(|(s, k, v)| Change::set(&s, &k, v)));
    }
    // the achievement popup's renderer asked first in its window: it is with no priorities set,
    // as the add-on puts it first; where another renderer's is set, ours is set above it
    let (popup_on, _) = crate::screens::popup_of(&main);
    if let Some(section) = popup_section(&popup_on)
        && renderer_priorities(table, &main, section).iter().any(|(id, p)| *p > 0 && id != ::library::vpx_addon::ID)
    {
        changes.push(Change::set(section, &format!("Priority.{}", ::library::vpx_addon::ID), 100));
    }
    if recording {
        let on = vpi_settings::get(&main, "Plugin.DMDUtil", "Enable").and_then(vpi_settings::vpx_bool).unwrap_or(false);
        changes.push(Change::set("Plugin.DMDUtil", "Enable", 1));
        changes.push(Change::set("Plugin.DMDUtil", "DumpDMDTxt", 1));
        if !on {
            changes.push(Change::set("Plugin.DMDUtil", "FindDisplays", 0));
        }
    }
    changes
}

/// A folder of the app's own for one run of VPX, made empty.
fn run_folder(name: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("vpi-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// The frames VPX's DMDUtil add-on wrote down in `folder` (`<ROM set>-<time>.txt`, the colouring
/// editor's text: each frame its time as `0x` and eight hex digits, then a line for each row)
/// gzipped into `to`, how many it holds written beside it (`<to>.frames`). Whether there were any.
fn collect_recording(folder: &Path, to: &Path) -> Result<bool, String> {
    use std::io::Write as _;
    let Some(file) = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("txt")))
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
    else {
        return Ok(false);
    };
    let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let frames = text.split(|&b| b == b'\n').filter(|l| l.starts_with(b"0x")).count();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&text).map_err(|e| e.to_string())?;
    let packed = gz.finish().map_err(|e| e.to_string())?;
    std::fs::write(to, packed).map_err(|e| format!("{}: {e}", to.display()))?;
    std::fs::write(format!("{}.frames", to.display()), frames.to_string()).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(true)
}

/// What a table whose script keeps its own counts says of its games, as VPX keeps them: how many
/// it's played and its best score.
fn script_tally(layout: &::library::plan::Layout, table: &Path, saved_under: &[String]) -> (Option<u64>, Option<u64>, Option<u64>) {
    let s = ::library::scores::for_table(layout, Some(table), None, saved_under);
    let vpx = s.kept.iter().find(|k| k.by == "vpx");
    // the lowest score on its list: a game past it makes a new high score
    let lowest = vpx.and_then(|k| k.scores.iter().filter_map(|e| e.score).filter(|&s| s > 0).min());
    (vpx.and_then(|k| k.games_played), vpx.and_then(|k| k.scores.first()).and_then(|e| e.score), lowest)
}

/// What a game needs made before VPX starts: its own copy of VPX's settings, where it changes them;
/// the folder VPX starts in, recording; where the achievements add-on says how it went; the table's
/// own settings for it, played hardcore.
struct Prepared {
    ini: Option<PathBuf>,
    folder: Option<PathBuf>,
    result: Option<PathBuf>,
    table_ini: Option<PathBuf>,
}

fn start(app: &AppHandle, id: &str, preset: Option<&str>, stay: bool, run: Run) -> Result<(), String> {
    let shared = app.state::<Shared>();
    let counted = matches!(run, Run::Play(_) | Run::Record(_));
    // the ROM its script asks for not here, and another of its machine that is: that one played
    if let Some((from, to)) = crate::library::use_usable_rom(app, id) {
        let name = {
            let st = shared.0.lock().unwrap();
            st.tables.iter().find(|t| t.id == id).map(|t| library::view(&st, t).name).unwrap_or_default()
        };
        let _ = app.emit("rom-chosen", crate::library::RomChosen { name, from, to });
    }
    // the library's layout, with the VPX this table plays with: Settings › Library's, or the one
    // chosen for it
    let (layout, library_layout) = {
        let st = shared.0.lock().unwrap();
        let layout = match st.tables.iter().find(|t| t.id == id) {
            Some(t) => crate::library::layout_for(&st, &t.path),
            None => st.layout.clone(),
        };
        (layout, st.layout.clone())
    };
    // VPX's settings kept for the VPX the library plays, one put in VPX's folder meanwhile too
    if let Some(l) = &library_layout {
        ::library::vpx::follow(l);
    }
    // recording for colouring needs VPX 10.8.1's DMDUtil add-on, whichever VPX plays
    let vpx = if matches!(run, Run::Record(_)) { vpx_with_addons(layout.as_ref(), "Recording the display for coloring")? } else { vpx_to_play(layout.as_ref())? };
    let prefs = ::library::vpx::prefs_override();
    let (path, name, at, rom, saved_under, achievements, rom_only) = {
        let mut st = shared.0.lock().unwrap();
        if st.playing.is_some() {
            return Err("A table is playing already.".into());
        }
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        // its achievement packs, counted once VPX has quit; a pack being made in their place while
        // it's tried; none while a PinUP layout's only tried
        let achievements = match &run {
            Run::Play(asked) => crate::achievements::for_play(&st, t, *asked),
            Run::Record(_) => crate::achievements::for_play(&st, t, Asked::AsItStands),
            Run::TryPack(asked) => Some(asked.clone()),
            Run::TryLayout(_) => None,
        };
        let (path, name, rom, saved_under) = (t.path.clone(), t.stem.clone(), t.holds.rom_set.clone(), t.read.saved_under.clone());
        // Play Hardcore keeps a script beside it that only changes its ROM
        let rom_only = matches!(run, Run::Play(Asked::Hardcore)) && crate::achievements::only_its_rom_changed_in(&st, t);
        st.playing = Some(id.to_string());
        let at = if counted { library::record_play(&mut st, &path) } else { 0 };
        (path, name, at, rom, saved_under, achievements, rom_only)
    };
    let recording = match &run {
        Run::Record(dump) => Some(dump.clone()),
        _ => None,
    };
    let hardcore = matches!(run, Run::Play(Asked::Hardcore));
    let prepared = (|| -> Result<Prepared, String> {
        // the add-ons VPX plays with, decided the first time, and the folders they look in: VPX
        // 10.8.1's, as 10.8.0 takes none
        if vpx.has_addons() {
            crate::addons::decide_first()?;
            if let Some(l) = &layout
                && let Err(e) = crate::addons::set_folders(l)
            {
                eprintln!("VPX's add-ons' folders couldn't be set: {e}");
            }
        }
        // our add-on: its achievements followed while it plays, where VPX's add-ons share the game,
        // and screenshots on a key
        let addon = addon_ready(&vpx);
        // the score display over its backglass's picture, for a VPX that keeps it in the file
        if let Some(l) = &layout {
            crate::backglass_dmd::before_play(l, &vpx, &path, rom.as_deref());
        }
        let at_high = addon && screenshot_at_high_score(&path);
        if let Some(rom) = rom.as_deref().filter(|_| addon && (achievements.is_some() || at_high)) {
            put_memory_map(&path, rom, &vpx);
        }
        let result = achievements.as_deref().filter(|_| addon).and_then(|a| follow_while_playing(a, &path));
        // the settings the VPX that plays reads: 10.8.1's, kept by the settings pages, or 10.8.0's own
        let vpx_settings = ::library::vpx::settings_text(&vpx);
        // Play Hardcore: what makes it softcore set aside for the game
        let (table_ini, hardcore_changes) = if hardcore { crate::hardcore::set_aside(&path, rom.as_deref(), rom_only, &vpi_settings::parse_ini(&vpx_settings))? } else { (None, Vec::new()) };
        // VPX's settings for this game alone, where a preset, a recording or Play Hardcore changes them
        let mut changes = run_changes(&vpx, &path, preset, recording.is_some());
        changes.extend(hardcore_changes);
        let ini = if changes.is_empty() {
            None
        } else {
            let file = run_folder("vpx-settings")?.join("VPinballX.ini");
            std::fs::write(&file, vpi_settings::edit(&vpx_settings, &changes)).map_err(|e| format!("VPX's settings for this game couldn't be written: {e}"))?;
            Some(file)
        };
        // a hardcore game: the settings files VPX plays it with — the game's own copies, else VPX's
        // and the table's own — for the add-on to watch for what makes a game softcore being changed
        // in VPX's menu during it
        if hardcore && let Some(asked) = achievements.as_deref().filter(|_| addon) {
            let main = ini.clone().or_else(|| ::library::vpx::settings_file(&vpx));
            let table = table_ini.clone().unwrap_or_else(|| path.with_extension("ini"));
            note_hardcore_files(asked, main.into_iter().chain([table]).collect());
        }
        // a recording made where VPX starts, a folder of its own
        let folder = if recording.is_some() { Some(run_folder("vpx-recording")?) } else { None };
        if let Run::TryLayout(trying) = &run {
            crate::pup_packs::put_in_place(trying)?;
        }
        Ok(Prepared { ini, folder, result, table_ini })
    })();
    let Prepared { ini, folder, result, table_ini } = match prepared {
        Ok(p) => p,
        Err(e) => {
            if let Err(e) = crate::pup_packs::put_back() {
                eprintln!("{e}");
            }
            if let Err(e) = crate::hardcore::put_back() {
                eprintln!("{e}");
            }
            shared.0.lock().unwrap().playing = None;
            return Err(e);
        }
    };
    // what our player kept for it — a machine's memory, a script's saved scores — carried over
    // where VPX keeps none, so its high scores carry on
    if let Some(l) = &layout
        && let Err(e) = ::library::scores::carry_to_vpx(l, &path, rom.as_deref(), &saved_under)
    {
        eprintln!("what {name} keeps couldn't be carried over to VPX: {e}");
    }
    // a ROM machine VPX's PinMAME has no memory of yet: its first start, unseen, before the game
    let first_start = match (&rom, &layout) {
        (Some(rom), Some(l)) => !::library::vpx::machine_memory_folder(l, Some(&path)).join(format!("{}.nv", rom.to_ascii_lowercase())).is_file(),
        _ => false,
    };
    let _ = app.emit("library-changed", ());
    let app = app.clone();
    let id = id.to_string();
    std::thread::Builder::new()
        .name("vpx".into())
        .spawn(move || {
            // in 10.8.1's capture mode, whichever VPX plays: the machine's memory is where both keep it
            if let Some(quiet) = first_start.then(|| vpx_with_addons(layout.as_ref(), "A machine's first start").ok()).flatten() {
                let _ = app.emit("vpx-first-start", &name);
                if let Err(e) = ::library::vpx_test::first_start(&quiet, &path, &vpi_settings::vpx_settings_text()) {
                    eprintln!("{name}'s machine's first start: {e}");
                }
            }
            // what a script keeping its own counts says before the game, to tell what the game did
            let tally_before = match (&rom, &layout) {
                (None, Some(l)) if !saved_under.is_empty() => script_tally(l, &path, &saved_under),
                _ => (None, None, None),
            };
            let since = now_ms();
            // VPX 10.8.0: the shared scripts it was released with, beside the table for the game
            crate::scripts_1080::lend_for(&vpx, &path);
            let mut command = ::library::vpx::play_command(&vpx, &path, prefs.as_deref());
            if let Some(ini) = &ini {
                command.arg("-Ini").arg(ini);
            }
            if let Some(table_ini) = &table_ini {
                command.arg("-TableIni").arg(table_ini);
            }
            if let Some(folder) = &folder {
                command.current_dir(folder);
            }
            if let Some(asked) = &achievements {
                command.env("VPI_ACHIEVEMENTS", asked);
            }
            if let (Some(folder), Some(key)) = (crate::screenshots::folder(&name), screenshot_key()) {
                command.env("VPI_SCREENSHOTS", folder).env("VPI_SCREENSHOT_KEY", key.to_string());
                if screenshot_at_high_score(&path) {
                    command.env("VPI_SCREENSHOT_AT_HIGH_SCORE", "1");
                    // a script keeping its own: the lowest score on its list, which a game past
                    // makes a new high score, as its backglass tells the scores
                    if let Some(lowest) = tally_before.2 {
                        command.env("VPI_SCREENSHOT_HIGH_SCORE_FROM", lowest.to_string());
                    }
                }
            }
            // the app's own pause menu, where it's asked for: our add-on keeps VPX's menu key for it
            let played = crate::pause_menu::Played { settings: ini.clone(), table_settings: table_ini.clone().unwrap_or_else(|| path.with_extension("ini")), table: path.clone() };
            if vpx.has_addons()
                && let Some((port, levels)) = crate::pause_menu::start(&app, &id, played)
            {
                command.env("VPI_MENU_PORT", port.to_string()).env("VPI_MENU_LEVELS", levels);
            }
            // no script runs in VPX without Windows' VBScript, and VPX says nothing of it: said
            // here, VPX not started
            let started = if ::library::vpx::vbscript_ready() {
                command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("VPX didn't start: {e}"))
            } else {
                Err(::library::vpx::NO_VBSCRIPT.to_string())
            };
            let mut child = match started {
                Ok(c) => c,
                Err(problem) => {
                    crate::pause_menu::end(&app);
                    crate::scripts_1080::take_back();
                    if let Err(e) = crate::pup_packs::put_back() {
                        eprintln!("{e}");
                    }
                    if let Err(e) = crate::hardcore::put_back() {
                        eprintln!("{e}");
                    }
                    app.state::<Shared>().0.lock().unwrap().playing = None;
                    eprintln!("{name}: {problem}");
                    let _ = app.emit("player-ended", Ended { id, name, code: None, log: None, earned: Vec::new(), problem: Some(problem), notes: Vec::new() });
                    let _ = app.emit("library-changed", ());
                    return;
                }
            };
            // the app out of the way once VPX's window shows (cabinet mode staying behind it), told
            // to the screens; given up after a minute, or once the game's ended
            let ended = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            {
                let (app, ended, pid) = (app.clone(), ended.clone(), child.id());
                let _ = std::thread::Builder::new().name("vpx-shows".into()).spawn(move || {
                    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
                    while !shows_window(pid) && std::time::Instant::now() < until {
                        if ended.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    if ended.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let _ = app.emit("player-showing", ());
                    if !stay && let Some(w) = crate::main_window::window(&app) {
                        let _ = w.minimize();
                    }
                });
            }
            let code = child.wait().ok().and_then(|s| s.code());
            ended.store(true, std::sync::atomic::Ordering::Relaxed);
            crate::scripts_1080::take_back();
            crate::pause_menu::end(&app);
            if let Err(e) = crate::pup_packs::put_back() {
                eprintln!("{e}");
            }
            if hardcore && let Err(e) = crate::hardcore::put_back() {
                eprintln!("{e}");
            }
            if let (Some(folder), Some(dump)) = (&folder, &recording) {
                match collect_recording(folder, dump) {
                    Ok(true) => {}
                    Ok(false) => eprintln!("VPX's DMDUtil add-on wrote no frames for {name}"),
                    Err(e) => eprintln!("the recording of {name} couldn't be kept: {e}"),
                }
                let _ = std::fs::remove_dir_all(folder);
            }
            // what the game left: a machine's last game, from its memory where VPX's PinMAME keeps
            // it and its map knows where; else, a script keeping its own, its games and best score
            let mut left = crate::achievements::GameLeft::default();
            if let (Some(rom), Some(layout)) = (&rom, &layout) {
                let memory = ::library::vpx::machine_memory_folder(layout, Some(&path)).join(format!("{}.nv", rom.to_ascii_lowercase()));
                if let Some(scores) = ::library::scores::last_game_in(&memory, since) {
                    left = crate::achievements::GameLeft { games: 1, scores };
                }
            } else if let Some(layout) = layout.as_ref().filter(|_| !saved_under.is_empty()) {
                let (games, best, _) = script_tally(layout, &path, &saved_under);
                let played = games.zip(tally_before.0).map_or(0, |(now, was)| now.saturating_sub(was)) as u32;
                let new_best = best.filter(|b| tally_before.1.is_none_or(|was| *b > was));
                left = crate::achievements::GameLeft { games: played.max(u32::from(new_best.is_some())), scores: new_best.into_iter().collect() };
            }
            // what the add-on earned as it followed the game; else what the game left, counted now
            let earned = match (&achievements, followed(result.as_deref())) {
                (Some(_), Some(earned)) => earned,
                (Some(asked), None) if left.games > 0 => crate::achievements::count_after(asked, &left).unwrap_or_else(|e| {
                    eprintln!("achievements: {e}");
                    Vec::new()
                }),
                _ => Vec::new(),
            };
            {
                let shared = app.state::<Shared>();
                let mut st = shared.0.lock().unwrap();
                st.playing = None;
                if counted {
                    library::record_end(&mut st, &path, at, left.scores.clone());
                }
            }
            if let Some(w) = crate::main_window::window(&app) {
                let _ = w.unminimize();
            }
            crate::main_window::focus(&app);
            let log = ::library::vpx::log_file(&vpx, prefs.as_deref()).filter(|l| l.is_file());
            // what VPX's log says of the game that the player can do something about
            let mut notes = log
                .as_deref()
                .and_then(|l| std::fs::read(l).ok())
                .map(|b| crate::game_notes::from_log(&String::from_utf8_lossy(&b), crate::game_notes::supersampling_of(&path, &vpi_settings::read_all())))
                .unwrap_or_default();
            // a script stopped with an error in VPX 10.8.1: whether it set a ball's ID, and whether
            // VPX 10.8.0, which allowed that, is there to play the table with instead
            let older = vpx.has_addons() && layout.as_ref().is_some_and(|l| ::library::vpx::all_for(l).iter().any(|v| v.plays() && !v.has_addons()));
            crate::game_notes::fill_script(&mut notes, &path, older);
            // a screen named alone, as builds before 5515 named one: named again as this VPX names
            // screens, in every file VPX reads one from
            if notes.iter().any(|n| matches!(n, crate::game_notes::GameNote::Screen { .. }))
                && let Some(l) = &layout
                && crate::screens::fit_screen_names(Some(&l.tables)).unwrap_or(0) > 0
            {
                for n in &mut notes {
                    if let crate::game_notes::GameNote::Screen { asked, fixed, .. } = n
                        && vpi_settings::split_display_id(asked).is_none()
                    {
                        *fixed = true;
                    }
                }
            }
            // stopped by Windows as it loaded its files: why, said
            let problem = code.and_then(::library::vpx::stopped_loading);
            if let Some(p) = &problem {
                eprintln!("{name}: {p}");
            }
            let _ = app.emit("player-ended", Ended { id, name, code, log, earned, problem, notes });
            let _ = app.emit("library-changed", ());
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_popups_window_and_its_renderers_priorities_are_handed_over() {
        let dir = std::env::temp_dir().join(format!("vpi-popup-asked-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("A Table.vpx");
        // the table's own settings over the main ones; another window's and empty values left out
        std::fs::write(dir.join("A Table.ini"), "[Backglass]\nPriority.PUP = 5\n").unwrap();
        let main = vpi_settings::parse_ini("[VPI]\nAchievementPopupSize = small\n[Backglass]\nPriority.PUP = 1\nPriority.B2S = \nPriority.Nixie = 2\n[ScoreView]\nPriority.B2S = 9\n");
        let asked = popup_asked(&table, &main);
        assert_eq!(asked, serde_json::json!({ "on": "backglass", "size": "small", "priorities": { "Nixie": 2, "PUP": 5 } }));
        // the playfield: VPX's own notice, nothing more to say
        let pf = vpi_settings::parse_ini("[VPI]\nAchievementPopup = playfield\n");
        assert_eq!(popup_asked(&table, &pf), serde_json::json!({ "on": "playfield", "size": "medium" }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
