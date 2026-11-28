//! Future Pinball's tables played through the app: the install the library finds
//! (`library::future_pinball`: the folder the settings name, else beside VPX's, else its usual
//! folders), its tables in the library beside VPX's (`library::build`), matched to VPS among VPS's
//! Future Pinball table files, and PLAY — BAM's loader started as front ends start it, its screens
//! set as the app's Screens settings have VPX's and its keys as Settings › Controls maps VPX's
//! actions, put back after, a controller's buttons and triggers passed as its keys, its sound on
//! the outputs Settings › Sound sets, drawn at the screen's own pixels on a screen Windows scales,
//! fullscreen optimizations off where Settings › Library says so, the app minimised until
//! Future Pinball has ended, the play counted. Settings › Library says where it is, or where
//! to get it. Ours.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::library::Shared;

/// Future Pinball as the app finds it, for Settings › Library.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpState {
    /// Its folder, where it's found.
    pub folder: Option<String>,
    /// Whether BAM's loader is there, which plays its tables as front ends play them.
    pub bam: bool,
    /// How many tables its folder holds.
    pub tables: usize,
    /// Whether Windows' fullscreen optimizations are turned off for its games (`NO_OPTIMIZATIONS`).
    pub no_optimizations: bool,
}

/// The install, from the settings' folder for it (`[VPI]` `FuturePinballFolder`) and VPX's.
pub fn install() -> Option<::library::future_pinball::Install> {
    let main = vpi_settings::read_all();
    let named = vpi_settings::get(&main, "VPI", "FuturePinballFolder").map(str::trim).filter(|v| !v.is_empty()).map(PathBuf::from);
    let vpx = vpi_settings::library_folders(&main).and_then(|f| f.vpx.or_else(|| f.tables.parent().map(Path::to_path_buf)));
    if let Some(found) = ::library::future_pinball::find(named.as_deref(), vpx.as_deref()) {
        return Some(found);
    }
    // where its installer put it: named in the settings, so the library finds it too
    let found = registry::installed_folder().and_then(|f| ::library::future_pinball::Install::at(&f))?;
    if let Err(e) = vpi_settings::save(&[vpi_settings::Change::set("VPI", "FuturePinballFolder", found.folder.display())]) {
        eprintln!("Future Pinball's folder wasn't kept: {e}");
    }
    Some(found)
}

/// Future Pinball, where it's found.
#[tauri::command]
pub async fn fp_state() -> Result<FpState, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let no_optimizations = vpi_settings::get(&vpi_settings::read_all(), "VPI", NO_OPTIMIZATIONS).and_then(vpi_settings::vpx_bool) == Some(true);
        match install() {
            Some(install) => FpState { folder: Some(install.folder.display().to_string()), bam: install.loader.is_some(), tables: ::library::future_pinball::tables(&install).len(), no_optimizations },
            None => FpState { folder: None, bam: false, tables: 0, no_optimizations },
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Windows' fullscreen optimizations turned off for Future Pinball's games, or left on (Settings ›
/// Library): set for each game, put back after.
#[tauri::command]
pub fn fp_set_no_optimizations(on: bool) -> Result<(), String> {
    vpi_settings::save(&[if on { vpi_settings::Change::set("VPI", NO_OPTIMIZATIONS, 1) } else { vpi_settings::Change::remove("VPI", NO_OPTIMIZATIONS) }]).map_err(|e| e.to_string())
}

/// One of Future Pinball's tables played, as PLAY plays a VPX table: BAM's loader (else Future
/// Pinball) started with it, its screens set as the app's Screens settings have VPX's — PinUP's own
/// screens kept on Windows' for a table playing a pack — drawn at the screen's own pixels where
/// Windows scales it, its sound on the outputs Settings › Sound sets, the app minimised until it
/// has ended, then each put back, the play counted where `counted` and the library told
/// (`player-ended`, as a VPX game's end). Only a table of the install's own is started.
pub fn play_table(app: &AppHandle, id: Option<&str>, table: &Path, counted: bool) -> Result<(), String> {
    start_game(app, id, table, counted, false, false)
}

/// One of Future Pinball's tables played in a VR headset, through BAM's VR (`library::future_pinball
/// ::vr_loader`): its OpenComposite loader where the VR program VPX plays with is Meta's or Virtual
/// Desktop's (`vr::reaches_through_openxr`), else its SteamVR one — BAM's VR settings put first in a
/// VR folder without them, from TerryRed's set
/// (`library::future_pinball::put_vr_settings_in_place`); Future Pinball's picture as TerryRed's VR
/// start sets it, put back after; VPX's own menu's place taken by BAM's, which works in the headset.
pub fn play_table_vr(app: &AppHandle, id: Option<&str>, table: &Path) -> Result<(), String> {
    start_game(app, id, table, true, false, true)
}

/// One of its tables played as Settings › VR's switch has VPX's (`[PlayerVR]` `AskToTurnOn`), as
/// VPX's tables play: in the headset always; where the headset answers as the game starts — asked
/// first, as VPX asks, on a thread of its own so the app's window isn't held, a problem starting it
/// told as a game's end — else on the screens.
pub fn play_table_as_set(app: &AppHandle, id: Option<&str>, table: &Path) -> Result<(), String> {
    match crate::vr::switch_of(&vpi_settings::read_all()) {
        0 => play_table_vr(app, id, table),
        1 => {
            let folder = app.state::<Shared>().0.lock().unwrap().layout.as_ref().and_then(::library::vpx::to_play).and_then(|v| v.program.parent().map(Path::to_path_buf));
            let Some(folder) = folder else { return play_table(app, id, table, true) };
            let (app, id, table) = (app.clone(), id.map(str::to_string), table.to_path_buf());
            std::thread::Builder::new()
                .name("future-pinball-headset".into())
                .spawn(move || {
                    let ready = crate::vr::check_in(&folder).ready;
                    eprintln!("Future Pinball's table {}: {}", table.display(), if ready { "the headset answered, played in VR" } else { "no headset answered, played on the screens" });
                    let started = if ready { play_table_vr(&app, id.as_deref(), &table) } else { play_table(&app, id.as_deref(), &table, true) };
                    if let Err(problem) = started {
                        let name = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        let _ = app.emit("player-ended", serde_json::json!({ "id": id.unwrap_or_default(), "name": name, "code": serde_json::Value::Null, "earned": [], "notes": [], "problem": problem }));
                    }
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        _ => play_table(app, id, table, true),
    }
}

/// A game started: played, or `recording` its media — then no pause menu and no controller, the
/// game left to its attract mode — or played in VR (`vr`).
fn start_game(app: &AppHandle, id: Option<&str>, table: &Path, counted: bool, recording: bool, vr: bool) -> Result<(), String> {
    let install = install().ok_or("Future Pinball isn't found on this PC.")?;
    let vr_loader = if vr {
        // BAM's VR settings put where a VR folder has none, as TerryRed's steps put them
        let set_up = ::library::future_pinball::put_vr_settings_in_place(&install);
        if !set_up.is_empty() {
            eprintln!("BAM's VR settings put in {}, from TerryRed's set", set_up.join(" and "));
        }
        Some(::library::future_pinball::vr_loader(&install, crate::vr::reaches_through_openxr())?)
    } else {
        None
    };
    if !::library::future_pinball::tables(&install).iter().any(|t| t == table) {
        return Err("That isn't one of Future Pinball's tables.".into());
    }
    let name = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    {
        let shared = app.state::<Shared>();
        let mut st = shared.0.lock().unwrap();
        if st.playing.is_some() {
            return Err("A table is playing already.".into());
        }
        st.playing = Some(id.unwrap_or_default().to_string());
    }
    // the app's own pause menu over it, where it's asked for
    let app_menu = !recording && !vr && crate::pause_menu::ours(app);
    // the playfield in its window mode without the window's border, over the whole of its screen,
    // where it fills one and the app's menu may show over it: Future Pinball's full screen covers
    // every other window, and gives way whenever it loses the focus
    let borderless = if app_menu { playfield_screen().filter(|&(.., full)| full).map(|(x, y, w, h, _)| (x, y, w, h)) } else { None };
    // its screens as the app's Screens settings have VPX's and its keys as Settings › Controls has
    // them, put back once it has ended
    let pup_pack = pup_pack_for(&name);
    set_screens(pup_pack, vr, borderless.is_some());
    if pup_pack {
        crate::pinup_screens::before_play();
    }
    set_compatibility(&install, on_scaled_screen(pup_pack));
    let at = crate::library::now_ms();
    let command = match &vr_loader {
        Some(loader) => ::library::future_pinball::play_command_vr(loader, table),
        None => ::library::future_pinball::play_command(&install, table),
    };
    let mut command = command;
    // through OpenComposite, the VR program chosen in Settings › VR, as VPX plays with it: its
    // runtime for 32-bit programs, as Future Pinball is (Windows' own where none is chosen)
    if let Some(loader) = &vr_loader
        && ::library::future_pinball::through_open_composite(loader)
        && let Some(manifest) = crate::vr::chosen_32()
    {
        command.env("XR_RUNTIME_JSON", manifest);
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            put_screens_back();
            put_scaling_back();
            app.state::<Shared>().0.lock().unwrap().playing = None;
            return Err(format!("Future Pinball didn't start: {e}"));
        }
    };
    // its sound on the outputs Settings › Sound sets, as each of its programs starts playing
    let ended = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let ended = ended.clone();
        let _ = std::thread::Builder::new().name("future-pinball-sound".into()).spawn(move || crate::sound::route_future_pinball(&ended));
    }
    // its window's border taken off and the window fitted to its screen as it shows
    if let Some(place) = borderless {
        let ended = ended.clone();
        let _ = std::thread::Builder::new().name("future-pinball-borderless".into()).spawn(move || keep_borderless(&ended, place));
    }
    // the app's own pause menu over it: the menu's key and button watched while its window is in
    // front
    if app_menu {
        crate::pause_menu::start_future_pinball(app, id.unwrap_or_default(), table);
    }
    // the controller Settings › Controls maps, its buttons and triggers passed to it as its keys
    // (as set for the game), whenever it's on; its menu button left to the app's own menu
    let key = |name: &str| match registry::read(name) {
        Some(::library::future_pinball::Value::Number(n)) => Some(n),
        _ => None,
    };
    if !recording
        && let Some(keys) = ::library::controller_1080::keys_with(&vpi_settings::read_all(), |action| if app_menu && action == "InGameUI" { None } else { ::library::future_pinball::key_for(action, key) })
    {
        crate::pad_keys::start(ended.clone(), keys, keys_in_front);
    }
    // its window brought to the front once its table has loaded, so the keyboard and the controller
    // reach it from the start
    if !recording {
        let ended = ended.clone();
        let _ = std::thread::Builder::new().name("future-pinball-front".into()).spawn(move || crate::pause_menu::keep_future_pinball_in_front(&ended));
    }
    // the screens' modes written down as it plays: its full screen changes its screen's mode
    {
        let ended = ended.clone();
        let _ = std::thread::Builder::new().name("future-pinball-modes".into()).spawn(move || watch_screen_modes(&ended));
    }
    // ended by the app where it stops answering once asked to end (asked while it still loads its
    // table, it stops for good)
    if !recording {
        let (ended, loader) = (ended.clone(), child.id());
        let _ = std::thread::Builder::new().name("future-pinball-ending".into()).spawn(move || watch_ending(&ended, loader));
    }
    let _ = app.emit("player-showing", ());
    if let Some(w) = crate::main_window::window(app) {
        let _ = w.minimize();
    }
    let (app, table, id) = (app.clone(), table.to_path_buf(), id.map(str::to_string));
    std::thread::Builder::new()
        .name("future-pinball".into())
        .spawn(move || {
            let _ = child.wait();
            ended.store(true, std::sync::atomic::Ordering::Relaxed);
            crate::pause_menu::end(&app);
            put_screens_back();
            put_scaling_back();
            {
                let shared = app.state::<Shared>();
                let mut st = shared.0.lock().unwrap();
                st.playing = None;
                if counted {
                    crate::library::record_end(&mut st, &table, at, Vec::new());
                }
            }
            if let Some(w) = crate::main_window::window(&app) {
                let _ = w.unminimize();
            }
            crate::main_window::focus(&app);
            let _ = app.emit("player-ended", serde_json::json!({ "id": id.unwrap_or_default(), "name": name, "code": serde_json::Value::Null, "earned": [], "notes": [] }));
            let _ = app.emit("library-changed", ());
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// How long a table is left to its attract mode once it plays, before it's recorded.
const ATTRACT: std::time::Duration = std::time::Duration::from_secs(12);

/// How long a table may take to load before it plays (a few take more than a minute).
const LOADING: std::time::Duration = std::time::Duration::from_secs(180);

/// The programs whose windows a recording may capture: Future Pinball's, and PinUP's that show a
/// PinEvent table's pack. A part of a screen where another program's window shows (a window over
/// the backglass, a screen PinUP hasn't drawn on yet) is left out.
const RECORDED_PROGRAMS: &str = "Future Pinball.exe;PinUpDisplay.exe;PinUpDisplayX.exe;PinUpDisplayX2.exe;PinUpPlayer.exe";

/// How long its playfield is recorded.
const RECORDED_SECONDS: u32 = 20;

/// A Future Pinball table's media recorded from Future Pinball itself, as front ends' media
/// recorders make them (our player draws VPX's tables): the table started on the user's screens as
/// PLAY starts it, not counted, left to its attract mode (`ATTRACT`), each screen it shows captured
/// by the player as it shows (`vpi-player --capture`) — the playfield's picture and video, the
/// backglass's picture (its backbox, or a PinEvent table's PinUP backglass), the score display's
/// where PinUP shows one — into the pictures the app makes of a table (`own_pictures`), then the
/// game ended (its window closed, as PinUP Popper ends it, else its program). In the background,
/// the tasks bar told, the screens told once it's done (`media-recorded`).
pub fn record_media(app: &AppHandle, id: &str, table: &Path) -> Result<(), String> {
    let name = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let pup_pack = pup_pack_for(&name);
    start_game(app, Some(id), table, false, true, false)?;
    let (app, id) = (app.clone(), id.to_string());
    std::thread::Builder::new()
        .name("future-pinball-record".into())
        .spawn(move || {
            let task = crate::tasks::begin(&app, format!("Recording {name}'s media"), 0);
            let made = record_playing(&id, pup_pack);
            end_game();
            drop(task);
            let problem = made.as_ref().err().cloned();
            match &made {
                Ok(kinds) => eprintln!("{name}'s media recorded: {}", kinds.join(", ")),
                Err(e) => eprintln!("{name}'s media weren't recorded: {e}"),
            }
            crate::library::forget_popper_copies(&app, &id, &made.unwrap_or_default());
            let _ = app.emit("media-recorded", serde_json::json!({ "id": id, "name": name, "problem": problem }));
            let _ = app.emit("library-changed", ());
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The game's screens recorded once its table plays and its attract mode has run: the kinds of
/// media made.
fn record_playing(id: &str, pup_pack: bool) -> Result<Vec<String>, String> {
    // its playfield's window, once it shows: filling most of a screen (its editor shows first, then
    // its loading screen in the playfield's place while the table loads)
    let monitors = crate::b2s_screen::screens();
    let fills_a_screen = |&(_, x, y, w, h): &(isize, i32, i32, i32, i32)| {
        monitors.iter().any(|((mx, my), _, (mw, mh))| x >= *mx && y >= *my && i64::from(w) * i64::from(h) * 2 >= i64::from(*mw) * i64::from(*mh) && x < mx + *mw as i32 && y < my + *mh as i32)
    };
    let started = std::time::Instant::now();
    let (px, py, pw, ph) = loop {
        if let Some(&(_, x, y, w, h)) = crate::pause_menu::future_pinball_windows().iter().find(|w| fills_a_screen(w)) {
            break (x, y, w, h);
        }
        if started.elapsed() > LOADING {
            return Err("Future Pinball's playfield never showed".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    // then its table playing, not its loading screen: told by the player from the screen itself
    let pid = ::library::processes::running_with_ids().into_iter().find(|(_, n)| n.eq_ignore_ascii_case(::library::future_pinball::PROGRAM)).map(|(p, _)| p).ok_or("Future Pinball closed before it was recorded")?;
    let left = LOADING.saturating_sub(started.elapsed()).as_secs().max(30);
    let waited = run_player(&["--wait-playing".into(), format!("{px},{py},{pw},{ph}"), "--process".into(), pid.to_string(), "--seconds".into(), left.to_string()])?;
    eprintln!("Future Pinball's table recorded once it played: {}", waited.trim());
    std::thread::sleep(ATTRACT);
    // where each screen shows now: the playfield its largest window; the backglass a PinEvent
    // table's PinUP backglass (its screen 2), else its backbox, a window of its own ("Backbox
    // Display"); the score display a PinEvent table's PinUP DMD (its screen 1)
    let windows = crate::pause_menu::future_pinball_windows();
    let Some(&(_, px, py, pw, ph)) = windows.first() else { return Err("Future Pinball closed before it was recorded".into()) };
    let mut parts = vec![format!("playfield:{px},{py},{pw},{ph}")];
    let on_a_screen = |x: i32, y: i32, w: u32, h: u32| monitors.iter().any(|((mx, my), _, (mw, mh))| x >= *mx && y >= *my && x + w as i32 <= mx + *mw as i32 && y + h as i32 <= my + *mh as i32);
    let pinup = if pup_pack {
        vpi_settings::library_folders_now()
            .map(|f| ::library::plan::Layout::from_folders(&f))
            .and_then(|l| std::fs::read(l.pup_videos.parent()?.join("PinUpPlayer.ini")).ok())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    } else {
        None
    };
    let pinup_screen = |n: u32| pinup.as_deref().and_then(|t| ::library::pinup_screens::screen_place(t, n)).filter(|&(x, y, w, h)| on_a_screen(x, y, w, h));
    let backbox = windows.iter().skip(1).find(|w| crate::pause_menu::window_title(w.0).to_lowercase().contains("backbox"));
    let backglass = pinup_screen(2).or_else(|| backbox.map(|&(_, x, y, w, h)| (x, y, w as u32, h as u32)).filter(|&(x, y, w, h)| on_a_screen(x, y, w, h)));
    if let Some((x, y, w, h)) = backglass {
        parts.push(format!("playfield-backglass:{x},{y},{w},{h}"));
    }
    if let Some((x, y, w, h)) = pinup_screen(1) {
        parts.push(format!("playfield-dmd:{x},{y},{w},{h}"));
    }
    // into a folder of its own first, then in place, so a recording that fails leaves the media
    // made before as they were; only what the game's own programs show
    let scratch = std::env::temp_dir().join(format!("vpi-recording-{id}"));
    let _ = std::fs::remove_dir_all(&scratch);
    let result = run_player(&[
        "--capture".into(),
        parts.join(";"),
        "--video".into(),
        "playfield".into(),
        "--seconds".into(),
        RECORDED_SECONDS.to_string(),
        "--programs".into(),
        RECORDED_PROGRAMS.into(),
        "--out".into(),
        scratch.display().to_string(),
    ]);
    let said = match result {
        Ok(said) => said,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&scratch);
            return Err(e);
        }
    };
    for line in said.lines().filter(|l| l.starts_with("left out:")) {
        eprintln!("Future Pinball's recording: {line}");
    }
    let folder = crate::own_pictures::folder(id).ok_or("no APPDATA folder")?;
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut kinds = Vec::new();
    for (kind, file) in crate::own_pictures::KINDS {
        let made = scratch.join(file);
        if made.is_file() {
            let to = folder.join(file);
            if std::fs::copy(&made, &to).is_ok() {
                kinds.push(kind.to_string());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(kinds)
}

/// The player run with these arguments, no console window showing: what it printed, or what it said
/// went wrong.
fn run_player(args: &[String]) -> Result<String, String> {
    let mut command = std::process::Command::new(crate::screens::player()?);
    command.args(args).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // a console program: no console window shows
        command.creation_flags(0x0800_0000);
    }
    let out = command.output().map_err(|e| format!("the player didn't start: {e}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if said.is_empty() { "the player couldn't record the screens".into() } else { said });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The game recorded ended as PinUP Popper ends Future Pinball: its windows asked to close, then
/// its program closed if it's still there a few seconds later.
fn end_game() {
    for (hwnd, ..) in crate::pause_menu::future_pinball_windows() {
        crate::pause_menu::close_window(hwnd);
    }
    for _ in 0..12 {
        if crate::pause_menu::future_pinball_windows().is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    // still there: its program closed
    close_program();
}

/// Future Pinball's program closed by Windows (`taskkill /F`), every process of it.
fn close_program() {
    for (pid, program) in ::library::processes::running_with_ids() {
        if program.eq_ignore_ascii_case(::library::future_pinball::PROGRAM) {
            kill(pid);
        }
    }
}

/// A process closed by Windows (`taskkill /F`).
fn kill(pid: u32) {
    let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

/// When the player last asked the game playing to end (milliseconds since 1970; 0, not since it
/// began): Esc pressed while Future Pinball's window is in front, or passed from the controller as
/// Esc, or the pause menu's Quit.
static ASKED_TO_END: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The player asked the game to end now (the pause menu's Quit).
pub(crate) fn asked_to_end() {
    ASKED_TO_END.store(crate::library::now_ms(), std::sync::atomic::Ordering::Relaxed);
}

/// How long Future Pinball's windows may stop answering, once it's been asked to end, before its
/// program is closed: it answers again within a moment once it's finished a step of its own.
const STOPPED_FOR: std::time::Duration = std::time::Duration::from_secs(5);

/// Future Pinball watched once asked to end, until `ended`: asked while it still loads its table, it
/// stops answering for good, its window frozen on its loading screen, and the app waited with it.
/// Where it has stopped answering a few seconds after it was asked — no window of its program's
/// answers Windows for `STOPPED_FOR`, or its program is still there a quarter of a minute on with
/// none shown — its program is closed by Windows, then BAM's loader that started it (`loader`) if
/// that's still there. Asked to end and still playing half a minute on, it's taken as not asked (Esc
/// means something else there).
fn watch_ending(ended: &std::sync::atomic::AtomicBool, loader: u32) {
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    const VK_ESCAPE: u32 = 0x1B;
    ASKED_TO_END.store(0, Ordering::Relaxed);
    let mut escape_was_down = false;
    let mut stopped_since: Option<Instant> = None;
    while !ended.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(100));
        // Esc pressed for the game: the keyboard's own, or the controller's passed as it (the pause
        // menu keeps the keyboard's from the game while it shows)
        let escape = crate::pause_menu::key_down(VK_ESCAPE) && in_front() && !crate::pause_menu::menu_over_game();
        if escape && !escape_was_down {
            asked_to_end();
        }
        escape_was_down = escape;
        let asked = ASKED_TO_END.load(Ordering::Relaxed);
        if asked == 0 {
            stopped_since = None;
            continue;
        }
        let since = crate::library::now_ms().saturating_sub(asked);
        if since > 30_000 {
            ASKED_TO_END.store(0, Ordering::Relaxed);
            continue;
        }
        if since < 2_000 {
            continue;
        }
        // its windows gone, its program still there: closing, or Windows' stand-in shown for a
        // window that's stopped — given longer
        let windows = crate::pause_menu::future_pinball_windows();
        let stopped = if windows.is_empty() { since > 15_000 } else { !windows.iter().any(|&(hwnd, ..)| crate::pause_menu::window_answers(hwnd)) };
        if !stopped {
            stopped_since = None;
            continue;
        }
        if !windows.is_empty() && stopped_since.get_or_insert_with(Instant::now).elapsed() < STOPPED_FOR {
            continue;
        }
        eprintln!("Future Pinball stopped answering once it was asked to end: its program closed by the app");
        close_program();
        let until = Instant::now() + Duration::from_secs(3);
        while !ended.load(Ordering::Relaxed) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        if !ended.load(Ordering::Relaxed) {
            kill(loader);
        }
        ASKED_TO_END.store(0, Ordering::Relaxed);
        stopped_since = None;
    }
}

/// The screens' modes written to the app's log while a game plays in Future Pinball, until `ended`:
/// as it starts, and each change. Future Pinball makes its full screen by changing its screen's mode
/// to its size and colours, naming no refresh rate, so the rate it plays at is Windows' pick.
fn watch_screen_modes(ended: &std::sync::atomic::AtomicBool) {
    let said = |modes: &[(u32, u32, u32, u32)]| modes.iter().map(|(n, w, h, hz)| format!("screen {n} {w} x {h} at {hz} Hz")).collect::<Vec<_>>().join(", ");
    let mut last = screen_modes();
    eprintln!("The screens' modes as Future Pinball starts: {}", said(&last));
    while !ended.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let now = screen_modes();
        if now != last {
            eprintln!("The screens' modes while Future Pinball plays: {}", said(&now));
            last = now;
        }
    }
}

/// Each screen's mode as Windows has it now: its number (`\\.\DISPLAY<n>`), size and refresh rate.
#[cfg(windows)]
fn screen_modes() -> Vec<(u32, u32, u32, u32)> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplaySettingsW(device: *const u16, mode: u32, devmode: *mut u8) -> i32;
    }
    const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
    // DEVMODEW: its size at 68, its width, height and refresh rate at 172, 176 and 184
    const SIZE: usize = 220;
    crate::b2s_screen::screens()
        .into_iter()
        .filter_map(|(_, n, _)| {
            let device: Vec<u16> = format!(r"\\.\DISPLAY{n}").encode_utf16().chain(std::iter::once(0)).collect();
            let mut mode = [0u8; SIZE];
            mode[68..70].copy_from_slice(&(SIZE as u16).to_le_bytes());
            // SAFETY: a NUL-ended device name, a DEVMODEW of its size to fill
            if unsafe { EnumDisplaySettingsW(device.as_ptr(), ENUM_CURRENT_SETTINGS, mode.as_mut_ptr()) } == 0 {
                return None;
            }
            let at = |o: usize| u32::from_le_bytes([mode[o], mode[o + 1], mode[o + 2], mode[o + 3]]);
            Some((n, at(172), at(176), at(184)))
        })
        .collect()
}

#[cfg(not(windows))]
fn screen_modes() -> Vec<(u32, u32, u32, u32)> {
    Vec::new()
}

/// Whether Future Pinball's window is in front for the controller's buttons passed as its keys: not
/// while the app's own menu shows over it, whose they are then.
fn keys_in_front() -> bool {
    !crate::pause_menu::menu_over_game() && in_front()
}

/// Whether a table plays a PinUP pack (PinEvent, its backglass and display): named so, or a pack in
/// PinUP's folder named as it.
fn pup_pack_for(name: &str) -> bool {
    if name.to_lowercase().contains("pinevent") {
        return true;
    }
    let layout = vpi_settings::library_folders_now().map(|f| ::library::plan::Layout::from_folders(&f));
    layout.is_some_and(|l| l.pup_videos.join(name).join("screens.pup").is_file())
}

/// The record of Future Pinball's settings as they were before the app set them for a game:
/// `Future Pinball\settings before.json` in the app's data folder.
fn record() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Future Pinball").join("settings before.json"))
}

/// Future Pinball's screens set as the app's Screens settings have VPX's windows, and its keys as
/// Settings › Controls maps VPX's actions (`wanted`) — in VR, its picture as TerryRed's VR start sets
/// it over them; `borderless`, its window mode at the playfield screen's size in place of its full
/// screen, the window made borderless as it shows (`keep_borderless`); what was there kept first, to
/// put back.
fn set_screens(pup_pack: bool, vr: bool, borderless: bool) {
    put_screens_back();
    let mut settings = wanted(pup_pack);
    if borderless && let Some(full) = settings.iter_mut().find(|(n, _)| *n == "FullScreen") {
        full.1 = ::library::future_pinball::Value::Number(0);
    }
    if vr {
        for &(name, value) in ::library::future_pinball::VR_SETTINGS {
            let value = ::library::future_pinball::Value::Number(value);
            match settings.iter_mut().find(|(n, _)| *n == name) {
                Some(s) => s.1 = value,
                None => settings.push((name, value)),
            }
        }
    }
    let before: Vec<(String, Option<::library::future_pinball::Value>)> = settings.iter().map(|(n, _)| (n.to_string(), registry::read(n))).collect();
    let Some(path) = record() else { return };
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
    if std::fs::write(&path, serde_json::to_vec(&before).unwrap_or_default()).is_err() {
        return;
    }
    for (name, value) in &settings {
        if let Err(e) = registry::write(name, value) {
            eprintln!("Future Pinball's {name} wasn't set: {e}");
        }
    }
    eprintln!("Future Pinball's screens and keys set as VPX's: {settings:?}");
}

/// What Future Pinball's settings are set to for a game: the playfield's screen and size and the
/// backglass's as its backbox, as the app's Screens settings have VPX's windows
/// (`library::future_pinball::screen_settings`), each screen found by its place; its keys as VPX's
/// settings map VPX's actions (`library::future_pinball::key_settings`).
fn wanted(pup_pack: bool) -> Vec<(&'static str, ::library::future_pinball::Value)> {
    let mut settings = screens_wanted(pup_pack);
    settings.extend(::library::future_pinball::key_settings(&vpi_settings::read_all()));
    settings
}

/// Its screens, as `wanted` sets them.
fn screens_wanted(pup_pack: bool) -> Vec<(&'static str, ::library::future_pinball::Value)> {
    let main = vpi_settings::read_all();
    let get = |s: &str, k: &str| vpi_settings::get(&main, s, k).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let number = |s: &str, k: &str| get(s, k).and_then(|v| v.parse::<i64>().ok());
    // Windows' screens, by their names Future Pinball takes (`\\.\DISPLAY2`), places and sizes
    let monitors: Vec<(String, i32, i32, u32, u32)> = crate::b2s_screen::screens().into_iter().map(|((x, y), n, (w, h))| (format!(r"\\.\DISPLAY{n}"), x, y, w, h)).collect();
    let window = |display: Option<String>, width: Option<i64>, height: Option<i64>, full: Option<i64>| {
        let (x, y) = ::library::future_pinball::screen_place(&display?)?;
        let (device, mw, mh) = monitors.iter().find(|(_, mx, my, ..)| *mx == x && *my == y).map(|(d, _, _, w, h)| (d.clone(), *w, *h))?;
        let full = full.unwrap_or(0) >= 1;
        // a size larger than the screen, VPX's "as large as it goes", is the screen's
        let fit = |v: Option<i64>, max: u32| v.filter(|v| *v > 0 && !full).map_or(max, |v| (v as u32).min(max));
        Some(::library::future_pinball::Window { screen: device, width: fit(width, mw), height: fit(height, mh), full })
    };
    let playfield = window(get("Player", "PlayfieldDisplay"), number("Player", "PlayfieldWidth"), number("Player", "PlayfieldHeight"), number("Player", "PlayfieldFullScreen"));
    let backbox = (number("Backglass", "BackglassOutput") == Some(1))
        .then(|| window(get("Backglass", "BackglassDisplay"), number("Backglass", "BackglassWidth"), number("Backglass", "BackglassHeight"), number("Backglass", "BackglassFullScreen")))
        .flatten();
    ::library::future_pinball::screen_settings(playfield.as_ref(), backbox.as_ref(), pup_pack)
}

/// The playfield's screen as the app's Screens settings have VPX's playfield window: the screen's
/// place and size, in desktop pixels, and whether the window fills it.
fn playfield_screen() -> Option<(i32, i32, u32, u32, bool)> {
    let main = vpi_settings::read_all();
    let get = |k: &str| vpi_settings::get(&main, "Player", k).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let (x, y) = ::library::future_pinball::screen_place(&get("PlayfieldDisplay")?)?;
    let (_, _, (w, h)) = crate::b2s_screen::screens().into_iter().find(|((mx, my), ..)| *mx == x && *my == y)?;
    let full = get("PlayfieldFullScreen").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) >= 1;
    Some((x, y, w, h, full))
}

/// The part of the title of Future Pinball's player window that tells it from its other windows
/// ("Future Pinball - © 2008 BSP Software Design Solutions").
#[cfg(windows)]
const PLAYER_TITLE: &str = "BSP Software Design Solutions";

/// Future Pinball's player window kept without its border over the whole of its screen while a game
/// plays, until `ended` (`place`: the screen's place and size, in desktop pixels), as Borderless
/// Gaming keeps it for front ends — Future Pinball has a full screen and a window with a border,
/// nothing between: its border taken off and the window fitted to the screen once it shows, and
/// again should it put them back. Looked at ten times a second until it has stayed so for ten
/// seconds, then each second. What was found written in the app's log.
#[cfg(windows)]
fn keep_borderless(ended: &std::sync::atomic::AtomicBool, place: (i32, i32, u32, u32)) {
    use std::time::{Duration, Instant};
    let started = Instant::now();
    let mut fitted = 0u32;
    let mut last_fit: Option<(isize, Instant)> = None;
    let mut said_after = false;
    while !ended.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(hwnd) = player_window() {
            if !borderless_at(hwnd, place) {
                if fitted == 0 {
                    eprintln!("Future Pinball's window as it shows, {} ms after the start: {}; its windows: {}", started.elapsed().as_millis(), window_said(hwnd), windows_said());
                }
                make_borderless(hwnd, place);
                fitted += 1;
                last_fit = Some((hwnd, Instant::now()));
            } else if let Some((fitted_hwnd, at)) = last_fit
                && !said_after
                && at.elapsed() >= Duration::from_secs(1)
            {
                said_after = true;
                eprintln!("Future Pinball's window a moment after it was made borderless: {}{}", window_said(hwnd), if hwnd == fitted_hwnd { "" } else { " (another window)" });
            }
        }
        let settled = last_fit.is_some_and(|(_, at)| at.elapsed() >= Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(if settled { 1000 } else { 100 }));
    }
    match fitted {
        0 => eprintln!("Future Pinball's window was never made borderless: its player window wasn't found with a border, or out of place"),
        n => eprintln!("Future Pinball's window kept without its border over its screen ({n} times)"),
    }
}

#[cfg(not(windows))]
fn keep_borderless(_ended: &std::sync::atomic::AtomicBool, _place: (i32, i32, u32, u32)) {}

/// Future Pinball's player window, by its title: the largest of its program's windows shown with it.
#[cfg(windows)]
fn player_window() -> Option<isize> {
    crate::pause_menu::future_pinball_windows().into_iter().map(|(hwnd, ..)| hwnd).find(|&hwnd| crate::pause_menu::window_title(hwnd).contains(PLAYER_TITLE))
}

/// The window styles of a window's border: its caption, its sizing frame, its buttons.
#[cfg(windows)]
const BORDER_STYLES: u32 = 0x00C0_0000 | 0x0004_0000 | 0x0008_0000 | 0x0002_0000 | 0x0001_0000;

/// The extended window styles of a window's edges.
#[cfg(windows)]
const BORDER_EX_STYLES: u32 = 0x0000_0001 | 0x0000_0100 | 0x0000_0200 | 0x0002_0000;

/// A window's place and size in desktop pixels, its picture's size (its client area), its styles.
#[cfg(windows)]
fn window_facts(hwnd: isize) -> Option<((i32, i32, i32, i32), (i32, i32), u32, u32)> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn GetClientRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn GetWindowLongW(hwnd: *mut c_void, index: i32) -> i32;
    }
    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;
    let hwnd = hwnd as *mut c_void;
    let (mut outer, mut inner) = (Rect::default(), Rect::default());
    // SAFETY: a window Windows found, rectangles to fill
    if unsafe { GetWindowRect(hwnd, &mut outer) } == 0 || unsafe { GetClientRect(hwnd, &mut inner) } == 0 {
        return None;
    }
    // SAFETY: the same window, its styles asked for
    let (style, ex) = unsafe { (GetWindowLongW(hwnd, GWL_STYLE) as u32, GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) };
    Some(((outer.left, outer.top, outer.right - outer.left, outer.bottom - outer.top), (inner.right - inner.left, inner.bottom - inner.top), style, ex))
}

/// Whether a window has no border and fills `place` exactly; one minimised is left as it is.
#[cfg(windows)]
fn borderless_at(hwnd: isize, (x, y, w, h): (i32, i32, u32, u32)) -> bool {
    const WS_MINIMIZE: u32 = 0x2000_0000;
    let Some((outer, _, style, ex)) = window_facts(hwnd) else { return true };
    style & WS_MINIMIZE != 0 || (style & BORDER_STYLES == 0 && ex & BORDER_EX_STYLES == 0 && outer == (x, y, w as i32, h as i32))
}

/// A window's border taken off and the window fitted to `place`, without the focus or a change of
/// its order among the windows; asked of the window's program without waiting for it, which may be
/// busy loading its table.
#[cfg(windows)]
fn make_borderless(hwnd: isize, (x, y, w, h): (i32, i32, u32, u32)) {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowLongW(hwnd: *mut c_void, index: i32) -> i32;
        fn SetWindowLongW(hwnd: *mut c_void, index: i32, value: i32) -> i32;
        fn SetWindowPos(hwnd: *mut c_void, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    }
    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_NOOWNERZORDER: u32 = 0x0200;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    let hwnd = hwnd as *mut c_void;
    // SAFETY: a window Windows found; its styles read and set without the border's, then its place
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        if style & BORDER_STYLES != 0 {
            SetWindowLongW(hwnd, GWL_STYLE, (style & !BORDER_STYLES) as i32);
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & BORDER_EX_STYLES != 0 {
            SetWindowLongW(hwnd, GWL_EXSTYLE, (ex & !BORDER_EX_STYLES) as i32);
        }
        SetWindowPos(hwnd, 0, x, y, w as i32, h as i32, SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_ASYNCWINDOWPOS);
    }
}

/// A window as the app's log says it: its title, place, size, picture's size and styles.
#[cfg(windows)]
fn window_said(hwnd: isize) -> String {
    let title = crate::pause_menu::window_title(hwnd);
    match window_facts(hwnd) {
        Some(((x, y, w, h), (cw, ch), style, ex)) => format!("\"{title}\" at {x}, {y}, {w} x {h}, its picture {cw} x {ch}, styles {style:#010x} {ex:#010x}"),
        None => format!("\"{title}\", gone"),
    }
}

/// Every window of Future Pinball's shown, as the app's log says them.
#[cfg(windows)]
fn windows_said() -> String {
    crate::pause_menu::future_pinball_windows().into_iter().map(|(hwnd, ..)| window_said(hwnd)).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    /// Each screen's mode now, as a Future Pinball game's log writes it down:
    /// `cargo test -p app screen_modes_on_this_pc -- --ignored --nocapture`. Nothing changed.
    #[test]
    #[ignore]
    fn screen_modes_on_this_pc() {
        let modes = super::screen_modes();
        println!("{modes:?}");
        assert!(modes.iter().all(|&(_, w, h, hz)| w > 0 && h > 0 && hz > 0));
    }

    /// What Future Pinball's screens would be set to on this PC, from the app's Screens settings:
    /// `cargo test -p app future_pinball_screens_on_this_pc -- --ignored --nocapture`. Nothing written.
    #[test]
    #[ignore]
    fn future_pinball_screens_on_this_pc() {
        let layout = vpi_settings::library_folders_now().map(|f| ::library::plan::Layout::from_folders(&f)).expect("the library's folders");
        // VPX's settings as the app follows them
        ::library::vpx::follow(&layout);
        println!("{:?}", crate::b2s_screen::screens());
        for (name, value) in super::wanted(false) {
            println!("{name} = {value:?}");
        }
        println!("drawn at the screen's own pixels: {}", super::on_scaled_screen(true));
        // the screen a borderless window would fill, with the app's own pause menu on
        println!("the playfield's screen (place, size, filled): {:?}", super::playfield_screen());
        // the controller's buttons and triggers as the keys they'd be passed as
        let key = |name: &str| match super::registry::read(name) {
            Some(::library::future_pinball::Value::Number(n)) => Some(n),
            _ => None,
        };
        println!("the controller's keys: {:?}", ::library::controller_1080::keys_with(&vpi_settings::read_all(), |action| ::library::future_pinball::key_for(action, key)));
        // PinUP's screens as they'd be moved before a PinEvent table, nothing written
        let file = layout.pup_videos.parent().expect("PinUP's folder").join("PinUpPlayer.ini");
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let monitors: Vec<::library::pinup_screens::Monitor> = crate::b2s_screen::screens().into_iter().map(|((x, y), _, (w, h))| (x, y, w, h)).collect();
        println!("PinUP's screens: {:?}", ::library::pinup_screens::follow_monitors(&text, &monitors).map(|(_, said)| said));
    }
}

/// Future Pinball's settings put back as they were before the app set them for a game, where it
/// did: after the game, and at start, for a game the app didn't see end.
pub fn put_screens_back() {
    let Some(path) = record() else { return };
    let Ok(text) = std::fs::read(&path) else { return };
    let before: Vec<(String, Option<::library::future_pinball::Value>)> = serde_json::from_slice(&text).unwrap_or_default();
    for (name, value) in &before {
        let done = match value {
            Some(v) => registry::write(name, v),
            None => registry::remove(name),
        };
        if let Err(e) = done {
            eprintln!("Future Pinball's {name} wasn't put back: {e}");
            return;
        }
    }
    let _ = std::fs::remove_file(&path);
}

/// Windows' compatibility settings for programs, the user's own: each program by its path, its
/// flags after a `~`.
const LAYERS: &str = r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers";

/// Windows' "Override high DPI scaling behavior: Application" for a program: Windows leaves its
/// picture at the screen's own pixels. Future Pinball, made before Windows scaled screens, is
/// otherwise enlarged past the screen's edges on a screen Windows scales (above 100% in its
/// display settings), as Future Pinball's own guides say.
const DPI_FLAG: &str = "HIGHDPIAWARE";

/// Windows' "Disable fullscreen optimizations" for a program, which Future Pinball's guides set on
/// its program and BAM's loader against stutters in its exclusive full screen.
const FULLSCREEN_FLAG: &str = "DISABLEDXMAXIMIZEDWINDOWEDMODE";

/// The setting that turns Windows' fullscreen optimizations off for Future Pinball's games
/// (`[VPI]`, Settings › Library's Future Pinball part): off unless set.
pub const NO_OPTIMIZATIONS: &str = "FuturePinballDisableFullscreenOptimizations";

/// The record of Future Pinball's compatibility settings before the app set them for a game.
fn scaling_record() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Future Pinball").join("scaling before.json"))
}

/// Whether Future Pinball's window is the one in front: the foreground window's program, asked
/// again only when another window comes to the front.
#[cfg(windows)]
pub(crate) fn in_front() -> bool {
    use std::sync::Mutex;
    static LAST: Mutex<(isize, bool)> = Mutex::new((0, false));
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> *mut std::ffi::c_void;
        fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, pid: *mut u32) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, len: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // SAFETY: no arguments
    let window = unsafe { GetForegroundWindow() };
    let front = window as isize;
    let mut last = LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if front == last.0 {
        return last.1;
    }
    let mut pid = 0u32;
    // SAFETY: a window handle, perhaps none, and a number to fill
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    let mut its = false;
    // SAFETY: a process asked for its name alone
    let process = if pid != 0 { unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) } } else { 0 };
    if process != 0 {
        let mut name = [0u16; 1024];
        let mut len = name.len() as u32;
        // SAFETY: the process opened above, a buffer of `len` characters
        if unsafe { QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len) } != 0 {
            let path = String::from_utf16_lossy(&name[..len as usize]);
            its = Path::new(&path).file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(::library::future_pinball::PROGRAM));
        }
        // SAFETY: the handle opened above
        unsafe { CloseHandle(process) };
    }
    *last = (front, its);
    its
}

#[cfg(not(windows))]
pub(crate) fn in_front() -> bool {
    false
}

/// Whether Windows scales a screen Future Pinball draws on: the playfield's, or the backglass's as
/// its backbox, as the app's Screens settings have VPX's windows; any screen where they name none.
fn on_scaled_screen(pup_pack: bool) -> bool {
    let main = vpi_settings::read_all();
    let get = |s: &str, k: &str| vpi_settings::get(&main, s, k).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let mut displays = vec![get("Player", "PlayfieldDisplay")];
    if !pup_pack && get("Backglass", "BackglassOutput").as_deref() == Some("1") {
        displays.push(get("Backglass", "BackglassDisplay"));
    }
    let mut places: Vec<(i32, i32)> = displays.into_iter().flatten().filter_map(|d| ::library::future_pinball::screen_place(&d)).collect();
    if places.is_empty() {
        places = crate::b2s_screen::screens().into_iter().map(|(at, ..)| at).collect();
    }
    places.into_iter().filter_map(|(x, y)| crate::b2s_screen::scale_at(x, y)).any(|s| (s - 100.0).abs() > 0.5)
}

/// Windows' compatibility settings for a game: Future Pinball's program drawn at the screen's own
/// pixels where Windows scales a screen it draws on (`scaled`, `DPI_FLAG`), and fullscreen
/// optimizations off for it and BAM's loader where Settings › Library says so
/// (`FULLSCREEN_FLAG`); what was there kept first, to put back after.
fn set_compatibility(install: &::library::future_pinball::Install, scaled: bool) {
    put_scaling_back();
    let no_optimizations = vpi_settings::get(&vpi_settings::read_all(), "VPI", NO_OPTIMIZATIONS).and_then(vpi_settings::vpx_bool) == Some(true);
    let mut wanted: Vec<(PathBuf, Vec<&str>)> = Vec::new();
    let own: Vec<&str> = [scaled.then_some(DPI_FLAG), no_optimizations.then_some(FULLSCREEN_FLAG)].into_iter().flatten().collect();
    if !own.is_empty() {
        wanted.push((install.program(), own));
    }
    if let Some(loader) = install.loader.clone().filter(|_| no_optimizations) {
        wanted.push((loader, vec![FULLSCREEN_FLAG]));
    }
    // each program's flags as they are, and with those it lacks
    let mut before: Vec<(String, Option<String>)> = Vec::new();
    let mut after: Vec<(String, String)> = Vec::new();
    for (program, flags) in &wanted {
        let name = program.display().to_string();
        let was = registry::read_text_at(LAYERS, &name);
        let has = |flag: &str| was.as_deref().is_some_and(|b| b.split_whitespace().any(|f| f.eq_ignore_ascii_case(flag)));
        let lacking: Vec<&str> = flags.iter().copied().filter(|f| !has(f)).collect();
        if lacking.is_empty() {
            continue;
        }
        let now = match was.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
            Some(b) => format!("{b} {}", lacking.join(" ")),
            None => format!("~ {}", lacking.join(" ")),
        };
        before.push((name.clone(), was));
        after.push((name, now));
    }
    if after.is_empty() {
        return;
    }
    let Some(path) = scaling_record() else { return };
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
    if std::fs::write(&path, serde_json::to_vec(&before).unwrap_or_default()).is_err() {
        return;
    }
    for (name, now) in &after {
        match registry::write_text_at(LAYERS, name, now) {
            Ok(()) => eprintln!("Windows' compatibility settings for the game: {name} = {now}"),
            Err(e) => eprintln!("{name}'s compatibility settings weren't set: {e}"),
        }
    }
}

/// Future Pinball's compatibility settings put back as they were before the app set them for a
/// game, where it did: after the game, and at start, for a game the app didn't see end.
pub fn put_scaling_back() {
    let Some(path) = scaling_record() else { return };
    let Ok(text) = std::fs::read(&path) else { return };
    // each program's, or the one program's an earlier version kept
    let before: Vec<(String, Option<String>)> = match serde_json::from_slice(&text) {
        Ok(all) => all,
        Err(_) => match serde_json::from_slice::<(String, Option<String>)>(&text) {
            Ok(one) => vec![one],
            Err(_) => {
                let _ = std::fs::remove_file(&path);
                return;
            }
        },
    };
    for (name, was) in &before {
        let done = match was {
            Some(b) => registry::write_text_at(LAYERS, name, b),
            None => registry::remove_at(LAYERS, name),
        };
        if let Err(e) = done {
            eprintln!("{name}'s compatibility settings weren't put back: {e}");
            return;
        }
    }
    let _ = std::fs::remove_file(&path);
}

/// Future Pinball's player settings in Windows' registry (`library::future_pinball::SETTINGS_KEY`),
/// and a text in another of the user's keys.
#[cfg(windows)]
mod registry {
    use ::library::future_pinball::{SETTINGS_KEY, Value};
    use std::ffi::c_void;

    const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    const RRF_RT_REG_DWORD: u32 = 0x10;
    const RRF_RT_REG_SZ: u32 = 0x2;
    const REG_SZ: u32 = 1;
    const REG_DWORD: u32 = 4;
    const ERROR_FILE_NOT_FOUND: i32 = 2;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(hkey: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
        fn RegSetKeyValueW(hkey: isize, sub_key: *const u16, value: *const u16, kind: u32, data: *const c_void, len: u32) -> i32;
        fn RegDeleteKeyValueW(hkey: isize, sub_key: *const u16, value: *const u16) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn read(name: &str) -> Option<Value> {
        let (key, n) = (wide(SETTINGS_KEY), wide(name));
        let (mut number, mut len) = (0u32, 4u32);
        // SAFETY: NUL-ended names; four bytes for a number
        if unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut number as *mut u32).cast(), &mut len) } == 0 {
            return Some(Value::Number(number));
        }
        let mut text = vec![0u16; 260];
        let mut bytes = (text.len() * 2) as u32;
        // SAFETY: NUL-ended names; a buffer of `bytes` bytes for a text
        if unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), text.as_mut_ptr().cast(), &mut bytes) } == 0 {
            let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            return Some(Value::Text(String::from_utf16_lossy(&text[..len])));
        }
        None
    }

    pub fn write(name: &str, value: &Value) -> Result<(), String> {
        let (key, n) = (wide(SETTINGS_KEY), wide(name));
        // SAFETY: NUL-ended names; the data's bytes and length; the key made where it isn't there
        let done = unsafe {
            match value {
                Value::Number(v) => RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr(), REG_DWORD, (v as *const u32).cast(), 4),
                Value::Text(t) => {
                    let w = wide(t);
                    RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr(), REG_SZ, w.as_ptr().cast(), (w.len() * 2) as u32)
                }
            }
        };
        if done == 0 { Ok(()) } else { Err(std::io::Error::from_raw_os_error(done).to_string()) }
    }

    pub fn remove(name: &str) -> Result<(), String> {
        remove_at(SETTINGS_KEY, name)
    }

    pub fn remove_at(key: &str, name: &str) -> Result<(), String> {
        let (key, n) = (wide(key), wide(name));
        // SAFETY: NUL-ended names
        let done = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr()) };
        if done == 0 || done == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(std::io::Error::from_raw_os_error(done).to_string()) }
    }

    /// A text in the user's `key`, where it's there.
    pub fn read_text_at(key: &str, name: &str) -> Option<String> {
        let (key, n) = (wide(key), wide(name));
        let mut text = vec![0u16; 1024];
        let mut bytes = (text.len() * 2) as u32;
        // SAFETY: NUL-ended names; a buffer of `bytes` bytes for a text
        (unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), text.as_mut_ptr().cast(), &mut bytes) } == 0).then(|| {
            let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            String::from_utf16_lossy(&text[..len])
        })
    }

    /// A text set in the user's `key`, the key made where it isn't there.
    pub fn write_text_at(key: &str, name: &str, text: &str) -> Result<(), String> {
        let (key, n, w) = (wide(key), wide(name), wide(text));
        // SAFETY: NUL-ended names; the text's bytes and length
        let done = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), n.as_ptr(), REG_SZ, w.as_ptr().cast(), (w.len() * 2) as u32) };
        if done == 0 { Ok(()) } else { Err(std::io::Error::from_raw_os_error(done).to_string()) }
    }

    /// Where Future Pinball's installer put it, as Windows' list of installed programs has it: the
    /// install folder of a program named Future Pinball, the user's own list first, then the PC's,
    /// both views of it.
    pub fn installed_folder() -> Option<std::path::PathBuf> {
        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn RegOpenKeyExW(hkey: isize, sub_key: *const u16, options: u32, access: u32, out: *mut isize) -> i32;
            fn RegEnumKeyExW(hkey: isize, index: u32, name: *mut u16, len: *mut u32, reserved: *mut u32, class: *mut u16, class_len: *mut u32, time: *mut c_void) -> i32;
            fn RegCloseKey(hkey: isize) -> i32;
        }
        const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
        const KEY_READ: u32 = 0x20019;
        const WOW64_64: u32 = 0x0100;
        const WOW64_32: u32 = 0x0200;
        let uninstall = wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall");
        let text = |key: isize, name: &str| -> Option<String> {
            let n = wide(name);
            let mut buf = vec![0u16; 1024];
            let mut bytes = (buf.len() * 2) as u32;
            // SAFETY: an open key, a NUL-ended name, a buffer of `bytes` bytes
            (unsafe { RegGetValueW(key, std::ptr::null(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut bytes) } == 0).then(|| {
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                String::from_utf16_lossy(&buf[..len])
            })
        };
        for (root, view) in [(HKEY_CURRENT_USER, 0), (HKEY_LOCAL_MACHINE, WOW64_64), (HKEY_LOCAL_MACHINE, WOW64_32)] {
            let mut list = 0isize;
            // SAFETY: a key of Windows' own, read only, a handle to fill
            if unsafe { RegOpenKeyExW(root, uninstall.as_ptr(), 0, KEY_READ | view, &mut list) } != 0 {
                continue;
            }
            let mut found = None;
            for i in 0.. {
                let mut name = [0u16; 256];
                let mut len = name.len() as u32;
                // SAFETY: the open list, a buffer for its subkey's name
                if unsafe { RegEnumKeyExW(list, i, name.as_mut_ptr(), &mut len, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) } != 0 {
                    break;
                }
                let sub: Vec<u16> = name[..len as usize].iter().copied().chain(std::iter::once(0)).collect();
                let mut program = 0isize;
                // SAFETY: a subkey of the open list, read only
                if unsafe { RegOpenKeyExW(list, sub.as_ptr(), 0, KEY_READ | view, &mut program) } != 0 {
                    continue;
                }
                let named = text(program, "DisplayName").is_some_and(|n| n.to_lowercase().contains("future pinball"));
                let folder = if named { text(program, "InstallLocation") } else { None };
                // SAFETY: the key opened above
                unsafe { RegCloseKey(program) };
                if let Some(f) = folder.map(|f| std::path::PathBuf::from(f.trim().trim_end_matches('\\'))).filter(|f| f.join(::library::future_pinball::PROGRAM).is_file()) {
                    found = Some(f);
                    break;
                }
            }
            // SAFETY: the key opened above
            unsafe { RegCloseKey(list) };
            if found.is_some() {
                return found;
            }
        }
        None
    }
}

#[cfg(not(windows))]
mod registry {
    use ::library::future_pinball::Value;
    pub fn installed_folder() -> Option<std::path::PathBuf> {
        None
    }
    pub fn read(_name: &str) -> Option<Value> {
        None
    }
    pub fn write(_name: &str, _value: &Value) -> Result<(), String> {
        Ok(())
    }
    pub fn remove(_name: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn remove_at(_key: &str, _name: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn read_text_at(_key: &str, _name: &str) -> Option<String> {
        None
    }
    pub fn write_text_at(_key: &str, _name: &str, _text: &str) -> Result<(), String> {
        Ok(())
    }
}
